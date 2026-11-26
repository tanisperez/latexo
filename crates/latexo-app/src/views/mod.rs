//! The gpui views. Kept thin: state and logic live in `crate::state`.

mod about;
mod alerts;
mod chart;
mod charts;
mod cores;
mod gpu;
mod logo;
mod overview;
mod pill;
mod processes;
mod resources;
mod root;
mod settings;
mod temperatures;
mod tray;
mod update;
mod widgets;

pub use root::AppView;
