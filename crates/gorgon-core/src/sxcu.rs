//! ShareX custom uploader files (`.sxcu`).
//!
//! Gorgon reaches every upload destination and URL shortener through these,
//! so files exported by ShareX-compatible servers (XBackBone, Zipline, ...)
//! work as they are. Both syntaxes are understood: `{json:url}` (ShareX 13
//! and later) and the older `$json:url$`, picked by the file's `Version`.
//!
//! Anything Gorgon can't do (an `{xml:...}` lookup, a `{prompt}`) is reported
//! when the file is read, not discovered halfway through an upload.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SxcuError {
    #[error("not a valid .sxcu file: {0}")]
    Json(String),
    #[error("the file has no RequestURL")]
    NoUrl,
    #[error("unsupported request method {0:?}")]
    Method(String),
    #[error("unsupported body type {0:?}")]
    Body(String),
    #[error("{field}: {message}")]
    Syntax { field: String, message: String },
}

/// What a custom uploader is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Image,
    Text,
    File,
    UrlShortener,
    UrlSharing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    None,
    MultipartFormData,
    FormUrlEncoded,
    Json,
    Xml,
    Binary,
}

/// A parsed `.sxcu` file. Every text field is a [`Template`].
#[derive(Debug, Clone, PartialEq)]
pub struct Uploader {
    pub name: String,
    pub destinations: Vec<Destination>,
    pub method: Method,
    pub request_url: Template,
    pub parameters: Vec<(String, Template)>,
    pub headers: Vec<(String, Template)>,
    pub body: Body,
    pub arguments: Vec<(String, Template)>,
    pub file_form_name: Option<String>,
    pub data: Option<Template>,
    pub url: Option<Template>,
    pub thumbnail_url: Option<Template>,
    pub deletion_url: Option<Template>,
    pub error_message: Option<Template>,
}

/// The file as written, before validation. Field names follow ShareX.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct Raw {
    version: Option<String>,
    name: Option<String>,
    destination_type: Option<String>,
    request_method: Option<String>,
    /// ShareX before 13 called it this.
    request_type: Option<String>,
    #[serde(rename = "RequestURL")]
    request_url: Option<String>,
    parameters: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: Option<String>,
    arguments: BTreeMap<String, String>,
    file_form_name: Option<String>,
    data: Option<String>,
    regex_list: Vec<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
    #[serde(rename = "ThumbnailURL")]
    thumbnail_url: Option<String>,
    #[serde(rename = "DeletionURL")]
    deletion_url: Option<String>,
    error_message: Option<String>,
}

impl Uploader {
    /// Parses and validates a `.sxcu` file's contents.
    pub fn parse(text: &str) -> Result<Self, SxcuError> {
        // ShareX writes a BOM on some systems.
        let text = text.trim_start_matches('\u{feff}');
        let raw: Raw = serde_json::from_str(text).map_err(|e| SxcuError::Json(e.to_string()))?;
        let legacy = raw.version.as_deref().is_none_or(is_legacy_version);
        let syntax = Syntax {
            legacy,
            regex_list: &raw.regex_list,
        };
        let template = |field: &str, text: &str| {
            Template::parse_with(text, &syntax).map_err(|message| SxcuError::Syntax {
                field: field.to_owned(),
                message,
            })
        };
        let optional = |field: &str, text: &Option<String>| {
            text.as_deref()
                .filter(|t| !t.is_empty())
                .map(|t| template(field, t))
                .transpose()
        };
        let pairs = |field: &str, map: &BTreeMap<String, String>| {
            map.iter()
                .map(|(k, v)| Ok((k.clone(), template(&format!("{field}.{k}"), v)?)))
                .collect::<Result<Vec<_>, SxcuError>>()
        };

        let method = match raw
            .request_method
            .or(raw.request_type)
            .unwrap_or_else(|| "POST".into())
            .to_ascii_uppercase()
            .as_str()
        {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "PATCH" => Method::Patch,
            "DELETE" => Method::Delete,
            other => return Err(SxcuError::Method(other.to_owned())),
        };
        let file_form_name = raw.file_form_name.filter(|f| !f.is_empty());
        let mut parameters = pairs("Parameters", &raw.parameters)?;
        let mut arguments = pairs("Arguments", &raw.arguments)?;
        let body = match raw.body.as_deref() {
            Some("None") => Body::None,
            Some("MultipartFormData") => Body::MultipartFormData,
            Some("FormURLEncoded") => Body::FormUrlEncoded,
            Some("JSON") => Body::Json,
            Some("XML") => Body::Xml,
            Some("Binary") => Body::Binary,
            Some(other) => return Err(SxcuError::Body(other.to_owned())),
            // Older files have no Body: a file field means a multipart upload,
            // otherwise arguments went in the query (GET) or a form.
            None if file_form_name.is_some() => Body::MultipartFormData,
            None if method == Method::Get => {
                parameters.append(&mut arguments);
                Body::None
            }
            None if !arguments.is_empty() => Body::FormUrlEncoded,
            None => Body::None,
        };

        let destinations = raw
            .destination_type
            .as_deref()
            .unwrap_or("ImageUploader")
            .split(',')
            .filter_map(|d| match d.trim() {
                "ImageUploader" => Some(Destination::Image),
                "TextUploader" => Some(Destination::Text),
                "FileUploader" => Some(Destination::File),
                "URLShortener" => Some(Destination::UrlShortener),
                "URLSharingService" => Some(Destination::UrlSharing),
                _ => None,
            })
            .collect();

        let request_url = raw
            .request_url
            .filter(|u| !u.is_empty())
            .ok_or(SxcuError::NoUrl)?;
        let name = raw
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| {
                // Name it after the host, as ShareX does.
                request_url
                    .split("://")
                    .nth(1)
                    .and_then(|rest| rest.split(['/', ':', '?']).next())
                    .unwrap_or("Custom uploader")
                    .to_owned()
            });

