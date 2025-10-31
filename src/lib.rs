//! The library

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
// TODO: REMOVE
#![allow(missing_docs)]

pub mod args;
pub mod container;
pub mod error;
pub mod meta;

/// Names of environment variables which are used repeatedly in different places
pub mod envs {
    /// Location of where to place all generated artifacts
    pub const CARGO_BUILD_TARGET: &str = "CARGO_BUILD_TARGET";
    /// Location of where to place all generated artifacts
    pub const CARGO_BUILD_TARGET_DIR: &str = "CARGO_BUILD_TARGET_DIR";
    /// The name of the package
    pub const CARGO_GUNGRAUN_ENGINE: &str = "CARGO_GUNGRAUN_ENGINE";
    /// The name of the package
    pub const CARGO_GUNGRAUN_ENVS: &str = "CARGO_GUNGRAUN_ENVS";
    /// The name of the package
    pub const CARGO_GUNGRAUN_IMAGE: &str = "CARGO_GUNGRAUN_IMAGE";
    /// TODO: DOCS
    pub const CARGO_GUNGRAUN_QEMU_ACCELERATOR: &str = "CARGO_GUNGRAUN_QEMU_ACCELERATOR";
    /// TODO: DOCS
    pub const CARGO_GUNGRAUN_QEMU_EXTRA_ARGS: &str = "CARGO_GUNGRAUN_QEMU_EXTRA_ARGS";
    /// TODO: DOCS
    pub const CARGO_GUNGRAUN_QEMU_TIMEOUT: &str = "CARGO_GUNGRAUN_QEMU_TIMEOUT";
    /// TODO: DOCS
    pub const CARGO_GUNGRAUN_TARGET: &str = "CARGO_GUNGRAUN_TARGET";
    /// TODO: DOCS
    pub const CARGO_GUNGRAUN_VOLUMES: &str = "CARGO_GUNGRAUN_VOLUMES";
    /// The name of the package
    pub const CARGO_HOME: &str = "CARGO_HOME";
    /// Location of where to place all generated artifacts
    pub const CARGO_TARGET_DIR: &str = "CARGO_TARGET_DIR";
    /// The default color mode
    pub const CARGO_TERM_COLOR: &str = "CARGO_TERM_COLOR";

    /// The environment variable to set the color (same syntax as `CARGO_TERM_COLOR`)
    pub const GUNGRAUN_COLOR: &str = "GUNGRAUN_COLOR";
    /// TODO: DOCS
    pub const GUNGRAUN_EXECUTOR: &str = "GUNGRAUN_EXECUTOR";
    /// TODO: DOCS
    pub const GUNGRAUN_EXECUTOR_ARGS: &str = "GUNGRAUN_EXECUTOR_ARGS";
    /// Set the logging output of Gungraun
    pub const GUNGRAUN_LOG: &str = "GUNGRAUN_LOG";
    /// Set the logging output of Gungraun
    pub const GUNGRAUN_HOME: &str = "GUNGRAUN_HOME";
    /// Set the runner
    pub const GUNGRAUN_RUNNER: &str = "GUNGRAUN_RUNNER";
    /// Separate targets
    pub const GUNGRAUN_SEPARATE_TARGETS: &str = "GUNGRAUN_SEPARATE_TARGETS";
    /// The gungraun version used in the manifest of the target package
    pub const GUNGRAUN_VERSION: &str = "GUNGRAUN_VERSION";
    /// TODO: DOCS
    pub const QEMU_LD_PREFIX: &str = "QEMU_LD_PREFIX";
    /// The rustup home
    pub const RUSTUP_HOME: &str = "RUSTUP_HOME";
}

use core::fmt::Display;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use log::info;

use crate::args::{Args, Color};
use crate::error::Error;

