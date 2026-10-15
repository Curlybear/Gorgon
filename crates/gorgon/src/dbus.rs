//! The daemon's D-Bus service, which the CLI talks to.

use gorgon_core::CaptureMode;
use zbus::{fdo, interface, Connection};

pub const NAME: &str = "dev.gorgon.Gorgon";
pub const PATH: &str = "/dev/gorgon/Gorgon";
pub const INTERFACE: &str = "dev.gorgon.Gorgon1";

struct Service;

#[interface(name = "dev.gorgon.Gorgon1")]
impl Service {
    /// Starts a capture: "region", "screen" or "window".
    fn capture(&self, mode: &str) -> fdo::Result<()> {
        let mode: CaptureMode = mode.parse().map_err(fdo::Error::InvalidArgs)?;
        crate::app::request_capture(mode).map_err(fdo::Error::Failed)
    }
}

/// Whether a Gorgon daemon already owns the bus name.
pub async fn is_running(conn: &Connection) -> zbus::Result<bool> {
    let dbus = fdo::DBusProxy::new(conn).await?;
    Ok(dbus.name_has_owner(NAME.try_into()?).await?)
}

/// Publishes the service. Keep the returned connection alive.
pub async fn serve() -> zbus::Result<Connection> {
    zbus::connection::Builder::session()?
        .serve_at(PATH, Service)?
        .name(NAME)?
        .build()
        .await
}
