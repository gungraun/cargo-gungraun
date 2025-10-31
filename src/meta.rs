use std::env::{current_dir, VarError};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use csv::StringRecord;
use home::{cargo_home, rustup_home};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simplematch::{DoWild, Options};

use crate::container::Engine;
use crate::error::Error;
use crate::{cargo_bin, envs, Target};

pub const CARGO_GUNGRAUN_VERSION: &str = env!("CARGO_PKG_VERSION");
// TODO: CHECK IF these options are needed or if without options would suffice
pub const SIMPLEMATCH_OPTIONS: Options<u8> = Options::new().enable_escape(true).enable_classes(true);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoMetadata {
    pub packages: Vec<Package>,
    pub target_directory: PathBuf,
    pub workspace_root: PathBuf,
}

pub struct ContainerData {
    pub cargo_home: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub gungraun_home: Utf8PathBuf,
    pub gungraun_runner: Utf8PathBuf,
    pub home: Utf8PathBuf,
    pub name: String,
    pub qemu_runner: Utf8PathBuf,
    pub runner: Utf8PathBuf,
    pub rustup_home: Utf8PathBuf,
    pub separate_targets: String,
    pub shell: Utf8PathBuf,
    pub target_dir: Utf8PathBuf,
    pub user: String,
    pub workspace_root: Utf8PathBuf,
}

pub struct EngineData {
    pub accelerator: Option<String>,
    pub engine: Engine,
    pub envs: Vec<(String, String)>,
    pub image: String,
    pub seccomp_path: Utf8PathBuf,
    pub volumes: Vec<String>,
}

pub struct HostData {
    pub cargo_home: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub gungraun_home: Utf8PathBuf,
    pub gungraun_runner: Option<Utf8PathBuf>,
    pub gungraun_version: String,
    pub rustup_home: Utf8PathBuf,
    pub target_dir: Utf8PathBuf,
    pub workspace_root: Utf8PathBuf,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

impl CargoMetadata {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn new() -> Result<Self> {
        let output = std::process::Command::new(cargo_bin())
            .args(["metadata", "--format-version=1"])
            .output()
            .map_err(Error::CommandSpawn)
            .and_then(|output| {
                if output.status.success() {
                    Ok(output)
                } else {
                    Err(Error::Command(output.status))
                }
            })
            .with_context(|| "Failed to execute cargo")?;

        serde_json::from_slice(&output.stdout).map_err(Into::into)
    }

    #[must_use]
    pub fn gungraun_version(&self) -> Option<String> {
        self.packages
            .iter()
            .find_map(|p| (p.name == "gungraun").then(|| p.version.clone()))
    }
}

impl ContainerData {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn new(host: &HostData) -> Result<Self> {
        // TODO: Adjust this to the gungraun user if the user namespace is not host (podman) and
        // whatever docker needs.
        let (user, home) = ("root".to_owned(), Utf8PathBuf::from("/root"));
        let container_name = format!(
            "cargo-gungraun-{}",
            hex::encode(rand::rng().random::<[u8; 8]>())
        );

        let workspace_root = Utf8PathBuf::from("/workspace");
        let current_dir = workspace_root.join(
            host.current_dir
                .strip_prefix(&host.workspace_root)
                .with_context(|| "The current directory should be within the workspace root")?,
        );
        let separate_targets =
            std::env::var(envs::GUNGRAUN_SEPARATE_TARGETS).unwrap_or_else(|_| "yes".to_owned());

        Ok(Self {
            cargo_home: home.join(".cargo"),
            current_dir,
            gungraun_home: Utf8PathBuf::from("/gungraun_home"),
            name: container_name,
            gungraun_runner: Utf8PathBuf::from("/usr/bin/gungraun-runner"),
            qemu_runner: Utf8PathBuf::from("/qemu_runner.sh"),
            runner: Utf8PathBuf::from("/runner.sh"),
            rustup_home: home.join(".rustup"),
            separate_targets,
            shell: Utf8PathBuf::from("/bin/bash"),
            target_dir: Utf8PathBuf::from("/target"),
            user,
            workspace_root,
            home,
        })
    }
}

impl EngineData {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn new(target: Target, host_data: &HostData) -> Result<Self> {
        let engine = match std::env::var_os(envs::CARGO_GUNGRAUN_ENGINE) {
            Some(var) => Engine::try_from(var.as_os_str())?,
            None => Engine::Podman
                .resolve()
                .map_or_else(|_| Engine::Docker, |_| Engine::Podman),
        };

        let image = if let Ok(value) = std::env::var(envs::CARGO_GUNGRAUN_IMAGE) {
            value
        } else {
            // TODO: Adjust this to the real address
            format!("ghcr.io/cargo-gungraun/{target}:{CARGO_GUNGRAUN_VERSION}")
        };

        // TODO: canonicalize_utf8? Parse like csv but with ';' as delimiter?
        let volumes = match std::env::var(envs::CARGO_GUNGRAUN_VOLUMES) {
            Ok(value) => value.split(';').map(ToOwned::to_owned).collect(),
            Err(VarError::NotUnicode(_)) => {
                return Err(anyhow!(
                    "Invalid {}: Not utf8",
                    envs::CARGO_GUNGRAUN_VOLUMES
                ))
            }
            Err(VarError::NotPresent) => vec![],
        };

        let envs = match std::env::var(envs::CARGO_GUNGRAUN_ENVS) {
            Ok(value) => resolve_csv_env(&value)?,
            Err(VarError::NotUnicode(_)) => {
                return Err(anyhow!("Invalid {}: Not utf8", envs::CARGO_GUNGRAUN_ENVS))
            }
            Err(VarError::NotPresent) => vec![],
        };

