//! A stand-in for `org.freedesktop.portal.Screenshot`, so Gorgon can be run
//! headless (CI, containers). Every screenshot is a copy of the PNG given as
//! the first argument.
//!
//! ```sh
//! dbus-run-session -- sh -c 'fake-portal shot.png & gorgon daemon'
//! ```

use std::{collections::HashMap, path::PathBuf, time::Duration};

use zbus::{
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedValue, Value},
    Connection,
};

struct Screenshot {
    source: PathBuf,
}

struct Request;

#[interface(name = "org.freedesktop.portal.Request")]
impl Request {
    #[zbus(signal)]
    async fn response(
        emitter: &SignalEmitter<'_>,
        response: u32,
        results: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<()>;
}

#[interface(name = "org.freedesktop.portal.Screenshot")]
impl Screenshot {
    async fn screenshot(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        _parent_window: &str,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<ObjectPath<'static>> {
        let sender = header
            .sender()
            .map(|s| s.trim_start_matches(':').replace('.', "_"))
            .unwrap_or_default();
        let token = options
            .get("handle_token")
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_else(|| "t".into());
        let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        let path =
            ObjectPath::try_from(path).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        // Like a real portal, write a fresh file the caller then owns.
        let out = std::env::temp_dir().join(format!("fake-portal-{token}.png"));
        std::fs::copy(&self.source, &out).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        conn.object_server().at(&path, Request).await?;
        let conn = conn.clone();
        let reply_path = path.clone().into_owned();
        // zbus runs handlers on its own executor, so reply from a separate thread.
        let runtime = tokio::runtime::Handle::try_current();
        let reply = async move {
            // Give the caller time to subscribe to the Response signal.
            tokio::time::sleep(Duration::from_millis(50)).await;
            let uri = format!("file://{}", out.display());
            let results = HashMap::from([("uri", Value::from(uri.as_str()))]);
            if let Ok(emitter) = SignalEmitter::new(&conn, reply_path.clone()) {
                let _ = Request::response(&emitter, 0, results).await;
            }
            let _ = conn.object_server().remove::<Request, _>(&reply_path).await;
        };
        match runtime {
            Ok(handle) => {
                handle.spawn(reply);
            }
            Err(_) => {
                std::thread::spawn(move || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .expect("runtime")
                        .block_on(reply)
                });
            }
        }
        Ok(path.into_owned())
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        2
    }
}

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let source = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: fake-portal <image.png>");
    let _conn = zbus::connection::Builder::session()?
        .name("org.freedesktop.portal.Desktop")?
        .serve_at("/org/freedesktop/portal/desktop", Screenshot { source })?
        .build()
        .await?;
    println!("fake portal ready");
    std::future::pending::<()>().await;
    Ok(())
}
