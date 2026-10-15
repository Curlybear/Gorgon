//! `gorgon capture ...`: asks the daemon to capture, starting it if needed.

use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use gorgon_core::{capture::runtime_dir, CaptureMode};
use zbus::Connection;

use crate::dbus;

pub async fn capture(mode: CaptureMode) -> Result<()> {
    let conn = Connection::session()
        .await
        .context("could not reach the session bus")?;
    if !dbus::is_running(&conn).await? {
        start_daemon()?;
        wait_for_daemon(&conn).await?;
    }
    conn.call_method(
        Some(dbus::NAME),
        dbus::PATH,
        Some(dbus::INTERFACE),
        "Capture",
        &(mode.as_str(),),
    )
    .await
    .context("the Gorgon daemon refused the capture")?;
    Ok(())
}

/// Starts `gorgon daemon` in the background, logging to the runtime folder.
fn start_daemon() -> Result<()> {
    let exe = std::env::current_exe().context("could not find the gorgon executable")?;
    let log_path = runtime_dir()?.join("daemon.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("could not open {}", log_path.display()))?;
    Command::new(exe)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("could not start the Gorgon daemon")?;
    Ok(())
}

async fn wait_for_daemon(conn: &Connection) -> Result<()> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if dbus::is_running(conn).await? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!(
        "the Gorgon daemon didn't start; see {}",
        runtime_dir()?.join("daemon.log").display()
    )
}
