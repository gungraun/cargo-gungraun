//! The command line arguments

use core::convert::AsRef;
use core::fmt::Display;
use std::ffi::{OsStr, OsString};
use std::io::stdout;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use colored::Colorize;

use crate::{cargo_bin, envs, Target};

// TODO: List, Test, Version, HelpAll (to print help of gungraun)
#[derive(Debug, Default)]
pub enum Command {
    /// Benchmarks
    #[default]
    Bench,
    Help,
    Version,
}

impl Command {
    fn parse(value: &OsStr) -> Result<Self> {
        match value.to_str() {
            Some("bench") => Ok(Self::Bench),
            Some("help") => Ok(Self::Help),
            Some(unknown) => Err(anyhow!("Unexpected command: {unknown}")),
            None => Err(anyhow!("Invalid arg: '{value:?}'")),
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub cargo: Vec<OsString>,
    pub color: Color,
    pub command: Command,
    pub help: bool,
    pub target: Option<Target>,
    pub target_dir: Option<Utf8PathBuf>,
}

impl Args {
    pub fn print_command_help(&self) {
        println!("This is the command help");
        // TODO: implement
    }

    /// TODO: DOCS
    ///
    /// # Errors
    pub fn print_bench_help(&self) -> Result<()> {
        colored::control::set_override(true);

        let message = format!(
            "A thin wrapper around cargo bench to run gungraun benchmarks on targets via \
             podman/docker

{} {}

cargo-gungraun uses the same CARGO_BENCH_ARGS as cargo bench. If you only run gungraun benchmarks,
then ARGS can be any gungraun arguments. Run `{}` to see all valid gungraun
arguments (Requires gungraun >= 0.18).

...Dispatching to `{}`:
",
            "Usage:".blue().bold(),
            "cargo gungraun bench [CARGO_BENCH_ARGS] [-- [ARGS]...]".bright_blue(),
            "cargo gungraun help-all".blue(),
            "cargo bench --help".blue()
        );

        println!("{message}");
        let output = std::process::Command::new(cargo_bin())
            .env("CARGO_TERM_COLOR", "always")
            .args(["bench", "--help"])
            .output()?;

        if output.status.success() {
            std::io::copy(&mut output.stdout.as_slice(), &mut stdout())?;
        }

        Ok(())
    }

    pub fn print_version() {
        let version = env!("CARGO_PKG_VERSION");
        println!("cargo-gungraun {version}");
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Color {
    Always,
    Auto,
    Never,
}

impl Color {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn parse<T>(value: Option<&T>) -> Result<Self>
    where
        T: AsRef<OsStr>,
    {
        let value = value.map(AsRef::as_ref);

        let s = value.map(|s| s.to_str().ok_or(s));
        match s {
            Some(Ok("always" | "")) => Ok(Self::Always),
            Some(Ok("auto")) | None => Ok(Self::Auto),
            Some(Ok("never")) => Ok(Self::Never),
            Some(invalid) => Err(anyhow!(
                "Invalid color value: '{invalid:?}'. Possible values are always, never, auto"
            )),
        }
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Always => write!(f, "always"),
            Self::Auto => write!(f, "auto"),
            Self::Never => write!(f, "never"),
        }
    }
}

/// TODO:
///
/// # Errors
pub fn parse(color: Option<Color>) -> Result<Args> {
    let raw = std::env::args_os().skip(1);
    let target = match std::env::var(envs::CARGO_BUILD_TARGET) {
        Ok(value) => Target::parse(Some(&value)),
        Err(_) => Ok(None),
    }?;

    let target_dir = std::env::var_os(envs::CARGO_TARGET_DIR)
        .or_else(|| std::env::var_os(envs::CARGO_BUILD_TARGET_DIR))
        .map(|value| {
            Utf8PathBuf::try_from(value).with_context(|| {
                format!(
                    "Invalid '{}' or '{}'",
                    envs::CARGO_TARGET_DIR,
                    envs::CARGO_BUILD_TARGET_DIR
                )
            })
        })
        .transpose()?;

    let mut args = Args {
        color: color.unwrap_or(Color::Auto),
        target,
        target_dir,
        cargo: vec![],
        command: Command::default(),
        help: false,
    };

    let mut is_command = true;

    let raw = clap_lex::RawArgs::new(raw);
    let mut cursor = raw.cursor();
    raw.next(&mut cursor); // Skip the bin
    while let Some(arg) = raw.next(&mut cursor) {
        args.cargo.push(arg.to_value_os().to_os_string());

        if arg.is_escape() {
            args.cargo
                .extend(raw.remaining(&mut cursor).map(OsStr::to_os_string));
        } else if let Some((long, value)) = arg.to_long() {
            match long {
                Ok("color") => {
                    args.color = Color::parse(value.as_ref())?;
                }
                Ok(flag @ "target") => {
                    let value = value.ok_or_else(|| anyhow!("A value is required for --{flag}"))?;
                    args.target = Target::parse(Some(&value))?;
                }
                Ok(flag @ "target-dir") => {
                    let value = value.ok_or_else(|| anyhow!("A value is required for --{flag}"))?;
                    let path = Utf8PathBuf::try_from(value.to_os_string())
                        .with_context(|| format!("Invalid --{flag}"))?;
                    args.target_dir = Some(path);
                }
                Ok("help") => {
                    args.help = true;
                    return Ok(args);
                }
                _ => {}
            }
        } else if let Some(mut shorts) = arg.to_short() {
            while let Some(short) = shorts.next_flag() {
                match short {
                    Ok('c') => {
                        let value = shorts.next_value_os();
                        args.color = Color::parse(value.as_ref())?;
                    }
                    Ok('h') => {
                        args.help = true;
                        return Ok(args);
                    }
                    _ => {}
                }
            }
        } else if is_command {
            args.command = Command::parse(arg.to_value_os())?;
            args.cargo.pop();
        } else {
            // do nothing
        }

        is_command = false;
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::x86_64("x86_64-unknown-linux-gnu", Target::X86_64_Unknown_Linux_Gnu)]
    #[case::i686("i686-unknown-linux-gnu", Target::I686_Unknown_Linux_Gnu)]
    #[case::powerpc("powerpc-unknown-linux-gnu", Target::Powerpc_Unknown_Linux_Gnu)]
    #[case::powerpc64("powerpc64-unknown-linux-gnu", Target::Powerpc64_Unknown_Linux_Gnu)]
    #[case::powerpc64le("powerpc64le-unknown-linux-gnu", Target::Powerpc64le_Unknown_Linux_Gnu)]
    #[case::s390x("s390x-unknown-linux-gnu", Target::S390x_Unknown_Linux_Gnu)]
    #[case::aarch64("aarch64-unknown-linux-gnu", Target::Aarch64_Unknown_Linux_Gnu)]
    #[case::armeabi("arm-unknown-linux-gnueabi", Target::Arm_Unknown_Linux_Gnueabi)]
    #[case::armeabihf("arm-unknown-linux-gnueabihf", Target::Arm_Unknown_Linux_Gnueabihf)]
    #[case::armv7eabi("armv7-unknown-linux-gnueabi", Target::Armv7_Unknown_Linux_Gnueabi)]
    #[case::armv7eabihf("armv7-unknown-linux-gnueabihf", Target::Armv7_Unknown_Linux_Gnueabihf)]
    #[case::mips("mips-unknown-linux-gnu", Target::Mips_Unknown_Linux_Gnu)]
    #[case::mipsel("mipsel-unknown-linux-gnu", Target::Mipsel_Unknown_Linux_Gnu)]
    #[case::mips64el(
        "mips64el-unknown-linux-gnuabi64",
        Target::Mips64el_Unknown_Linux_Gnuabi64
    )]
    #[case::risc64gc("riscv64gc-unknown-linux-gnu", Target::Riscv64gc_Unknown_Linux_Gnu)]
    fn target_parse_when_valid(#[case] from: &str, #[case] expected: Target) {
        assert_eq!(Target::parse(Some(&from)).unwrap(), Some(expected));
    }

    #[test]
    fn target_parse_when_invalid() {
        assert_eq!(
            Target::parse(Some(&"invalid")).unwrap_err().to_string(),
            "Unsupported or invalid target: 'invalid'".to_owned()
        );
    }
}
