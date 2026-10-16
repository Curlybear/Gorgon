//! After a capture is saved: upload it, shorten the link.

use std::path::Path;

use gorgon_core::{config::Config, upload, uploaders};

/// Links for an uploaded capture.
#[derive(Debug, Clone)]
pub struct Links {
    /// The link to share: shortened when a shortener is set and worked.
    pub url: String,
    pub deletion_url: Option<String>,
    /// Why shortening failed, when it did; the long link is used instead.
    pub shorten_error: Option<String>,
}

/// Uploads and shortens per the settings. `None` when uploading is off or no
/// uploader is set. Blocks; call it off the UI thread.
pub fn upload(path: &Path, config: &Config) -> Option<Result<Links, String>> {
    let name = config.image_uploader.as_deref().filter(|_| config.upload)?;
    Some(upload_with(
        path,
        name,
        config
            .shorten
            .then_some(config.url_shortener.as_deref())
            .flatten(),
    ))
}

/// Uploads with the named uploader, then shortens with the named shortener.
pub fn upload_with(path: &Path, uploader: &str, shortener: Option<&str>) -> Result<Links, String> {
    let uploader = uploaders::find(uploader).map_err(|e| e.to_string())?;
    let shortener = shortener
        .map(uploaders::find)
        .transpose()
        .map_err(|e| e.to_string())?;
    crate::runtime().block_on(async {
        let uploaded = upload::upload_file(&uploader, path)
            .await
            .map_err(|e| e.to_string())?;
        let (url, shorten_error) = match &shortener {
            Some(s) => match upload::shorten(s, &uploaded.url).await {
                Ok(short) => (short, None),
                Err(e) => (uploaded.url.clone(), Some(e.to_string())),
            },
            None => (uploaded.url.clone(), None),
        };
        Ok(Links {
            url,
            deletion_url: uploaded.deletion_url,
            shorten_error,
        })
    })
}
