//! The main executable

use std::io::Write;

use cargo_gungraun::args::Color;
use cargo_gungraun::envs;
use colored::{control, Colorize};
use env_logger::Env;
use log::error;

/// The main function of `cargo-gungraun`
///
/// We initialize the logging interface and configure the usage of colors as early as possible
/// here. Then we're printing warnings with [`print_warnings`].
fn main() {
    // Configure the colored crate to respect GUNGRAUN_COLOR and CARGO_TERM_COLOR
    let gungraun_color = std::env::var(envs::GUNGRAUN_COLOR).ok();
    let color = gungraun_color
        .clone()
        .or_else(|| std::env::var(envs::CARGO_TERM_COLOR).ok());
    if let Some(var) = &color {
        if var == "never" {
            control::set_override(false);
        } else if var == "always" {
            control::set_override(true);
        } else {
            // do nothing
        }
    }

    // Configure the env_logger crate to respect GUNGRAUN_COLOR and CARGO_TERM_COLOR
    env_logger::Builder::from_env(
        Env::default()
            .filter_or(envs::GUNGRAUN_LOG, "info")
            .write_style(
                gungraun_color.map_or_else(|| envs::CARGO_TERM_COLOR, |_| envs::GUNGRAUN_COLOR),
            ),
    )
    .format(|buf, record| {
        writeln!(
            buf,
            "{}: {:<5}: {}",
            record
                .module_path()
                .unwrap_or_else(|| record.module_path_static().unwrap_or("???")),
            match record.level() {
                log::Level::Error => "Error".red().bold(),
                log::Level::Warn => "Warn".yellow().bold(),
                log::Level::Info => "Info".green().bold(),
                log::Level::Debug => "Debug".blue().bold(),
                log::Level::Trace => "Trace".cyan().bold(),
            },
            record.args()
        )
    })
    .init();

    if let Err(error) = cargo_gungraun::run(color.and_then(|c| Color::parse(Some(&c)).ok())) {
        error!("{error}");
        std::process::exit(1)
    }
}