        let accelerator = std::env::var(envs::CARGO_GUNGRAUN_QEMU_ACCELERATOR).ok();

        Ok(Self {
            accelerator,
            engine,
            envs,
            image,
            seccomp_path: host_data.gungraun_home.join("seccomp.json"),
            volumes,
        })
    }

    /// TODO: DOCS
    #[must_use]
    pub const fn has_accelerator(&self) -> bool {
        self.accelerator.is_some()
    }
}

impl HostData {
    /// TODO: DOCS
    ///
    /// # Errors
    #[allow(clippy::too_many_lines)]
    pub fn new() -> Result<Self> {
        let cargo_home: Utf8PathBuf = cargo_home()
            .with_context(|| "Failed resolving cargo home directory")?
            .try_into()
            .with_context(|| "Failed converting cargo home directory into an utf8 path")?;
        let rustup_home: Utf8PathBuf = rustup_home()
            .with_context(|| "Failed resolving rustup home directory")?
            .try_into()
            .with_context(|| "Failed converting rustup home directory into an utf8 path")?;

        let metadata = CargoMetadata::new()?;
        let gungraun_version = metadata
            .gungraun_version()
            .with_context(|| "Failed to detect gungraun version. Is gungraun installed?")?;

        let mut target_dir: Utf8PathBuf = metadata
            .target_directory
            .try_into()
            .with_context(|| "Failed converting target directory into an utf8 path")?;

        std::fs::create_dir_all(&target_dir)
            .with_context(|| "Failed creating the target directory")?;

        target_dir = target_dir
            .canonicalize_utf8()
            .with_context(|| format!("Failed to resolve the target directory: '{target_dir}'"))?;

        let workspace_root: Utf8PathBuf = metadata
            .workspace_root
            .try_into()
            .with_context(|| "Failed converting workspace root directory into an utf8 path")?;

        let mut current_dir: Utf8PathBuf = current_dir()
            .with_context(|| "Failed retrieving current directory")?
            .try_into()
            .with_context(|| "Failed converting current directory into an utf8 path")?;

        current_dir = current_dir
            .canonicalize_utf8()
            .with_context(|| "Failed to canonicalize the current directory")?;

        let mut gungraun_home = match std::env::var(envs::GUNGRAUN_HOME) {
            Ok(path) => current_dir.join(Utf8PathBuf::from(&path)),
            Err(_) => target_dir.join("gungraun"),
        };

        std::fs::create_dir_all(&gungraun_home)
            .with_context(|| "Failed creating the gungraun home directory")?;

        gungraun_home = gungraun_home.canonicalize_utf8().with_context(|| {
            format!("Failed to resolve gungraun home directory: '{gungraun_home}'")
        })?;

        // TODO: canonicalize_utf8?
        let gungraun_runner = std::env::var_os(envs::GUNGRAUN_RUNNER)
            .map(|var| Utf8PathBuf::try_from(var).map(|path| current_dir.join(path)))
            .transpose()
            .with_context(|| format!("{} points to an invalid utf8 path", envs::GUNGRAUN_RUNNER))?;

        Ok(Self {
            cargo_home,
            current_dir,
            gungraun_home,
            gungraun_runner,
            gungraun_version,
            rustup_home,
            target_dir,
            workspace_root,
        })
    }
}

fn parse_csv_env(data: &str) -> Result<Vec<String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut record = StringRecord::new();
    reader.read_record(&mut record)?;

    Ok(record
        .iter()
        .filter(|r| !r.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<String>>())
}

fn resolve_csv_env(key: &str) -> Result<Vec<(String, String)>> {
    match std::env::var(key) {
        Ok(value) => {
            let envs = parse_csv_env(&value).with_context(|| {
                format!("Parsing environment variable '{key}' failed: Invalid csv")
            })?;

            let mut pairs = vec![];
            for env in envs {
                match env.split_once('=') {
                    Some((key, value)) => pairs.push((key.to_owned(), value.to_owned())),
                    None => {
                        for (key, value) in std::env::vars() {
                            if env.as_str().dowild_with(&key, SIMPLEMATCH_OPTIONS) {
                                pairs.push((key, value));
                            }
                        }
                    }
                }
            }
            Ok(pairs)
        }
        Err(VarError::NotUnicode(_)) => Err(anyhow!("Invalid {key}: Not utf8")),
        Err(VarError::NotPresent) => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty("", vec![])]
    #[case::empty("FOO", vec!["FOO"])]
    #[case::key_value("FOO=bar", vec!["FOO=bar"])]
    #[case::value_in_quotes("FOO=\"bar\"", vec!["FOO=\"bar\""])]
    #[case::value_with_spaces("FOO=\"bar with spaces\"", vec!["FOO=\"bar with spaces\""])]
    #[case::trailing_comma("FOO=bar,", vec!["FOO=bar"])]
    #[case::multiple_comma("FOO=bar,,,,BAR=foo", vec!["FOO=bar", "BAR=foo"])]
    #[case::two_valid_vars("FOO=value,BAR=other", vec!["FOO=value", "BAR=other"])]
    #[case::quote_spaces("\"FOO=value with spaces\",BAR=other", vec!["FOO=value with spaces", "BAR=other"])]
    #[case::quote_commas("\"FOO=value,with,commas\",BAR=other", vec!["FOO=value,with,commas", "BAR=other"])]
    fn parse_csv_env_when_ok(#[case] data: &str, #[case] expected: Vec<&str>) {
        let result = parse_csv_env(data).unwrap();
        assert_eq!(result, expected);
    }
}
