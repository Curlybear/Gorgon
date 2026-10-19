//! Running a custom uploader: build the request, send it, read the result.

use std::{path::Path, time::Duration};

use crate::sxcu::{Body, Context, Escape, Method, Response, Template, Uploader};

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("{0}")]
    Template(String),
    #[error("{uploader} can't {what}")]
    WrongKind {
        uploader: String,
        what: &'static str,
    },
    #[error("could not reach {host}: {source}")]
    Network {
        host: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{uploader} answered {status}: {message}")]
    Server {
        uploader: String,
        status: u16,
        message: String,
    },
    #[error("{uploader} didn't return a URL (response: {body})")]
    NoUrl { uploader: String, body: String },
    #[error("{uploader} returned a link Gorgon won't open: {link}")]
    BadLink { uploader: String, link: String },
}

/// What is being sent.
pub enum Input<'a> {
    File {
        name: &'a str,
        bytes: Vec<u8>,
        mime: &'a str,
    },
    /// A URL to shorten, or a text to upload.
    Text(&'a str),
}

impl<'a> Input<'a> {
    /// Reads a file to upload.
    pub fn file(path: &'a Path, bytes: Vec<u8>) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        Self::File {
            name,
            bytes,
            mime: mime_for(path),
        }
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Links returned by an upload.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Uploaded {
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub deletion_url: Option<String>,
}

/// A request ready to send, kept separate from sending so it can be tested.
#[derive(Debug, PartialEq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: RequestBody,
}

#[derive(Debug, PartialEq)]
pub enum RequestBody {
    None,
    Multipart {
        fields: Vec<(String, String)>,
        file: Option<FilePart>,
    },
    Form(Vec<(String, String)>),
    Raw {
        content_type: String,
        bytes: Vec<u8>,
    },
}

#[derive(Debug, PartialEq)]
pub struct FilePart {
    pub field: String,
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Fills in the uploader's templates for this input.
pub fn build(uploader: &Uploader, input: Input) -> Result<Request, UploadError> {
    let ctx = match &input {
        Input::File { name, .. } => Context {
            filename: Some((*name).to_owned()),
            ..Default::default()
        },
        Input::Text(text) => Context {
            input: Some((*text).to_owned()),
            ..Default::default()
        },
    };
    let fill = |t: &Template, escape| t.render(&ctx, escape).map_err(UploadError::Template);
    let pairs = |list: &[(String, Template)]| {
        list.iter()
            .map(|(k, v)| Ok((k.clone(), fill(v, Escape::None)?)))
            .collect::<Result<Vec<_>, UploadError>>()
    };

    let mut headers = pairs(&uploader.headers)?;
    let has_header = |headers: &[(String, String)], name: &str| {
        headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
    };
    let raw = |content_type: &str, escape, headers: &[(String, String)]| {
        let data = uploader
            .data
            .as_ref()
            .map(|d| fill(d, escape))
            .transpose()?
            .unwrap_or_default();
        let content_type = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map_or(content_type.to_owned(), |(_, v)| v.clone());
        Ok::<_, UploadError>(RequestBody::Raw {
            content_type,
            bytes: data.into_bytes(),
        })
    };

    let body = match uploader.body {
        Body::None => RequestBody::None,
        Body::MultipartFormData => {
            let file =
                match input {
                    Input::File { name, bytes, mime } => {
                        let field = uploader.file_form_name.clone().ok_or_else(|| {
                            UploadError::WrongKind {
                                uploader: uploader.name.clone(),
                                what: "take files (it has no FileFormName)",
                            }
                        })?;
                        Some(FilePart {
                            field,
                            name: name.to_owned(),
                            mime: mime.to_owned(),
                            bytes,
                        })
                    }
                    Input::Text(_) => None,
                };
            RequestBody::Multipart {
                fields: pairs(&uploader.arguments)?,
                file,
            }
        }
        Body::FormUrlEncoded => RequestBody::Form(pairs(&uploader.arguments)?),
        Body::Json => raw("application/json", Escape::Json, &headers)?,
        Body::Xml => raw("application/xml", Escape::Xml, &headers)?,
        Body::Binary => match input {
            Input::File { bytes, mime, .. } => RequestBody::Raw {
                content_type: mime.to_owned(),
                bytes,
            },
            Input::Text(text) => RequestBody::Raw {
                content_type: "text/plain".into(),
                bytes: text.as_bytes().to_vec(),
            },
        },
    };
    if let RequestBody::Raw { .. } = body {
        // The content type travels with the body; don't send it twice.
        headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
    }
    if !has_header(&headers, "user-agent") {
        headers.push((
            "User-Agent".into(),
            concat!("Gorgon/", env!("CARGO_PKG_VERSION")).into(),
        ));
    }

    Ok(Request {
        method: uploader.method,
        url: fill(&uploader.request_url, Escape::Url)?,
        query: pairs(&uploader.parameters)?,
        headers,
        body,
    })
}

/// Reads the links out of the server's response.
pub fn parse_response(
    uploader: &Uploader,
    input_text: Option<&str>,
    response: Response,
) -> Result<Uploaded, UploadError> {
    let ctx = Context {
        input: input_text.map(str::to_owned),
        response: Some(response),
        ..Default::default()
    };
    let response = ctx.response.as_ref().expect("set above");
    let fill = |t: &Option<Template>| -> Result<Option<String>, UploadError> {
        Ok(t.as_ref()
            .map(|t| t.render(&ctx, Escape::None))
            .transpose()
            .map_err(UploadError::Template)?
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()))
    };
    if !(200..300).contains(&response.status) {
        let message = fill(&uploader.error_message)
            .ok()
            .flatten()
            .unwrap_or_else(|| snippet(&response.body));
        return Err(UploadError::Server {
            uploader: uploader.name.clone(),
            status: response.status,
            message,
        });
    }
    // Without a URL template, ShareX takes the whole response as the URL.
    let url = match &uploader.url {
        Some(_) => fill(&uploader.url)?,
        None => Some(response.body.trim().to_owned()).filter(|s| !s.is_empty()),
    };
    let url = url.ok_or_else(|| UploadError::NoUrl {
        uploader: uploader.name.clone(),
        body: snippet(&response.body),
    })?;
    if !is_web_link(&url) {
        return Err(UploadError::BadLink {
            uploader: uploader.name.clone(),
            link: snippet(&url),
        });
    }
    // The extra links are optional: drop one that isn't a web link rather
    // than failing an upload that worked.
    Ok(Uploaded {
        url,
        thumbnail_url: fill(&uploader.thumbnail_url)?.filter(|l| is_web_link(l)),
        deletion_url: fill(&uploader.deletion_url)?.filter(|l| is_web_link(l)),
    })
}

