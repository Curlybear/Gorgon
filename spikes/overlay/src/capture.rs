//! Takes the portal screenshot and saves crops of it.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QUrl, image_url, cxx_name = "imageUrl")]
        #[qproperty(i32, image_width, cxx_name = "imageWidth")]
        #[qproperty(i32, image_height, cxx_name = "imageHeight")]
        #[qproperty(QString, status)]
        type Capture = super::CaptureRust;

        /// Emitted once a screenshot is ready to show.
        #[qsignal]
        fn captured(self: Pin<&mut Capture>);

        /// Takes a non-interactive portal screenshot. Blocks until the portal answers.
        #[qinvokable]
        fn capture(self: Pin<&mut Capture>);

        /// Crops the screenshot (in image pixels) and saves it. Returns the path.
        #[qinvokable]
        #[cxx_name = "saveCrop"]
        fn save_crop(self: Pin<&mut Capture>, x: f64, y: f64, w: f64, h: f64) -> QString;

        /// Prints what Qt reports for a screen, to compare with the image size.
        #[qinvokable]
        #[cxx_name = "reportScreen"]
        fn report_screen(
            &self,
            name: &QString,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            device_pixel_ratio: f64,
        );
    }
}

use std::{path::PathBuf, pin::Pin, time::Instant};

use ashpd::desktop::screenshot::Screenshot;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};

#[derive(Default)]
pub struct CaptureRust {
    image_url: QUrl,
    image_width: i32,
    image_height: i32,
    status: QString,
    path: Option<PathBuf>,
}

impl qobject::Capture {
    pub fn capture(mut self: Pin<&mut Self>) {
        let start = Instant::now();
        match take_screenshot() {
            Ok(path) => {
                let (w, h) = match image::image_dimensions(&path) {
                    Ok(dims) => dims,
                    Err(e) => {
                        self.set_status(QString::from(&format!("unreadable screenshot: {e}")));
                        return;
                    }
                };
                println!(
                    "screenshot: {w}x{h} in {} ms ({})",
                    start.elapsed().as_millis(),
                    path.display()
                );
                self.as_mut().set_image_width(w as i32);
                self.as_mut().set_image_height(h as i32);
                self.as_mut()
                    .set_image_url(QUrl::from(&format!("file://{}", path.display())));
                self.as_mut().rust_mut().path = Some(path);
                self.captured();
            }
            Err(e) => {
                eprintln!("screenshot failed: {e}");
                self.set_status(QString::from(&format!("screenshot failed: {e}")));
            }
        }
    }

    pub fn save_crop(self: Pin<&mut Self>, x: f64, y: f64, w: f64, h: f64) -> QString {
        let Some(src) = self.rust().path.clone() else {
            return QString::default();
        };
        let result = image::open(&src)
            .map_err(|e| e.to_string())
            .and_then(|img| {
                let crop = img.crop_imm(
                    x.max(0.0).round() as u32,
                    y.max(0.0).round() as u32,
                    w.round() as u32,
                    h.round() as u32,
                );
                let out = std::env::temp_dir().join(format!(
                    "gorgon-spike-{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ));
                crop.save(&out).map_err(|e| e.to_string())?;
                Ok((out, crop.width(), crop.height()))
            });
        match result {
            Ok((out, cw, ch)) => {
                println!("saved {cw}x{ch} crop to {}", out.display());
                QString::from(&out.display().to_string())
            }
            Err(e) => {
                eprintln!("crop failed: {e}");
                QString::default()
            }
        }
    }

    pub fn report_screen(
        &self,
        name: &QString,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        device_pixel_ratio: f64,
    ) {
        println!(
            "screen {name}: logical {w}x{h} at {x},{y}, device pixel ratio {device_pixel_ratio}"
        );
    }
}

fn take_screenshot() -> Result<PathBuf, String> {
    // Lets the overlay be tested without a portal.
    if let Ok(path) = std::env::var("GORGON_SPIKE_IMAGE") {
        return Ok(PathBuf::from(path));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let shot = rt
        .block_on(async {
            Screenshot::request()
                .interactive(false)
                .modal(false)
                .send()
                .await?
                .response()
        })
        .map_err(|e| e.to_string())?;
    let uri = shot.uri().to_string();
    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("unexpected uri {uri}"))?
        .replace("%20", " ");
    Ok(PathBuf::from(path))
}