/// All currently supported targets in rustc target triple format
///
/// Targets supported by valgrind but currently not by us:
///
/// X86/FreeBSD, AMD64/FreeBSD: supported since FreeBSD 11.3.
/// ARM64/FreeBSD: supported since FreeBSD 14.
/// X86/Solaris, AMD64/Solaris, X86/Illumos, AMD64/Illumos: supported since Solaris 11.
/// X86/Darwin (10.5 to 10.13), AMD64/Darwin (10.5 to 10.13): supported.
/// ARM/Android, ARM64/Android, MIPS32/Android, X86/Android: supported.
///
/// A list of all supported valgrind targets <https://valgrind.org/info/platforms.html>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Target {
    Aarch64_Unknown_Linux_Gnu,
    Arm_Unknown_Linux_Gnueabi,
    Arm_Unknown_Linux_Gnueabihf,
    Armv7_Unknown_Linux_Gnueabi,
    Armv7_Unknown_Linux_Gnueabihf,
    I686_Unknown_Linux_Gnu,
    Mips64el_Unknown_Linux_Gnuabi64,
    Mips_Unknown_Linux_Gnu,
    Mipsel_Unknown_Linux_Gnu,
    Powerpc64_Unknown_Linux_Gnu,
    Powerpc64le_Unknown_Linux_Gnu,
    Powerpc_Unknown_Linux_Gnu,
    Riscv64gc_Unknown_Linux_Gnu,
    S390x_Unknown_Linux_Gnu,
    X86_64_Unknown_Linux_Gnu,
}

impl Target {
    fn parse<T>(value: Option<&T>) -> Result<Option<Self>>
    where
        T: AsRef<OsStr>,
    {
        let value = value.as_ref().map(AsRef::as_ref);

        let target = match value {
            Some(v) if v == "x86_64-unknown-linux-gnu" => Self::X86_64_Unknown_Linux_Gnu,
            Some(v) if v == "i686-unknown-linux-gnu" => Self::I686_Unknown_Linux_Gnu,
            Some(v) if v == "powerpc-unknown-linux-gnu" => Self::Powerpc_Unknown_Linux_Gnu,
            Some(v) if v == "powerpc64-unknown-linux-gnu" => Self::Powerpc64_Unknown_Linux_Gnu,
            Some(v) if v == "powerpc64le-unknown-linux-gnu" => Self::Powerpc64le_Unknown_Linux_Gnu,
            Some(v) if v == "s390x-unknown-linux-gnu" => Self::S390x_Unknown_Linux_Gnu,
            Some(v) if v == "aarch64-unknown-linux-gnu" => Self::Aarch64_Unknown_Linux_Gnu,
            Some(v) if v == "arm-unknown-linux-gnueabi" => Self::Arm_Unknown_Linux_Gnueabi,
            Some(v) if v == "arm-unknown-linux-gnueabihf" => Self::Arm_Unknown_Linux_Gnueabihf,
            Some(v) if v == "armv7-unknown-linux-gnueabi" => Self::Armv7_Unknown_Linux_Gnueabi,
            Some(v) if v == "armv7-unknown-linux-gnueabihf" => Self::Armv7_Unknown_Linux_Gnueabihf,
            Some(v) if v == "mips-unknown-linux-gnu" => Self::Mips_Unknown_Linux_Gnu,
            Some(v) if v == "mipsel-unknown-linux-gnu" => Self::Mipsel_Unknown_Linux_Gnu,
            Some(v) if v == "mips64el-unknown-linux-gnuabi64" => {
                Self::Mips64el_Unknown_Linux_Gnuabi64
            }
            Some(v) if v == "riscv64gc-unknown-linux-gnu" => Self::Riscv64gc_Unknown_Linux_Gnu,
            Some(v) => {
                return Err(anyhow!(
                    "Unsupported or invalid target: '{}'",
                    v.to_string_lossy()
                ))
            }
            None => return Ok(None),
        };

        Ok(Some(target))
    }

    #[must_use]
    pub fn to_upper_env(&self) -> String {
        self.to_string().to_uppercase().replace('-', "_")
    }

    #[must_use]
    pub fn to_lower_env(&self) -> String {
        self.to_string().replace('-', "_")
    }

