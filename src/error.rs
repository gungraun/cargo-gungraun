//! The crate error

use core::fmt::Display;
use std::process::ExitStatus;

/// The cargo-gungraun error
#[derive(Debug)]
pub enum Error {
    CommandSpawn(std::io::Error),
    Command(ExitStatus),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // TODO: Improve error messages
        match self {
            Self::CommandSpawn(error) => write!(f, "Failed spawning command: {error}"),
            Self::Command(exit_status) => write!(
                f,
                "Failed executing command: Exit status was: {exit_status}"
            ),
        }
    }
}
