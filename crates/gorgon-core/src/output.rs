//! Cropping and saving captures.

use std::path::Path;

use image::{ImageFormat, RgbaImage};

/// A rectangle in image pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Loads a capture, optionally cropped to `rect` (clamped to the image).
pub fn load(path: &Path, rect: Option<Rect>) -> image::ImageResult<RgbaImage> {
    let image = image::open(path)?.into_rgba8();
    let Some(rect) = rect else {
        return Ok(image);
    };
    let (w, h) = image.dimensions();
    let x = (rect.x.round().max(0.0) as u32).min(w.saturating_sub(1));
    let y = (rect.y.round().max(0.0) as u32).min(h.saturating_sub(1));
    let cw = (rect.width.round().max(1.0) as u32).min(w - x);
    let ch = (rect.height.round().max(1.0) as u32).min(h - y);
    Ok(image::imageops::crop_imm(&image, x, y, cw, ch).to_image())
}

/// Saves as PNG, creating the folder if needed.
pub fn save_png(image: &RgbaImage, path: &Path) -> image::ImageResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save_with_format(path, ImageFormat::Png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("in.png");
        RgbaImage::from_fn(100, 50, |x, y| image::Rgba([x as u8, y as u8, 0, 255]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn crops_to_rect() {
        let dir = tempfile::tempdir().unwrap();
        let rect = Rect {
            x: 10.0,
            y: 5.0,
            width: 20.0,
            height: 10.0,
        };
        let img = load(&sample(dir.path()), Some(rect)).unwrap();
        assert_eq!(img.dimensions(), (20, 10));
        assert_eq!(img.get_pixel(0, 0).0, [10, 5, 0, 255]);
    }

    #[test]
    fn clamps_rect_to_image() {
        let dir = tempfile::tempdir().unwrap();
        let rect = Rect {
            x: 90.0,
            y: 40.0,
            width: 50.0,
            height: 50.0,
        };
        let img = load(&sample(dir.path()), Some(rect)).unwrap();
        assert_eq!(img.dimensions(), (10, 10));
    }

    #[test]
    fn saves_into_new_folder() {
        let dir = tempfile::tempdir().unwrap();
        let img = load(&sample(dir.path()), None).unwrap();
        let out = dir.path().join("a/b/out.png");
        save_png(&img, &out).unwrap();
        assert_eq!(image::image_dimensions(&out).unwrap(), (100, 50));
    }
}