        Ok(Self {
            name,
            destinations,
            method,
            request_url: template("RequestURL", &request_url)?,
            parameters,
            headers: pairs("Headers", &raw.headers)?,
            body,
            arguments,
            file_form_name,
            data: optional("Data", &raw.data)?,
            url: optional("URL", &raw.url)?,
            thumbnail_url: optional("ThumbnailURL", &raw.thumbnail_url)?,
            deletion_url: optional("DeletionURL", &raw.deletion_url)?,
            error_message: optional("ErrorMessage", &raw.error_message)?,
        })
    }

    /// Whether this uploader takes files (images, text or any file).
    pub fn uploads_files(&self) -> bool {
        self.destinations.iter().any(|d| {
            matches!(
                d,
                Destination::Image | Destination::File | Destination::Text
            )
        }) && (self.file_form_name.is_some() || self.body == Body::Binary)
    }

    pub fn shortens_urls(&self) -> bool {
        self.destinations.contains(&Destination::UrlShortener)
    }
}

/// ShareX switched from `$...$` to `{...}` in 13.0.
fn is_legacy_version(version: &str) -> bool {
    version
        .split('.')
        .next()
        .and_then(|major| major.trim().parse::<u32>().ok())
        .is_none_or(|major| major < 13)
}

struct Syntax<'a> {
    legacy: bool,
    regex_list: &'a [String],
}

/// A text with `{function:argument}` parts filled in at upload time.
#[derive(Debug, Clone, PartialEq)]
pub struct Template(Vec<Part>);

#[derive(Debug, Clone, PartialEq)]
enum Part {
    Text(String),
    Call(Call),
}

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Filename,
    Input,
    Response,
    ResponseUrl,
    Header(Template),
    Json(Template),
    Regex {
        pattern: Template,
        group: Option<String>,
    },
    Random(Vec<Template>),
    Base64(Template),
}

/// Functions ShareX has that Gorgon can't run, with why.
const UNSUPPORTED: &[(&str, &str)] = &[
    ("xml", "XML responses aren't supported"),
    (
        "prompt",
        "asking for input during an upload isn't supported",
    ),
    (
        "inputbox",
        "asking for input during an upload isn't supported",
    ),
    (
        "select",
        "asking for input during an upload isn't supported",
    ),
    ("outputurl", "{outputurl} isn't supported"),
];

const KNOWN: &[&str] = &[
    "filename",
    "input",
    "response",
    "responseurl",
    "header",
    "json",
    "regex",
    "random",
    "base64",
];

