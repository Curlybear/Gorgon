//! Everything in Gorgon that doesn't need Qt: settings, taking screenshots
//! through the portal, cropping and saving, uploading.

pub mod annotate;
pub mod capture;
pub mod config;
pub mod naming;
pub mod output;
pub mod sxcu;
pub mod upload;
pub mod uploaders;

/// What the user asked to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Freeze the screen and let the user drag a region.
    Region,
    /// Every monitor.
    Screen,
    /// A window, picked through the desktop's own dialog.
    Window,
}

impl std::str::FromStr for CaptureMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "region" => Ok(Self::Region),
            "screen" => Ok(Self::Screen),
            "window" => Ok(Self::Window),
            other => Err(format!(
                "unknown capture mode {other:?} (expected region, screen or window)"
            )),
        }
    }
}

impl CaptureMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Screen => "screen",
            Self::Window => "window",
        }
    }
}
