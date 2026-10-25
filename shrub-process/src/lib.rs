//! Process launch for shrub and for programs that embed it: one spawn
//! request, [`Spawn`], and the functions that start it, [`spawn`] and
//! [`exec`].
//!
//! A request names the program, its arguments, environment, working
//! directory, standard streams and any other descriptors it inherits, the
//! process group it joins, and the steps its child takes after the fork and
//! before its program runs: joining a cgroup, setting resource limits,
//! checking process counts, and taking the terminal. Those steps need code
//! that runs between `fork` and `exec`, which only `unsafe` code can register;
//! this crate holds that code, so a caller writes none.
//!
//! `shrub-core` runs every external command through this crate, so a shell and
//! the program that embeds it launch processes the same way.
//!
//! ```no_run
//! # async fn run() -> Result<(), shrub_process::Error> {
//! use shrub_process::{Group, Spawn, spawn};
//!
//! let mut started = spawn(Spawn::new("ls").args(["-l"]).group(Group::New))?;
//! let status = started
//!     .child
//!     .wait()
//!     .await
//!     .map_err(shrub_process::Error::Spawn)?;
//! # let _ = status;
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeMap;
use std::ffi::{CString, OsString};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;

#[cfg(not(unix))]
mod stubs;
#[cfg(not(unix))]
use stubs as sys;
#[cfg(not(any(unix, windows)))]
pub use stubs::Child;
#[cfg(any(unix, windows))]
pub use tokio::process::Child;

/// A process to start: its program and everything the child gets.
///
/// Built with [`Spawn::new`] and the methods that follow it; a field left
/// unset keeps what [`std::process::Command`] does by default.
#[derive(Debug)]
pub struct Spawn {
    program: OsString,
    #[cfg_attr(not(unix), expect(dead_code, reason = "only Unix sets argv[0]"))]
    arg0: Option<OsString>,
    args: Vec<OsString>,
    env: Option<BTreeMap<OsString, OsString>>,
    cwd: Option<PathBuf>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    #[cfg(unix)]
    fds: Vec<(std::os::fd::RawFd, std::os::fd::OwnedFd)>,
    group: Group,
    terminal: Terminal,
    options: SpawnOptions,
    kill_on_drop: bool,
}

/// The process group a child is placed in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Group {
    /// The group of the process that spawns it.
    #[default]
    Same,
    /// A new group, whose id is the child's process id.
    New,
    /// The existing group with this id.
    Join(i32),
}

/// What a child does with the terminal on its standard input before its
/// program runs. Only a child in a new group ([`Group::New`]) takes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Terminal {
    /// Nothing.
    #[default]
    Keep,
    /// Moves its process group to the terminal's foreground.
    TakeForeground,
    /// Leads a new session, whose controlling terminal is the one on its
    /// standard input; the session's process group is the child's.
    LeadSession,
}

impl Spawn {
    /// A request to run `program`, found on `PATH` when it holds no `/`,
    /// with no arguments.
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arg0: None,
            args: Vec::new(),
            env: None,
            cwd: None,
            stdin: None,
            stdout: None,
            stderr: None,
            #[cfg(unix)]
            fds: Vec::new(),
            group: Group::Same,
            terminal: Terminal::Keep,
            options: SpawnOptions::default(),
            kill_on_drop: false,
        }
    }

    /// Sets `argv[0]`, which is otherwise the program. Only Unix sets it.
    #[must_use]
    pub fn arg0(mut self, arg0: impl Into<OsString>) -> Self {
        self.arg0 = Some(arg0.into());
        self
    }

    /// Appends `args` to the arguments.
    #[must_use]
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Gives the child exactly `vars` as its environment, in place of this
    /// process's.
    #[must_use]
    pub fn env(mut self, vars: BTreeMap<OsString, OsString>) -> Self {
        self.env = Some(vars);
        self
    }

    /// Runs the child in `dir`.
    #[must_use]
    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Sets the child's standard input.
    #[must_use]
    pub fn stdin(mut self, stdin: Stdio) -> Self {
        self.stdin = Some(stdin);
        self
    }

    /// Sets the child's standard output.
    #[must_use]
    pub fn stdout(mut self, stdout: Stdio) -> Self {
        self.stdout = Some(stdout);
        self
    }

    /// Sets the child's standard error.
    #[must_use]
    pub fn stderr(mut self, stderr: Stdio) -> Self {
        self.stderr = Some(stderr);
        self
    }

    /// Gives the child `parent` as its descriptor `child`, beside its
    /// standard streams. Two descriptors mapped to the same `child` fail the
    /// spawn with [`Error::FdMapping`].
    #[cfg(unix)]
    #[must_use]
    pub fn fd(mut self, child: std::os::fd::RawFd, parent: std::os::fd::OwnedFd) -> Self {
        self.fds.push((child, parent));
        self
    }

    /// Places the child in `group`.
    #[must_use]
    pub const fn group(mut self, group: Group) -> Self {
        self.group = group;
        self
    }

    /// Has the child take the terminal as `terminal` says.
    #[must_use]
    pub const fn terminal(mut self, terminal: Terminal) -> Self {
        self.terminal = terminal;
        self
    }

    /// Applies `options`: the child's setup steps and the spawn callbacks.
    #[must_use]
    pub fn options(mut self, options: SpawnOptions) -> Self {
        self.options = options;
        self
    }

    /// Whether dropping the [`Child`] kills the process.
    #[must_use]
    pub const fn kill_on_drop(mut self, kill: bool) -> Self {
        self.kill_on_drop = kill;
        self
    }

    /// The program the request runs.
    pub fn program(&self) -> &std::ffi::OsStr {
        &self.program
    }

    /// The arguments after `argv[0]`.
    pub fn arguments(&self) -> impl Iterator<Item = &std::ffi::OsStr> {
        self.args.iter().map(OsString::as_os_str)
    }

    /// The process group the child gets once its id is `pid`.
    const fn process_group(&self, pid: i32) -> Option<i32> {
        match (self.group, self.terminal) {
            (Group::New, _) | (_, Terminal::LeadSession) => Some(pid),
            (Group::Join(group), _) => Some(group),
            (Group::Same, _) => None,
        }
    }

    /// The [`std::process::Command`] with everything but the setup steps.
    fn base_command(&mut self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        if let Some(env) = &self.env {
            command.env_clear().envs(env);
        }
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        if let Some(stdin) = self.stdin.take() {
            command.stdin(stdin);
        }
        if let Some(stdout) = self.stdout.take() {
            command.stdout(stdout);
        }
        if let Some(stderr) = self.stderr.take() {
            command.stderr(stderr);
        }
        command
    }
}