/// Whether `link` is an `http://` or `https://` link. Links come from the
/// server and end up opened with a click, so `file:` and custom schemes
/// (which hand arguments to other programs) are refused.
pub fn is_web_link(link: &str) -> bool {
    let lower = link.get(..8).unwrap_or(link).to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !link.chars().any(char::is_whitespace)
}

fn snippet(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return "(empty)".into();
    }
    let mut s: String = body.chars().take(200).collect();
    if s.len() < body.len() {
        s.push('…');
    }
    s
}

/// The most of a response that is read; the rest is ignored. Upload APIs
/// answer with a few hundred bytes of JSON.
const MAX_RESPONSE: usize = 1024 * 1024;

fn client() -> reqwest::Client {
    // rustls needs a crypto provider; ignore "already installed".
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(same_origin_redirects))
        .build()
        .expect("HTTP client")
}

/// Follows redirects only within the same scheme, host and port. Headers
/// like Kutt's `X-API-KEY` and the uploaded form (with its token) would
/// otherwise go along to wherever the server points.
fn same_origin_redirects(attempt: reqwest::redirect::Attempt) -> reqwest::redirect::Action {
    let origin = |u: &reqwest::Url| {
        (
            u.scheme().to_owned(),
            u.host_str().map(str::to_owned),
            u.port_or_known_default(),
        )
    };
    let from = attempt.previous().last().map(origin);
    if attempt.previous().len() > 5 {
        attempt.error("too many redirects")
    } else if from.is_some_and(|f| f != origin(attempt.url())) {
        let to = attempt.url().host_str().unwrap_or("?").to_owned();
        attempt.error(format!("refused a redirect to {to}"))
    } else {
        attempt.follow()
    }
}

