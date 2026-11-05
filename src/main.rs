#![windows_subsystem = "windows"]

mod data;
mod island;
mod platform;
mod services;
mod tools;
mod ui;
mod view;

fn main() -> windows_core::Result<()> {
    if platform::taskbar::handle_cli() || services::agents::handle_cli() {
        return Ok(());
    }
    island::run()
}
