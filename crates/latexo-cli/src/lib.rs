//! The command line of Latexo: the commands that run in the terminal and exit instead of
//! opening the window. No `gpui`, so everything is unit tested without a window.
//!
//! [`start`] reads every argument: `-h`/`--help` and `-V`/`--version` print and exit, and anything
//! else is a usage error. Latexo opens no files, so with no argument the window opens.
//!
//! To add a command:
//! 1. Create a module with its type, a `parse` from one argument, a `run` returning an exit
//!    code and its help text (see [`info`]).
//! 2. Register it in one place: a variant in [`Command`], a line in [`Command::parse`] and one
//!    in [`Command::run`], and mention it in the help of [`info`].
//! 3. Add its tests next to it.

pub mod info;

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::sync::LazyLock;

use telaio_app::AppIdentity;

pub use info::Info;

/// Who Latexo is, for every piece of `telaio-app` the app and the command line use: the name,
/// the lowercase name of folders and files, the application id (Wayland `app_id`, X11 `WM_CLASS`,
/// Windows AppUserModelID, the same as `codes.tanis.latexo.desktop` and the Windows installer
/// shortcut), the version, the environment variable prefix and the repository.
pub const IDENTITY: AppIdentity = AppIdentity {
    name: "Latexo",
    slug: "latexo",
    app_id: "codes.tanis.latexo",
    version: env!("CARGO_PKG_VERSION"),
    env_prefix: "LATEXO",
    repository: "https://github.com/tanisperez/latexo",
};

/// The short git commit this build was made from, captured by `build.rs` (or taken from the
/// `LATEXO_COMMIT` environment variable at build time). `None` outside a repository, for example
/// a source tarball.
pub const COMMIT: Option<&str> = option_env!("LATEXO_COMMIT");

/// The suffix of the versions between two releases (`0.4.0-dev`), whose builds show their commit.
const DEV_SUFFIX: &str = "-dev";

/// The version as shown to people: `0.4.0-dev (4e74e2d)` for a development build with a known
/// commit, the bare version otherwise. A release is named by its tag, so it never shows a commit.
/// The updater, the release notes link and anything machine readable use the bare version.
pub fn format_version_label(version: &str, commit: Option<&str>) -> String {
    match commit {
        Some(commit) if version.ends_with(DEV_SUFFIX) => format!("{version} ({commit})"),
        _ => version.to_string(),
    }
}

/// [`format_version_label`] of this build.
pub fn version_label() -> &'static str {
    static LABEL: LazyLock<&'static str> = LazyLock::new(|| {
        let label = format_version_label(IDENTITY.version, COMMIT);
        // `AppIdentity` wants a `&'static str`: leaked once, for the life of the process.
        Box::leak(label.into_boxed_str())
    });
    *LABEL
}

/// [`IDENTITY`] with the version label, for what only displays the version (About, the version
/// line). Pass [`IDENTITY`] itself to everything that needs the bare version.
pub fn display_identity() -> AppIdentity {
    AppIdentity {
        version: version_label(),
        ..IDENTITY
    }
}

/// A command line command that runs without opening the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `-h`/`--help` and `-V`/`--version`.
    Info(Info),
    /// An argument Latexo does not know: a usage error.
    Unknown(String),
}

impl Command {
    /// The command asked by one argument. Every argument is a command: Latexo takes no paths.
    pub fn parse(arg: &str) -> Self {
        match Info::parse(arg) {
            Some(info) => Command::Info(info),
            None => Command::Unknown(arg.to_string()),
        }
    }

    /// Runs the command and returns the process exit code.
    pub fn run(self) -> ExitCode {
        let code = match self {
            Command::Info(info) => info.run(),
            Command::Unknown(option) => {
                let _ = std::io::stderr().write_all(unknown_argument(&option).as_bytes());
                telaio_app::cli::EXIT_USAGE
            }
        };
        ExitCode::from(code)
    }
}

/// The message of an unknown argument, on stderr.
fn unknown_argument(arg: &str) -> String {
    format!("error: unexpected argument '{arg}'\nRun 'latexo --help' to see the options.\n")
}

/// Reads the arguments (without the program name): the first one is the command, `None` opens the
/// window. A non UTF-8 argument is an unknown one.
pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Option<Command> {
    let first = args.into_iter().next()?;
    Some(Command::parse(&first.to_string_lossy()))
}

/// Handles the command line before anything else is set up: runs a command and returns its exit
/// code as the error, or `Ok` when the window should open.
///
/// On Windows `latexo.exe` is a console subsystem binary, so the output goes to the terminal, file
/// or pipe the shell gave it, and the shell waits for the exit code.
pub fn start(args: impl IntoIterator<Item = OsString>) -> Result<(), ExitCode> {
    match parse_args(args) {
        None => Ok(()),
        Some(command) => Err(command.run()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_dev_build_shows_its_commit_next_to_the_version() {
        assert_eq!(
            format_version_label("0.4.0-dev", Some("4e74e2d")),
            "0.4.0-dev (4e74e2d)"
        );
        assert_eq!(format_version_label("0.4.0-dev", None), "0.4.0-dev");
    }

    #[test]
    fn a_release_is_named_by_its_tag_only() {
        assert_eq!(format_version_label("0.4.0", Some("4e74e2d")), "0.4.0");
        assert_eq!(
            format_version_label("0.4.0-rc.1", Some("4e74e2d")),
            "0.4.0-rc.1"
        );
    }

    #[test]
    fn the_display_identity_only_changes_the_version() {
        let display = display_identity();
        assert_eq!(display.version, version_label());
        assert_eq!(display.name, IDENTITY.name);
        assert!(version_label().starts_with(IDENTITY.version));
    }

    #[test]
    fn no_argument_opens_the_window() {
        assert_eq!(parse_args(args(&[])), None);
        assert_eq!(start(args(&[])), Ok(()));
    }

    #[test]
    fn the_info_flags_are_commands_and_anything_else_is_an_error() {
        assert_eq!(
            parse_args(args(&["--help", "x"])),
            Some(Command::Info(Info::Help))
        );
        assert_eq!(Command::parse("-V"), Command::Info(Info::Version));
        assert_eq!(
            Command::parse("file.txt"),
            Command::Unknown("file.txt".to_string())
        );
        assert!(unknown_argument("--foo").contains("latexo --help"));
    }
}