/// Sends a built request and returns the raw response.
pub async fn send(request: Request) -> Result<Response, reqwest::Error> {
    let method =
        reqwest::Method::from_bytes(request.method.as_str().as_bytes()).expect("valid method");
    let mut req = client().request(method, &request.url);
    if !request.query.is_empty() {
        req = req.query(&request.query);
    }
    for (k, v) in &request.headers {
        req = req.header(k, v);
    }
    req = match request.body {
        RequestBody::None => req,
        RequestBody::Form(fields) => req.form(&fields),
        RequestBody::Raw {
            content_type,
            bytes,
        } => req
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes),
        RequestBody::Multipart { fields, file } => {
            let mut form = reqwest::multipart::Form::new();
            for (k, v) in fields {
                form = form.text(k, v);
            }
            if let Some(f) = file {
                let part = reqwest::multipart::Part::bytes(f.bytes)
                    .file_name(f.name)
                    .mime_str(&f.mime)?;
                form = form.part(f.field, part);
            }
            req.multipart(form)
        }
    };
    let mut resp = req.send().await?;
    let status = resp.status().as_u16();
    let url = resp.url().to_string();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        let room = MAX_RESPONSE - bytes.len();
        bytes.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if bytes.len() == MAX_RESPONSE {
            break;
        }
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();
    Ok(Response {
        status,
        url,
        headers,
        body,
    })
}

fn host_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|r| r.split('/').next())
        .unwrap_or(url)
        .to_owned()
}

/// Uploads a file with an image or file uploader.
pub async fn upload_file(uploader: &Uploader, path: &Path) -> Result<Uploaded, UploadError> {
    if !uploader.uploads_files() {
        return Err(UploadError::WrongKind {
            uploader: uploader.name.clone(),
            what: "upload files",
        });
    }
    let bytes = std::fs::read(path)
        .map_err(|e| UploadError::Template(format!("could not read {}: {e}", path.display())))?;
    let request = build(uploader, Input::file(path, bytes))?;
    let host = host_of(&request.url);
    let response = send(request)
        .await
        .map_err(|source| UploadError::Network { host, source })?;
    parse_response(uploader, None, response)
}

