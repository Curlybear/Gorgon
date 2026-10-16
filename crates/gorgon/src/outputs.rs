//! The real scale of each monitor, read from Wayland.
//!
//! Qt can report a rounded device pixel ratio (2 on a 125% GNOME screen when
//! it doesn't use fractional scaling), so the editor's "100%" would be off.
//! The compositor knows better: an output's mode is its size in screen pixels
//! and xdg-output gives its size in logical pixels, which is what Qt lays out
//! in. Their ratio is the number of screen pixels per Qt pixel.

use std::collections::HashMap;

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_output, wl_registry},
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};

#[derive(Default)]
struct Output {
    name: Option<String>,
    mode: Option<(i32, i32)>,
    rotated: bool,
    logical: Option<(i32, i32)>,
}

#[derive(Default)]
struct State {
    outputs: Vec<Output>,
}

/// Screen pixels per Qt logical pixel, by output name (as `Screen.name` in
/// QML). Empty when this isn't Wayland or the compositor lacks xdg-output.
/// Asked fresh each time, since scales can change while the daemon runs.
pub fn scales() -> HashMap<String, f64> {
    query().unwrap_or_default()
}

/// The scale of the output called `name`, or of the only output.
pub fn scale_of(name: &str) -> Option<f64> {
    let scales = scales();
    scales
        .get(name)
        .copied()
        .or_else(|| (scales.len() == 1).then(|| scales.values().next().copied()).flatten())
}

fn query() -> Option<HashMap<String, f64>> {
    let conn = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
    let qh = queue.handle();
    let manager: zxdg_output_manager_v1::ZxdgOutputManagerV1 = globals.bind(&qh, 2..=3, ()).ok()?;
    let mut state = State::default();
    let wl_outputs: Vec<wl_output::WlOutput> = globals
        .contents()
        .clone_list()
        .into_iter()
        .filter(|g| g.interface == "wl_output")
        .enumerate()
        .map(|(i, g)| {
            state.outputs.push(Output::default());
            globals
                .registry()
                .bind(g.name, g.version.min(4), &qh, i)
        })
        .collect();
    for (i, o) in wl_outputs.iter().enumerate() {
        manager.get_xdg_output(o, &qh, i);
    }
    queue.roundtrip(&mut state).ok()?;
    queue.roundtrip(&mut state).ok()?;

    Some(
        state
            .outputs
            .into_iter()
            .filter_map(|o| {
                let (mut w, mut h) = o.mode?;
                if o.rotated {
                    std::mem::swap(&mut w, &mut h);
                }
                let (lw, _) = o.logical?;
                (lw > 0).then(|| (o.name.unwrap_or_default(), f64::from(w) / f64::from(lw)))
            })
            .collect(),
    )
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, usize> for State {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        &i: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let out = &mut state.outputs[i];
        match event {
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) => out.mode = Some((width, height)),
            wl_output::Event::Geometry {
                transform: WEnum::Value(t),
                ..
            } => {
                use wl_output::Transform::*;
                out.rotated = matches!(t, _90 | _270 | Flipped90 | Flipped270);
            }
            wl_output::Event::Name { name } if out.name.is_none() => out.name = Some(name),
            _ => {}
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, usize> for State {
    fn event(
        state: &mut Self,
        _: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        &i: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let out = &mut state.outputs[i];
        match event {
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                out.logical = Some((width, height))
            }
            zxdg_output_v1::Event::Name { name } => out.name = Some(name),
            _ => {}
        }
    }
}

impl Dispatch<zxdg_output_manager_v1::ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &zxdg_output_manager_v1::ZxdgOutputManagerV1,
        _: zxdg_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