    #[must_use]
    pub fn to_gnu_triple(&self) -> String {
        let toolchain = match self {
            Self::X86_64_Unknown_Linux_Gnu => "x86-64-linux-gnu",
            Self::I686_Unknown_Linux_Gnu => "i686-linux-gnu",
            Self::Powerpc_Unknown_Linux_Gnu => "powerpc-linux-gnu",
            Self::Powerpc64_Unknown_Linux_Gnu => "powerpc64-linux-gnu",
            Self::Powerpc64le_Unknown_Linux_Gnu => "powerpc64le-linux-gnu",
            Self::S390x_Unknown_Linux_Gnu => "s390x-linux-gnu",
            Self::Aarch64_Unknown_Linux_Gnu => "aarch64-linux-gnu",
            Self::Arm_Unknown_Linux_Gnueabi | Self::Armv7_Unknown_Linux_Gnueabi => {
                "arm-linux-gnueabi"
            }
            Self::Arm_Unknown_Linux_Gnueabihf | Self::Armv7_Unknown_Linux_Gnueabihf => {
                "arm-linux-gnueabihf"
            }
            Self::Mips_Unknown_Linux_Gnu => "mips-linux-gnu",
            Self::Mipsel_Unknown_Linux_Gnu => "mipsel-linux-ngu",
            Self::Mips64el_Unknown_Linux_Gnuabi64 => "mips64el-linux-gnu",
            Self::Riscv64gc_Unknown_Linux_Gnu => "riscv64-linux-gnu",
        };
        toolchain.to_owned()
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let string = match self {
            Self::X86_64_Unknown_Linux_Gnu => "x86_64-unknown-linux-gnu",
            Self::I686_Unknown_Linux_Gnu => "i686-unknown-linux-gnu",
            Self::Powerpc_Unknown_Linux_Gnu => "powerpc-unknown-linux-gnu",
            Self::Powerpc64_Unknown_Linux_Gnu => "powerpc64-unknown-linux-gnu",
            Self::Powerpc64le_Unknown_Linux_Gnu => "powerpc64le-unknown-linux-gnu",
            Self::S390x_Unknown_Linux_Gnu => "s390x-unknown-linux-gnu",
            Self::Aarch64_Unknown_Linux_Gnu => "aarch64-unknown-linux-gnu",
            Self::Arm_Unknown_Linux_Gnueabi => "arm-unknown-linux-gnueabi",
            Self::Arm_Unknown_Linux_Gnueabihf => "arm-unknown-linux-gnueabihf",
            Self::Armv7_Unknown_Linux_Gnueabi => "armv7-unknown-linux-gnueabi",
            Self::Armv7_Unknown_Linux_Gnueabihf => "armv7-unknown-linux-gnueabihf",
            Self::Mips_Unknown_Linux_Gnu => "mips-unknown-linux-gnu",
            Self::Mipsel_Unknown_Linux_Gnu => "mipsel-unknown-linux-gnu",
            Self::Mips64el_Unknown_Linux_Gnuabi64 => "mips64el-unknown-linux-gnuabi64",
            Self::Riscv64gc_Unknown_Linux_Gnu => "riscv64gc-unknown-linux-gnu",
        };

        write!(f, "{string}")
    }
}

#[must_use]
pub fn cargo_bin() -> PathBuf {
    std::env::var_os("CARGO")
        .unwrap_or_else(|| OsString::from("cargo"))
        .into()
}

/// TODO: Result error
///
/// # Errors
pub fn run(color: Option<Color>) -> Result<()> {
    let args = args::parse(color)?;
    match args.command {
        args::Command::Bench if args.help => {
            args.print_bench_help()?;
        }
        args::Command::Bench => {
            if let Some(target) = args.target {
                container::run_bench(target, args.cargo)?;
            } else {
                info!("No target given. Falling back to run `cargo bench` on the host");
                return std::process::Command::new(cargo_bin())
                    .args(args.cargo)
                    .status()
                    .map_err(Error::CommandSpawn)
                    .and_then(|status| {
                        if status.success() {
                            Ok(())
                        } else {
                            Err(Error::Command(status))
                        }
                    })
                    .with_context(|| "Failed to execute cargo");
            }
        }
        args::Command::Help => {
            args.print_command_help();
        }
        args::Command::Version => {
            Args::print_version();
        }
    }

    Ok(())
}
