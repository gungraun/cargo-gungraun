//! The module for docker and podman

// spell-checker: ignore idirafter nocapture termmodes

use core::fmt::Write as _;
use core::ops::{Deref, DerefMut};
use core::time::Duration;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::thread::sleep;

use anyhow::{anyhow, Context, Result};
use log::{debug, log_enabled};
use which::which;

use crate::error::Error;
use crate::meta::{ContainerData, EngineData, HostData};
use crate::{envs, Target};

#[derive(Debug, Clone, Copy)]
pub enum Engine {
    Podman,
    Docker,
}

impl Engine {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn resolve(&self) -> Result<PathBuf> {
        let name = match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        };

        which(name).with_context(|| "Container engine executable not found")
    }
}

impl TryFrom<&OsStr> for Engine {
    type Error = anyhow::Error;

    fn try_from(value: &OsStr) -> core::result::Result<Self, Self::Error> {
        match value.as_bytes() {
            b"podman" => Ok(Self::Podman),
            b"docker" => Ok(Self::Docker),
            _ => Err(anyhow!(
                "Invalid container engine: '{}'. Expected one of 'podman' or 'docker'",
                value.to_string_lossy()
            )),
        }
    }
}

#[derive(Debug)]
pub struct Command(std::process::Command);

impl Deref for Command {
    type Target = std::process::Command;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Command {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Command {
    /// TODO: DOCS
    ///
    /// # Errors
    pub fn new(engine: Engine) -> Result<Self> {
        let exe = engine.resolve()?;
        let cmd = std::process::Command::new(exe);
        Ok(Self(cmd))
    }
}

/// TODO: DOCS
///
/// # Errors
///
/// # Panics
#[allow(clippy::too_many_lines)]
pub fn run_bench(target: Target, cargo_args: Vec<OsString>) -> Result<()> {
    let host = HostData::new()?;
    let container = ContainerData::new(&host)?;
    let engine_data = EngineData::new(target, &host)?;

    let target_upper_env = target.to_upper_env();
    let gnu_triple = target.to_gnu_triple();
    let sysroot = format!("/usr/{gnu_triple}");

    let seccomp = include_str!("seccomp.json");
    let mut file =
        File::create(&engine_data.seccomp_path).with_context(|| "Failed to create seccomp.json")?;
    file.write_all(seccomp.as_bytes())
        .with_context(|| "Failed to write to seccomp.json")?;

    std::process::Command::new("rustup")
        .args(["target", "add", &target.to_string()])
        .status()
        .map_err(Error::CommandSpawn)
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::Command(status))
            }
        })?;

    let mut up_command = Command::new(engine_data.engine)?;
    up_command.arg("run");

    match engine_data.engine {
        // TODO: This flag will ignore the PODMAN_USERNS environment variable
        Engine::Podman => {
            up_command.arg("--userns=host");
        }
        Engine::Docker => {} // TODO: implement
    }

    up_command.arg(format!(
        "--security-opt=seccomp={}",
        &engine_data.seccomp_path
    ));
    up_command.args(["--name", &container.name, "--rm"]);
    up_command.args([
        "--volume",
        &format!("{}:{}", host.gungraun_home, container.gungraun_home),
        "--volume",
        &format!("{}:{}", host.target_dir, container.target_dir),
        "--volume",
        &format!("{}:{}", host.workspace_root, container.workspace_root),
        "--volume",
        &format!("{}:{}", host.cargo_home, container.cargo_home),
        "--volume",
        &format!("{}:{}", host.rustup_home, container.rustup_home),
    ]);
    for volume in &engine_data.volumes {
        up_command.args(["--volume", volume]);
    }

    if let Some(path) = host.gungraun_runner {
        debug!("Found {}. Using '{path}'", envs::GUNGRAUN_RUNNER);
        up_command.args([
            "--volume",
            &format!("{}:{}:exec,ro", path, container.gungraun_runner),
        ]);
    }

    up_command.args([
        "--env",
        &format!("AR={gnu_triple}-ar"),
        "--env",
        &format!("CC={gnu_triple}-gcc"),
        "--env",
        &format!("LD={gnu_triple}-ld"),
        "--env",
        &format!(
            "BINDGEN_EXTRA_CLANG_ARGS_{target_upper_env}=--sysroot={sysroot} -idirafter/usr/include"
        ),
    ]);

    let mut extra_envs = String::new();
    for (key, value) in &engine_data.envs {
        let env = format!("{key}={value}");
        up_command.args(["--env", &env]);
        write!(extra_envs, "{env} ").unwrap();
    }

    up_command.args([
        "--env",
        &format!("USER={}", container.user),
        "--env",
        &format!("HOME={}", container.home),
        "--env",
        // TODO: EXTRA_PATH
        &format!(
            "PATH={}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            container.cargo_home
        ),
        "--env",
        &format!("SHELL={}", container.shell),
        "--env",
        &format!("{}={}", envs::CARGO_HOME, container.cargo_home),
        "--env",
        &format!("{}={}", envs::RUSTUP_HOME, container.rustup_home),
        "--env",
        &format!("{}={}", envs::GUNGRAUN_HOME, container.gungraun_home),
        "--env",
        &format!("{}={}", envs::GUNGRAUN_RUNNER, container.gungraun_runner),
        "--env",
        &format!(
            "{}={}",
            envs::GUNGRAUN_SEPARATE_TARGETS,
            container.separate_targets
        ),
        "--env",
        &format!("{}={}", envs::GUNGRAUN_VERSION, host.gungraun_version),
        "--env",
        &format!("{}={}", envs::CARGO_TARGET_DIR, container.target_dir),
        "--env",
        &format!(
            "CARGO_TARGET_{target_upper_env}_RUNNER={}",
            container.runner
        ),
        "--env",
        &format!("CARGO_TARGET_{target_upper_env}_LINKER={gnu_triple}-gcc",),
        "--env",
        &format!("{}={sysroot}", envs::QEMU_LD_PREFIX),
    ]);

    if engine_data.has_accelerator() {
        up_command.arg("--privileged");
    }

    up_command.args([&engine_data.image, "/bootstrap.sh"]);

    debug!("Running up cmd: {up_command:?}");
    let mut up_child = up_command
        // TODO: CHECK or make configurable via `--nocapture` or similar
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| "Spawning the container should succeed")?;

    if let Some(stdout) = up_child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut buffer = String::new();
            match reader.read_line(&mut buffer) {
                Ok(_) => {}
                Err(error) => {
                    up_child
                        .kill()
                        .expect("Killing the up child process should succeed");
                    up_child
                        .wait()
                        .expect("Waiting for the up child process should succeed");
                    return Err(anyhow!("Failed to execute the run command: '{error}'"));
                }
            }

            if buffer.trim() == "cargo-gungraun: bootstrap finished" {
                debug!("bootstrap script succeeded");
                break;
            }

            // Avoid a possible hot loop waiting some milliseconds
            sleep(Duration::from_millis(100));
            match up_child.try_wait() {
                Ok(Some(status)) => {
                    return Err(anyhow!(
                        "Failed executing the container engine. Child returned with: '{status}'"
                    ))
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(anyhow!("Failed to wait for the child process: '{error}'"))
                }
            }
        }
    } else {
        up_child
            .kill()
            .expect("Killing the up child process should succeed");
        up_child
            .wait()
            .expect("Waiting for the up child process should succeed");
        return Err(anyhow!("Expected a stdout handle of the up child process"));
    }

    // TODO: The log-file doesn't have any effect, and neither the others below
    let mut executor_args = format!(
        "--qemu-arch {target} --log-file {}/qemu.log",
        container.gungraun_home
    );

    // TODO: add other envs,
    // TODO: QEMU_EXTRA_ARGS, QEMU_MAX_MEM, QEMU_MIN_MEM, QEMU_MEM, same with cpus
    // TODO: QEMU_LOG_FILE

    if log_enabled!(log::Level::Trace) {
        write!(executor_args, " --debug trace").unwrap();
    } else if log_enabled!(log::Level::Debug) {
        write!(executor_args, " --debug debug").unwrap();
    } else {
        // do nothing
    }

    if !extra_envs.is_empty() {
        write!(executor_args, " --envs '{extra_envs}'").unwrap();
    }

    let mut exec_command = Command::new(engine_data.engine)?;
    exec_command.args([
        "exec",
        "--workdir",
        container.current_dir.as_str(),
        "--env",
        &format!("{}={}", envs::GUNGRAUN_EXECUTOR, container.qemu_runner),
        "--env",
        &format!("{}={}", envs::GUNGRAUN_EXECUTOR_ARGS, executor_args),
        // TODO: CLEANUP, doesn't work. Error message is
        // dbclient: Failed reading termmodes
        //
        // dbclient: Connection to root@localhost:10022 exited: Failed to set raw TTY mode
        // "--env",
        // "GUNGRAUN_NOCAPTURE=yes",
        "--env",
        "GUNGRAUN_LOG=warn",
    ]);

    if let Some(accel) = engine_data.accelerator {
        exec_command.args([
            "--env",
            &format!("{}={accel}", envs::CARGO_GUNGRAUN_QEMU_ACCELERATOR),
        ]);
    }

    for env in [
        envs::CARGO_GUNGRAUN_QEMU_TIMEOUT,
        envs::CARGO_GUNGRAUN_QEMU_EXTRA_ARGS,
    ] {
        if let Ok(value) = std::env::var(env) {
            exec_command.args(["--env", &format!("{env}={value}")]);
        }
    }

    if io::stdin().is_terminal() && io::stdout().is_terminal() && io::stderr().is_terminal() {
        exec_command.arg("-t");
    }

    exec_command.args([&container.name, "cargo", "bench"]);
    exec_command.args(cargo_args);
    // TODO: revert TEST
    // exec_command.args(["-i", &container.name, "/bin/bash"]);

    debug!("Running the exec command: {exec_command:?}");
    exec_command
        .status()
        .map_err(Error::CommandSpawn)
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::Command(status))
            }
        })?;

    // TODO: Instead of Stdio::null route to a log file
    debug!("Stopping the container '{}' ...", &container.name);
    Command::new(engine_data.engine)?
        .args(["stop", "--ignore", &container.name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(Error::CommandSpawn)
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::Command(status))
            }
        })?;

    // The process should not run anymore but still wait to avoid zombies
    up_child.wait().map_err(Error::CommandSpawn)?;

    Ok(())
}
