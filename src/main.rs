#![windows_subsystem = "windows"]

mod data;
mod island;
mod platform;
mod services;
mod tools;
mod ui;
mod view;

fn main() -> windows_core::Result<()> {
    island::run()
}
