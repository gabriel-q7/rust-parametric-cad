//! Desktop entry point for the parametric CAD editor.

mod app;
mod commands;
mod editor;
mod plugins;
mod ui;
mod viewport;

use bevy::app::AppExit;

fn main() -> AppExit {
    app::build_app().run()
}