impl Template {
    /// Parses a template in the current (`{...}`) syntax.
    pub fn parse(text: &str) -> Result<Self, String> {
        Self::parse_with(
            text,
            &Syntax {
                legacy: false,
                regex_list: &[],
            },
        )
    }

    fn parse_with(text: &str, syntax: &Syntax) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let open = if syntax.legacy { '$' } else { '{' };
        while i < chars.len() {
            let c = chars[i];
            if !syntax.legacy && c == '\\' && i + 1 < chars.len() && "{}|\\".contains(chars[i + 1])
            {
                literal.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == open {
                if let Some((call, len)) = parse_call(&chars[i..], syntax)? {
                    if !literal.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut literal)));
                    }
                    parts.push(Part::Call(call));
                    i += len;
                    continue;
                }
            }
            literal.push(c);
            i += 1;
        }
        if !literal.is_empty() {
            parts.push(Part::Text(literal));
        }
        Ok(Self(parts))
    }

    /// Fills the template in. Values from functions are escaped with
    /// `escape`, literal text is left as written.
    pub fn render(&self, ctx: &Context, escape: Escape) -> Result<String, String> {
        let mut out = String::new();
        for part in &self.0 {
            match part {
                Part::Text(t) => out.push_str(t),
                Part::Call(call) => out.push_str(&escape.apply(&call.eval(ctx)?)),
            }
        }
        Ok(out)
    }

    /// Whether the template reads the server's response.
    pub fn uses_response(&self) -> bool {
        self.0.iter().any(|p| match p {
            Part::Call(c) => c.uses_response(),
            Part::Text(_) => false,
        })
    }
}

/// Reads one `{name:arg}` (or legacy `$name:arg$`) starting at `chars[0]`.
/// Returns `None` when it isn't one, so the text stays literal: JSON bodies
/// are full of braces that aren't templates.
fn parse_call(chars: &[char], syntax: &Syntax) -> Result<Option<(Call, usize)>, String> {
    let (inner, len) = if syntax.legacy {
        match chars[1..].iter().position(|&c| c == '$') {
            Some(end) => (chars[1..1 + end].iter().collect::<String>(), end + 2),
            None => return Ok(None),
        }
    } else {
        // Find the matching brace, allowing nested templates in the argument.
        let mut depth = 0;
        let mut end = None;
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '\\' => i += 1,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        match end {
            Some(end) => (chars[1..end].iter().collect::<String>(), end + 1),
            None => return Ok(None),
        }
    };

    let (name, arg) = match inner.split_once(':') {
        Some((n, a)) => (n.trim().to_ascii_lowercase(), Some(a.to_owned())),
        None => (inner.trim().to_ascii_lowercase(), None),
    };
    if let Some((_, why)) = UNSUPPORTED.iter().find(|(n, _)| *n == name) {
        return Err((*why).to_owned());
    }
    // Legacy files refer to their RegexList: $regex:1,2$ is list entry 1, group 2.
    if syntax.legacy && name == "regex" {
        let arg = arg.unwrap_or_default();
        let (index, group) = arg.split_once(',').unwrap_or((&arg, ""));
        let pattern = index
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| syntax.regex_list.get(n.checked_sub(1)?))
            .ok_or_else(|| format!("$regex:{arg}$ refers to a missing RegexList entry"))?;
        let group = Some(group.trim().to_owned()).filter(|g| !g.is_empty());
        let pattern = Template(vec![Part::Text(pattern.clone())]);
        return Ok(Some((Call::Regex { pattern, group }, len)));
    }
    if !KNOWN.contains(&name.as_str()) {
        return Ok(None);
    }

    let sub = |text: &str| Template::parse_with(text, syntax);
    let needs_arg = |what: &str| {
        arg.clone()
            .filter(|a| !a.is_empty())
            .ok_or_else(|| format!("{{{name}}} needs {what}"))
    };
    let call = match name.as_str() {
        "filename" => Call::Filename,
        "input" => Call::Input,
        "response" => Call::Response,
        "responseurl" => Call::ResponseUrl,
        "header" => Call::Header(sub(&needs_arg("a header name")?)?),
        "json" => Call::Json(sub(&needs_arg("a JSON path")?)?),
        "base64" => Call::Base64(sub(&needs_arg("a text")?)?),
        "random" => Call::Random(
            split_unescaped(&needs_arg("choices")?)
                .iter()
                .map(|c| sub(c))
                .collect::<Result<_, _>>()?,
        ),
        "regex" => {
            let arg = needs_arg("a pattern")?;
            let mut pieces = split_unescaped(&arg);
            let group = (pieces.len() > 1).then(|| pieces.pop().unwrap_or_default());
            let pattern = pieces.join("|");
            regex::Regex::new(&pattern).map_err(|e| format!("bad regex {pattern:?}: {e}"))?;
            Call::Regex {
                pattern: Template(vec![Part::Text(pattern)]),
                group,
            }
        }
        _ => unreachable!("checked against KNOWN"),
    };
    Ok(Some((call, len)))
}