/// What each child does before its program runs, and the callbacks around
/// each spawn, for a program that confines or tracks the processes it
/// starts. The default does nothing.
///
/// [`exec`] runs the steps in the calling process, with no fork, so an `exec`
/// that fails leaves in place the steps that ran before the failure. The order
/// is the cgroup join, the process counts, the other descriptors, then the
/// open-file limit.
#[derive(Clone, Default)]
pub struct SpawnOptions {
    /// A file each child writes `0` to after it is forked and before it runs
    /// its program. For a cgroup v2's `cgroup.procs` file, the write moves
    /// the child into that cgroup, so it cannot fork a process outside it;
    /// only Linux has cgroups. When the child cannot open the file or write
    /// to it, the spawn fails and its program never runs.
    pub cgroup_procs: Option<Arc<CString>>,
    /// The soft limit on open files (`RLIMIT_NOFILE`) each child gets, set
    /// after it is forked and before it runs its program, for a program
    /// that raised its own soft limit and gives its children the one it
    /// started with. A value above the hard limit sets the hard limit, and
    /// on macOS a value it refuses is lowered to `OPEN_MAX` (10240). `None`
    /// leaves the limit the child inherits; on a platform without resource
    /// limits, setting it fails every spawn.
    pub open_file_limit: Option<u64>,
    /// The limit on the processes of the user id (`RLIMIT_NPROC`) each
    /// child gets, set after it is forked and before it runs its program:
    /// both its soft and its hard limit become this value, or its hard limit
    /// when that is lower, so neither the program nor its descendants can
    /// raise it. The kernel then refuses a fork by the child or a descendant
    /// while the user id has that many processes, except for root and a
    /// process with `CAP_SYS_RESOURCE` or `CAP_SYS_ADMIN`, which it does not
    /// bind. [`exec`] would set it in the calling process, which would keep
    /// a hard limit it cannot raise again if the exec failed, so a caller
    /// should not exec with it set. `None` leaves the limit the child
    /// inherits. Only Linux sets it; on another platform, setting it fails
    /// every spawn.
    pub process_limit: Option<u64>,
    /// Process counts each child checks after it joins the cgroup in
    /// `cgroup_procs` and before it runs its program. The kernel does not
    /// apply a cgroup's `pids.max` to a process that moves into it, so the
    /// child checks the count itself and fails its spawn, without running its
    /// program, when a count is above its limit (`EAGAIN`) or cannot be read
    /// (`EIO`); `on_spawn` is then not called. In the calling process, as
    /// under [`exec`], the check fails with `ENOTSUP` without reading a
    /// count, because there a count would include every thread of that
    /// process. Empty, the default, checks nothing; on a platform without
    /// this check, a non-empty list fails every spawn.
    pub pids_limits: Arc<[PidsLimit]>,
    /// Called before each child is spawned. [`spawn`] holds the value it
    /// returns until `on_spawn` has returned for that child, or until the
    /// spawn fails, then drops it. A program that reaps processes it did not
    /// spawn, such as a child subreaper, holds a lock in it so that no reap
    /// runs between the fork and `on_spawn`: a child that exits in that
    /// window is a zombie the program does not yet know, and reaping it
    /// would take the exit status the spawner waits for. It runs on the
    /// spawning task, so it should return quickly.
    pub before_spawn: Option<Arc<dyn Fn() -> SpawnGuard + Send + Sync>>,
    /// Called with each child once it has started, before anything waits for
    /// it. It runs on the spawning task, so it should return quickly.
    pub on_spawn: Option<Arc<dyn Fn(SpawnedProcess) + Send + Sync>>,
}