/// Shortens a URL with a URL shortener.
pub async fn shorten(uploader: &Uploader, url: &str) -> Result<String, UploadError> {
    if !uploader.shortens_urls() {
        return Err(UploadError::WrongKind {
            uploader: uploader.name.clone(),
            what: "shorten URLs",
        });
    }
    let request = build(uploader, Input::Text(url))?;
    let host = host_of(&request.url);
    let response = send(request)
        .await
        .map_err(|source| UploadError::Network { host, source })?;
    Ok(parse_response(uploader, Some(url), response)?.url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_json, header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn xbackbone(base: &str) -> Uploader {
        Uploader::parse(&format!(
            r#"{{
                "DestinationType": "ImageUploader, TextUploader, FileUploader",
                "RequestURL": "{base}/upload",
                "FileFormName": "upload",
                "Arguments": {{ "file": "$filename$", "text": "$input$", "token": "token_abc" }},
                "URL": "$json:url$",
                "ThumbnailURL": "$json:url$/raw",
                "DeletionURL": "$json:url$/delete/token_abc"
            }}"#
        ))
        .unwrap()
    }

    fn kutt(base: &str) -> Uploader {
        Uploader::parse(&format!(
            r#"{{
                "Version": "16.0.0",
                "Name": "Kutt",
                "DestinationType": "URLShortener",
                "RequestMethod": "POST",
                "RequestURL": "{base}/api/v2/links",
                "Headers": {{ "X-API-KEY": "secret" }},
                "Body": "JSON",
                "Data": "{{\"target\":\"{{input}}\"}}",
                "URL": "{{json:link}}",
                "ErrorMessage": "{{json:error}}"
            }}"#
        ))
        .unwrap()
    }

    #[test]
    fn multipart_request_for_xbackbone() {
        let up = xbackbone("https://xbb.example.com");
        let req = build(
            &up,
            Input::File {
                name: "shot.png",
                bytes: vec![1, 2, 3],
                mime: "image/png",
            },
        )
        .unwrap();
        assert_eq!(req.url, "https://xbb.example.com/upload");
        let RequestBody::Multipart { fields, file } = req.body else {
            panic!("expected multipart");
        };
        assert!(fields.contains(&("token".into(), "token_abc".into())));
        assert!(fields.contains(&("file".into(), "shot.png".into())));
        let file = file.unwrap();
        assert_eq!((file.field.as_str(), file.bytes.len()), ("upload", 3));
    }

    #[tokio::test]
    async fn uploads_to_xbackbone() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "message": "OK", "version": "3.8.0", "url": "https://xbb.example.com/Ab/Cd"
            })))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shot.png");
        std::fs::write(&file, b"png").unwrap();

        let up = xbackbone(&server.uri());
        let result = upload_file(&up, &file).await.unwrap();
        assert_eq!(result.url, "https://xbb.example.com/Ab/Cd");
        assert_eq!(
            result.thumbnail_url.as_deref(),
            Some("https://xbb.example.com/Ab/Cd/raw")
        );

        let received = &server.received_requests().await.unwrap()[0];
        let body = String::from_utf8_lossy(&received.body);
        assert!(body.contains("name=\"upload\"; filename=\"shot.png\""));
        assert!(body.contains("name=\"token\"\r\n\r\ntoken_abc"));
    }

    #[tokio::test]
    async fn shortens_with_kutt() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/links"))
            .and(header("X-API-KEY", "secret"))
            .and(header("content-type", "application/json"))
            .and(body_json(
                serde_json::json!({ "target": "https://xbb.example.com/Ab/Cd" }),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "1", "link": "https://kutt.example.com/q9", "target": "..."
            })))
            .mount(&server)
            .await;
        let up = kutt(&server.uri());
        let short = shorten(&up, "https://xbb.example.com/Ab/Cd").await.unwrap();
        assert_eq!(short, "https://kutt.example.com/q9");
    }

    #[tokio::test]
    async fn server_errors_use_error_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({ "error": "API key invalid" })),
            )
            .mount(&server)
            .await;
        let err = shorten(&kutt(&server.uri()), "https://a.b")
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "Kutt answered 401: API key invalid");
    }

    #[tokio::test]
    async fn wrong_kind_is_refused() {
        let up = kutt("https://kutt.example.com");
        let err = upload_file(&up, Path::new("/nonexistent.png"))
            .await
            .unwrap_err();
        assert!(matches!(err, UploadError::WrongKind { .. }));
    }

    #[tokio::test]
    async fn redirects_to_another_host_are_refused() {
        let elsewhere = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({ "link": "https://x.y/z" })),
            )
            .mount(&elsewhere)
            .await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(307)
                    .insert_header("Location", format!("{}/api/v2/links", elsewhere.uri())),
            )
            .mount(&server)
            .await;
        let err = shorten(&kutt(&server.uri()), "https://a.b")
            .await
            .unwrap_err();
        assert!(matches!(err, UploadError::Network { .. }), "{err}");
        assert!(elsewhere.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn redirects_on_the_same_host_are_followed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/old"))
            .respond_with(ResponseTemplate::new(308).insert_header("Location", "/api/v2/links"))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/links"))
            .and(header("X-API-KEY", "secret"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({ "link": "https://k.example.com/q" })),
            )
            .mount(&server)
            .await;
        let mut up = kutt(&server.uri());
        up.request_url = Template::parse(&format!("{}/old", server.uri())).unwrap();
        assert_eq!(
            shorten(&up, "https://a.b").await.unwrap(),
            "https://k.example.com/q"
        );
    }

    fn response(body: &str) -> Response {
        Response {
            status: 200,
            url: "https://k.example.com/api".into(),
            headers: vec![],
            body: body.into(),
        }
    }

    #[test]
    fn only_web_links_are_accepted() {
        let up = kutt("https://k.example.com");
        for bad in [
            "file:///etc/passwd",
            "steam://run/1",
            "javascript:alert(1)",
            "https://a.b/x y",
        ] {
            let body = serde_json::json!({ "link": bad }).to_string();
            let err = parse_response(&up, None, response(&body)).unwrap_err();
            assert!(matches!(err, UploadError::BadLink { .. }), "{bad}: {err}");
        }
        let body = serde_json::json!({ "link": "HTTPS://k.example.com/q" }).to_string();
        assert!(parse_response(&up, None, response(&body)).is_ok());
    }

    #[test]
    fn bad_extra_links_are_dropped() {
        let mut up = xbackbone("https://xbb.example.com");
        up.deletion_url = Some(Template::parse("file:///x").unwrap());
        let body = serde_json::json!({ "url": "https://xbb.example.com/Ab" }).to_string();
        let got = parse_response(&up, None, response(&body)).unwrap();
        assert_eq!(got.deletion_url, None);
        assert_eq!(
            got.thumbnail_url.as_deref(),
            Some("https://xbb.example.com/Ab/raw")
        );
    }

    #[tokio::test]
    async fn huge_responses_are_cut() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("a".repeat(3 * MAX_RESPONSE)))
            .mount(&server)
            .await;
        let up = kutt(&server.uri());
        let request = build(&up, Input::Text("https://a.b")).unwrap();
        assert_eq!(send(request).await.unwrap().body.len(), MAX_RESPONSE);
    }
}