/// Splits on `|` that isn't escaped as `\|`; escapes are removed.
fn split_unescaped(text: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                parts.last_mut().unwrap().push('|');
                chars.next();
            }
            '|' => parts.push(String::new()),
            c => parts.last_mut().unwrap().push(c),
        }
    }
    parts
}

/// What a template can read while it is filled in.
#[derive(Debug, Default, Clone)]
pub struct Context {
    /// The uploaded file's name.
    pub filename: Option<String>,
    /// The text being sent: a URL to shorten, or a text upload.
    pub input: Option<String>,
    pub response: Option<Response>,
}

/// What the server answered.
#[derive(Debug, Default, Clone)]
pub struct Response {
    pub status: u16,
    /// The final URL, after redirects.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// How values are escaped for the place they're written into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escape {
    None,
    /// Inside a URL.
    Url,
    /// Inside a JSON string.
    Json,
    Xml,
}

impl Escape {
    fn apply(self, value: &str) -> String {
        match self {
            Self::None => value.to_owned(),
            Self::Url => {
                let mut out = String::new();
                for b in value.bytes() {
                    if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                        out.push(b as char);
                    } else {
                        out.push_str(&format!("%{b:02X}"));
                    }
                }
                out
            }
            Self::Json => {
                let quoted = serde_json::to_string(value).unwrap_or_default();
                quoted[1..quoted.len() - 1].to_owned()
            }
            Self::Xml => value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;"),
        }
    }
}

impl Call {
    fn uses_response(&self) -> bool {
        match self {
            Self::Response | Self::ResponseUrl | Self::Header(_) | Self::Json(_) => true,
            Self::Regex { .. } => true,
            Self::Filename | Self::Input => false,
            Self::Random(choices) => choices.iter().any(Template::uses_response),
            Self::Base64(t) => t.uses_response(),
        }
    }

    fn eval(&self, ctx: &Context) -> Result<String, String> {
        let response = || {
            ctx.response
                .as_ref()
                .ok_or_else(|| "the response isn't available here".to_owned())
        };
        let arg = |t: &Template| t.render(ctx, Escape::None);
        Ok(match self {
            Self::Filename => ctx.filename.clone().unwrap_or_default(),
            Self::Input => ctx.input.clone().unwrap_or_default(),
            Self::Response => response()?.body.clone(),
            Self::ResponseUrl => response()?.url.clone(),
            Self::Header(name) => {
                let name = arg(name)?;
                response()?
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&name))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            }
            Self::Json(path) => {
                let path = arg(path)?;
                let value: serde_json::Value =
                    serde_json::from_str(&response()?.body).map_err(|_| {
                        format!("the response isn't JSON, so {{json:{path}}} can't be read")
                    })?;
                match json_path(&value, &path) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Null) | None => String::new(),
                    Some(other) => other.to_string(),
                }
            }
            Self::Regex { pattern, group } => {
                let pattern = arg(pattern)?;
                let re = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
                let body = &response()?.body;
                let Some(caps) = re.captures(body) else {
                    return Ok(String::new());
                };
                let m = match group.as_deref() {
                    None => caps.get(0),
                    Some(g) => match g.parse::<usize>() {
                        Ok(n) => caps.get(n),
                        Err(_) => caps.name(g),
                    },
                };
                m.map(|m| m.as_str().to_owned()).unwrap_or_default()
            }
            Self::Random(choices) => {
                let pick = &choices[fastrand::usize(..choices.len())];
                arg(pick)?
            }
            Self::Base64(text) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(arg(text)?)
            }
        })
    }
}