/// What [`SpawnOptions::before_spawn`] returns, held across one spawn and
/// dropped after it.
pub type SpawnGuard = Box<dyn std::any::Any + Send>;

impl std::fmt::Debug for SpawnOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnOptions")
            .field("cgroup_procs", &self.cgroup_procs)
            .field("open_file_limit", &self.open_file_limit)
            .field("process_limit", &self.process_limit)
            .field("pids_limits", &self.pids_limits)
            .field("before_spawn", &self.before_spawn.as_ref().map(|_| "Fn"))
            .field("on_spawn", &self.on_spawn.as_ref().map(|_| "Fn"))
            .finish()
    }
}

/// A process count that each child checks before its program runs; see
/// [`SpawnOptions::pids_limits`].
#[derive(Clone, Debug)]
pub struct PidsLimit {
    /// A cgroup's `pids.current` file, opened before the spawn, so the child
    /// reads it without opening anything.
    pub current: Arc<std::fs::File>,
    /// The highest count, the child included, at which the child still runs
    /// its program.
    pub max: u64,
}

/// A process that has started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnedProcess {
    /// The child's process ID.
    pub pid: i32,
    /// The process group the child was placed in, if it is known.
    pub process_group: Option<i32>,
}

/// A started child and what is known of it.
///
/// On Unix and Windows the child runs under tokio's process driver, which is
/// the only listener for `SIGCHLD` this crate installs and which reaps only
/// the children tokio spawned; a caller waits through [`Spawned::child`] and
/// never reaps a process it did not start.
#[derive(Debug)]
pub struct Spawned {
    /// The child, to wait for or kill.
    pub child: Child,
    /// Its process id, when the platform reports one.
    pub pid: Option<i32>,
    /// Its process group, when it is known.
    pub process_group: Option<i32>,
}

/// Why a spawn or an exec failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The request asks for a step this platform cannot take.
    #[error("not supported on this platform: {0}")]
    Unsupported(&'static str),
    /// The descriptor map could not be applied, for example because two
    /// descriptors map to the same child descriptor.
    #[error("the child's descriptors could not be mapped")]
    FdMapping,
    /// The operating system refused the spawn or the exec, or a setup step
    /// failed in the child; a step reports only its errno.
    #[error(transparent)]
    Spawn(std::io::Error),
}

/// Starts `request` and returns its child.
///
/// [`SpawnOptions::before_spawn`] is called after the request is checked and
/// before the fork, and what it returns is held until
/// [`SpawnOptions::on_spawn`] has returned, or until the spawn fails.
///
/// # Errors
///
/// [`Error::Unsupported`] when the request asks for a step this platform
/// cannot take, before `before_spawn` is called; [`Error::FdMapping`] when
/// the descriptor map cannot be applied; [`Error::Spawn`] when the spawn
/// fails or a setup step fails in the child.
pub fn spawn(mut request: Spawn) -> Result<Spawned, Error> {
    let command = sys::command(&mut request)?;
    // Held until `on_spawn` returns, or until the spawn fails.
    let _guard = request.options.before_spawn.as_ref().map(|before| before());
    let child = sys::spawn(command, request.kill_on_drop).map_err(Error::Spawn)?;
    #[expect(clippy::cast_possible_wrap, reason = "a process id fits an i32")]
    let pid = child.id().map(|id| id as i32);
    let process_group = pid.and_then(|pid| request.process_group(pid));
    if let (Some(on_spawn), Some(pid)) = (&request.options.on_spawn, pid) {
        on_spawn(SpawnedProcess { pid, process_group });
    }
    Ok(Spawned {
        child,
        pid,
        process_group,
    })
}

/// Replaces the calling process with `request`'s program, after taking its
/// setup steps in this process. It returns only when that fails.
///
/// The process group, the terminal step and the spawn callbacks do not apply.
///
/// # Errors
///
/// Always returns the failure: [`Error::Unsupported`] when the request asks
/// for a step this platform cannot take, or this platform cannot replace a
/// process; [`Error::FdMapping`] when the descriptor map cannot be applied;
/// [`Error::Spawn`] when the exec or a setup step fails.
pub fn exec(mut request: Spawn) -> Error {
    match sys::command(&mut request) {
        Ok(command) => sys::exec(command),
        Err(error) => error,
    }
}

/// The error for a spawn or exec that failed with
/// [`std::io::ErrorKind::NotFound`] because the cgroup that
/// [`SpawnOptions::cgroup_procs`] names is gone, or `None` when it is not.
///
/// A child reports only an errno from before its program runs, so a cgroup
/// file that is gone reads as a missing program; this tells them apart.
pub fn missing_cgroup(options: &SpawnOptions) -> Option<std::io::Error> {
    let cgroup_procs = options.cgroup_procs.as_ref()?;
    sys::is_missing(cgroup_procs).then(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "cannot join the cgroup: {} is missing",
                cgroup_procs.to_string_lossy()
            ),
        )
    })
}
