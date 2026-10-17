//! `gorgon upload`, `gorgon shorten` and `gorgon uploader ...`. These run in
//! the command itself; no daemon needed.

use std::path::Path;

use anyhow::{bail, Context, Result};
use gorgon_core::{config, config::Config, history, sxcu::Uploader, upload, uploaders};

pub fn upload(file: &Path, with: Option<&str>, no_shorten: bool) -> Result<()> {
    let config = Config::load()?;
    let name = with
        .or(config.image_uploader.as_deref())
        .context("no uploader set: import one with `gorgon uploader import <file.sxcu>`")?;
    let shortener = if no_shorten || !config.shorten {
        None
    } else {
        config.url_shortener.as_deref()
    };
    let (w, h) = image::image_dimensions(file).unwrap_or((0, 0));
    let path = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_owned());
    let id = history::add(history::Entry::new(&path, w, h)).ok();
    let result = crate::pipeline::upload_with(file, name, shortener);
    if let Some(id) = id {
        let _ = history::update(id, |e| match &result {
            Ok(links) => {
                e.url = Some(links.url.clone());
                e.deletion_url = links.deletion_url.clone();
                e.uploader = Some(name.to_owned());
            }
            Err(err) => e.error = Some(err.clone()),
        });
    }
    let links = result.map_err(anyhow::Error::msg)?;
    if let Some(e) = &links.shorten_error {
        eprintln!("gorgon: shortening failed, keeping the long link: {e}");
    }
    println!("{}", links.url);
    if let Some(d) = &links.deletion_url {
        eprintln!("deletion link: {d}");
    }
    Ok(())
}

pub fn shorten(url: &str, with: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let name = with
        .or(config.url_shortener.as_deref())
        .context("no URL shortener set: import one with `gorgon uploader import <file.sxcu>`")?;
    let shortener = uploaders::find(name)?;
    let short = crate::runtime().block_on(upload::shorten(&shortener, url))?;
    println!("{short}");
    Ok(())
}

fn kind(u: &Uploader) -> &'static str {
    match (u.uploads_files(), u.shortens_urls()) {
        (true, true) => "uploader and URL shortener",
        (true, false) => "uploader",
        (false, true) => "URL shortener",
        (false, false) => "unusable (neither uploads files nor shortens URLs)",
    }
}

pub fn import(file: &Path) -> Result<()> {
    let (uploader, stored) = uploaders::import(file)?;
    println!(
        "Imported {:?} ({}) to {}",
        uploader.name,
        kind(&uploader),
        stored.display()
    );
    let config = Config::load()?;
    if uploader.uploads_files() && config.image_uploader.is_none() {
        let path = config::set_value("image_uploader", uploader.name.as_str())?;
        println!(
            "Captures now upload with it (image_uploader in {}).",
            path.display()
        );
    }
    if uploader.shortens_urls() && config.url_shortener.is_none() {
        let path = config::set_value("url_shortener", uploader.name.as_str())?;
        println!(
            "Links are now shortened with it (url_shortener in {}).",
            path.display()
        );
    }
    if !uploader.uploads_files() && !uploader.shortens_urls() {
        bail!("Gorgon can't use this uploader for captures or links");
    }
    Ok(())
}

pub fn list() -> Result<()> {
    let config = Config::load()?;
    let (ok, bad) = uploaders::list()?;
    if ok.is_empty() && bad.is_empty() {
        println!("No uploaders yet. Import one with `gorgon uploader import <file.sxcu>`.");
    }
    for u in &ok {
        let mut marks = vec![];
        if config
            .image_uploader
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(&u.name))
        {
            marks.push("captures");
        }
        if config
            .url_shortener
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(&u.name))
        {
            marks.push("links");
        }
        let marks = if marks.is_empty() {
            String::new()
        } else {
            format!("  [default for {}]", marks.join(" and "))
        };
        println!("{}: {}{marks}", u.name, kind(u));
    }
    for e in &bad {
        eprintln!("skipped: {e}");
    }
    Ok(())
}

pub fn raw_links(name: &str) -> Result<()> {
    let path = uploaders::make_raw_links(name).map_err(anyhow::Error::msg)?;
    println!(
        "{name:?} now links straight to the file ({}).",
        path.display()
    );
    Ok(())
}

pub fn set_default(name: &str) -> Result<()> {
    let uploader = uploaders::find(name)?;
    if uploader.uploads_files() {
        config::set_value("image_uploader", uploader.name.as_str())?;
        println!("Captures now upload with {:?}.", uploader.name);
    }
    if uploader.shortens_urls() {
        config::set_value("url_shortener", uploader.name.as_str())?;
        println!("Links are now shortened with {:?}.", uploader.name);
    }
    Ok(())
}