/// Looks up a path like `files[0].url`, `$.data.link` or `['key']`.
fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let path = path.trim();
    let path = path.strip_prefix('$').unwrap_or(path);
    let mut current = value;
    let mut chars = path.chars().peekable();
    let mut key = String::new();
    let step = |current: &'a serde_json::Value, key: &str| -> Option<&'a serde_json::Value> {
        if key.is_empty() {
            return Some(current);
        }
        current.get(key)
    };
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                current = step(current, &key)?;
                key.clear();
            }
            '[' => {
                current = step(current, &key)?;
                key.clear();
                let mut inner = String::new();
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                    inner.push(c);
                }
                let inner = inner.trim();
                current = if let Ok(n) = inner.parse::<usize>() {
                    current.get(n)?
                } else {
                    current.get(inner.trim_matches(['\'', '"']))?
                };
            }
            c => key.push(c),
        }
    }
    step(current, &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_body(body: &str) -> Context {
        Context {
            filename: Some("shot.png".into()),
            input: Some("https://example.com/a b".into()),
            response: Some(Response {
                status: 200,
                url: "https://up.example.com/upload".into(),
                headers: vec![("Location".into(), "https://up.example.com/x".into())],
                body: body.into(),
            }),
        }
    }

    fn render(t: &str, ctx: &Context) -> String {
        Template::parse(t)
            .unwrap()
            .render(ctx, Escape::None)
            .unwrap()
    }

    #[test]
    fn xbackbone_legacy_export() {
        // As XBackBone generates it: no Version, no Body, $...$ syntax.
        let file = r#"{
            "DestinationType": "ImageUploader, TextUploader, FileUploader",
            "RequestURL": "https://xbb.example.com/upload",
            "FileFormName": "upload",
            "Arguments": { "file": "$filename$", "text": "$input$", "token": "token_abc" },
            "URL": "$json:url$",
            "ThumbnailURL": "$json:url$/raw",
            "DeletionURL": "$json:url$/delete/token_abc"
        }"#;
        let up = Uploader::parse(file).unwrap();
        assert_eq!(up.name, "xbb.example.com");
        assert_eq!(up.method, Method::Post);
        assert_eq!(up.body, Body::MultipartFormData);
        assert!(up.uploads_files());
        assert!(!up.shortens_urls());
        let ctx = ctx_with_body(r#"{"message":"OK","url":"https://xbb.example.com/AbC/xyz"}"#);
        let url = up.url.as_ref().unwrap().render(&ctx, Escape::None).unwrap();
        assert_eq!(url, "https://xbb.example.com/AbC/xyz");
        let thumb = up
            .thumbnail_url
            .unwrap()
            .render(&ctx, Escape::None)
            .unwrap();
        assert_eq!(thumb, "https://xbb.example.com/AbC/xyz/raw");
        let file_arg = &up.arguments.iter().find(|(k, _)| k == "file").unwrap().1;
        assert_eq!(file_arg.render(&ctx, Escape::None).unwrap(), "shot.png");
    }

    #[test]
    fn kutt_shortener() {
        let file = r#"{
            "Version": "16.0.0",
            "Name": "Kutt",
            "DestinationType": "URLShortener",
            "RequestMethod": "POST",
            "RequestURL": "https://kutt.example.com/api/v2/links",
            "Headers": { "X-API-KEY": "key" },
            "Body": "JSON",
            "Data": "{\"target\":\"{input}\"}",
            "URL": "{json:link}"
        }"#;
        let up = Uploader::parse(file).unwrap();
        assert!(up.shortens_urls());
        assert!(!up.uploads_files());
        let ctx = Context {
            input: Some("https://x.com/\"quoted\"".into()),
            ..Default::default()
        };
        let data = up.data.unwrap().render(&ctx, Escape::Json).unwrap();
        assert_eq!(data, r#"{"target":"https://x.com/\"quoted\""}"#);
        let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(parsed["target"], "https://x.com/\"quoted\"");
    }

    #[test]
    fn json_paths() {
        let ctx = ctx_with_body(r#"{"files":[{"url":"a"},{"url":"b"}],"data":{"link":"L","n":3}}"#);
        assert_eq!(render("{json:files[1].url}", &ctx), "b");
        assert_eq!(render("{json:$.data.link}", &ctx), "L");
        assert_eq!(render("{json:data['link']}", &ctx), "L");
        assert_eq!(render("{json:data.n}", &ctx), "3");
        assert_eq!(render("{json:missing}", &ctx), "");
    }

    #[test]
    fn xbackbone_raw_link() {
        // examples/xbackbone.sxcu turns the preview link into the raw file's
        // link, whether or not the server escapes slashes in its JSON.
        let up = Uploader::parse(include_str!("../../../examples/xbackbone.sxcu")).unwrap();
        for body in [
            r#"{"message":"OK","version":"3.8.1","url":"https:\/\/x.example.eu\/me\/AbC123.gif"}"#,
            r#"{"message":"OK","version":"3.8.1","url":"https://x.example.eu/me/AbC123.gif"}"#,
        ] {
            let ctx = ctx_with_body(body);
            assert_eq!(
                up.url.as_ref().unwrap().render(&ctx, Escape::None).unwrap(),
                "https://x.example.eu/me/AbC123/raw.gif"
            );
            assert_eq!(
                up.deletion_url
                    .as_ref()
                    .unwrap()
                    .render(&ctx, Escape::None)
                    .unwrap(),
                "https://x.example.eu/me/AbC123.gif/delete/YOUR_TOKEN"
            );
        }
    }

    #[test]
    fn regex_header_and_response() {
        let ctx = ctx_with_body("done: https://i.example.com/k3y.png ok");
        assert_eq!(
            render(r"{regex:https://\S+}", &ctx),
            "https://i.example.com/k3y.png"
        );
        assert_eq!(render(r"{regex:/(\w+)\.png|1}", &ctx), "k3y");
        assert_eq!(render(r"{regex:/(?<id>\w+)\.png|id}", &ctx), "k3y");
        assert_eq!(
            render("{header:location}", &ctx),
            "https://up.example.com/x"
        );
        assert_eq!(
            render("{responseurl}", &ctx),
            "https://up.example.com/upload"
        );
        assert_eq!(
            render("<{response}>", &ctx),
            "<done: https://i.example.com/k3y.png ok>"
        );
    }

    #[test]
    fn legacy_regex_list() {
        let file = r#"{
            "RequestURL": "https://x.example.com",
            "FileFormName": "f",
            "RegexList": ["id=(\\w+)"],
            "URL": "https://x.example.com/$regex:1,1$"
        }"#;
        let up = Uploader::parse(file).unwrap();
        let ctx = ctx_with_body("id=abc");
        assert_eq!(
            up.url.unwrap().render(&ctx, Escape::None).unwrap(),
            "https://x.example.com/abc"
        );
    }

    #[test]
    fn escapes_and_literals() {
        let ctx = ctx_with_body("");
        assert_eq!(render(r"\{input\}", &ctx), "{input}");
        assert_eq!(render("{not a function}", &ctx), "{not a function}");
        assert_eq!(render("{base64:user:pass}", &ctx), "dXNlcjpwYXNz");
        let t = Template::parse("q={input}").unwrap();
        assert_eq!(
            t.render(&ctx, Escape::Url).unwrap(),
            "q=https%3A%2F%2Fexample.com%2Fa%20b"
        );
        assert!(["a", "b"].contains(&render("{random:a|b}", &ctx).as_str()));
    }

    #[test]
    fn unsupported_syntax_is_reported() {
        let file = r#"{"Version":"15.0.0","RequestURL":"https://x","URL":"{xml:/a/b}"}"#;
        let err = Uploader::parse(file).unwrap_err();
        assert_eq!(
            err,
            SxcuError::Syntax {
                field: "URL".into(),
                message: "XML responses aren't supported".into()
            }
        );
        assert_eq!(
            Uploader::parse(r#"{"Name":"x"}"#).unwrap_err(),
            SxcuError::NoUrl
        );
    }

    #[test]
    fn get_without_body_sends_arguments_as_query() {
        let file = r#"{"RequestType":"GET","RequestURL":"https://s.example.com/api","Arguments":{"url":"$input$"},"DestinationType":"URLShortener"}"#;
        let up = Uploader::parse(file).unwrap();
        assert_eq!(up.method, Method::Get);
        assert_eq!(up.body, Body::None);
        assert_eq!(up.parameters.len(), 1);
    }
}
