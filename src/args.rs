use std::{io, path::PathBuf};

use crate::constants::{APP_NAME, CONFIG_NAME, STATE_FILE};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, propagate_version = true)]
pub struct Cli {
    /// Path to configuration file. If not provided will follow the XDG base directory
    /// specification.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Path to state directory. If not provided will follow the XDG base directory
    /// specification.
    #[arg(long, global = true)]
    pub state_dir: Option<PathBuf>,

    #[arg(long, hide = true)]
    test: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Go to project
    #[command(alias = "g")]
    Go(Go),
    /// Go to project and open
    #[command(alias = "o")]
    Open(Go),
    /// Create a new project
    #[command(alias = "n")]
    New(New),
    /// Create a new project and go to it
    #[command(alias = "gn")]
    GoNew(New),
    /// Create a new project, go to it, and open
    #[command(alias = "on")]
    OpenNew(New),

    /// Setup options (Generate runner and config)
    Setup(Setup),
}

/// Use in subcommands with command(flatten) to hide the display of globals in help messages
#[derive(Args)]
struct HideGlobals {
    #[arg(long, global = true, hide = true)]
    config: (),

    #[arg(long, global = true, hide = true)]
    state_dir: (),
}
impl HideGlobals {
    fn empty() -> Self {
        Self {
            config: (),
            state_dir: (),
        }
    }
}

#[derive(Args)]
pub struct Setup {
    /// Generate the runner shell script and place it at PATH, or in the same directory as this
    /// executable if not specified
    #[arg(
        long = "generate-script",
        num_args = 0..=1,
        name = "PATH",
    )]
    pub generate_shell_script: Option<Option<PathBuf>>,

    /// Generate the runner shell script with the sepcified path to the executable
    #[arg(long)]
    pub with_executable: Option<String>,

    /// Print the runner shell script
    #[arg(long = "print-script")]
    pub print_shell_script: bool,

    /// Create a default config file
    #[arg(long)]
    pub place_config: bool,

    // Hide globals in help message. see: https://github.com/clap-rs/clap/issues/5355
    #[command(flatten)]
    hidden: HideGlobals,
}

#[derive(Args)]
pub struct Go {
    lang_or_name: Option<String>,
    name: Option<String>,

    /// Language the project is under. If only lang is given, lang will be treated as name. If only
    /// lang is specified as lang, then this flag should be set to signal that
    #[arg(long, short)]
    lang: bool,

    /// Config variant to use
    #[arg(long, short)]
    pub var: Option<String>,

    // Hide globals in help message. see: https://github.com/clap-rs/clap/issues/5355
    #[command(flatten)]
    hidden: HideGlobals,
}

impl Go {
    // Returns (lang, name)
    pub fn get_lang_name(&self) -> (Option<&String>, Option<&String>) {
        match (&self.lang_or_name, &self.name, &self.lang) {
            (Some(lang), Some(name), _) => (Some(lang), Some(name)),
            (Some(lang), None, true) => (Some(lang), None),
            (Some(lang), None, false) => (None, Some(lang)),
            (None, None, _) => (None, None),
            (None, Some(_), _) => {
                unreachable!("clap ensures that name cannot be filled if lang is not")
            }
        }
    }
}
impl From<New> for Go {
    fn from(value: New) -> Self {
        Go {
            lang_or_name: Some(value.lang),
            name: Some(value.name),
            lang: false,
            var: value.var,
            hidden: HideGlobals::empty(),
        }
    }
}
#[derive(Args)]
pub struct New {
    pub lang: String,
    pub name: String,

    /// Create in cwd instead of <LANG>
    #[arg(long, short = 'H')]
    pub here: bool,

    /// Config variant to use
    #[arg(long, short)]
    pub var: Option<String>,

    /// Pass to init procedure
    #[arg(last = true)]
    pub passthrough: Vec<String>,

    // Hide globals in help message. see: https://github.com/clap-rs/clap/issues/5355
    #[command(flatten)]
    hidden: HideGlobals,
}

pub struct Pargs {
    pub config: PathBuf,
    pub state: PathBuf,
    pub command: Command,
    pub test: bool,
}
impl Pargs {
    fn get_canon_path_or_parent(path: PathBuf) -> io::Result<PathBuf> {
        let canon_path = match path.canonicalize() {
            Ok(path) => path,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // If the file doesn't exist, we might need to create it found, try see if the parent exists
                path.parent()
                    .ok_or(io::Error::new(
                        io::ErrorKind::NotFound,
                        "could not get parent",
                    ))?
                    .canonicalize()?
                    .join(
                        path.file_name()
                            .expect("Getting parent should of failed if this is None"),
                    )
            }
            // If it is a different error, return that error
            err => err?,
        };

        Ok(canon_path)
    }

    pub fn xdg_config_path(bd: &xdg::BaseDirectories) -> crate::errors::Result<PathBuf> {
        use crate::errors::new_raw;

        Ok(bd
            .get_config_file(CONFIG_NAME)
            .ok_or_else(|| new_raw("could not work out path to config file".into()))?)
    }

    pub fn new_xdg_with_prefix() -> xdg::BaseDirectories {
        xdg::BaseDirectories::with_prefix(APP_NAME)
    }

    fn resolve_config(
        bd: &xdg::BaseDirectories,
        path: Option<PathBuf>,
    ) -> crate::errors::Result<PathBuf> {
        use crate::errors::new_io;

        static ERR_MSG: &str = "locating config file (a config file can be generated with '{prog} setup --place-config'): ";

        if let Some(path) = path {
            path
        } else {
            Pargs::xdg_config_path(bd)?
        }
        // Config file must exist
        .canonicalize()
        .map_err(|e| new_io(ERR_MSG.into(), e))
    }

    fn resolve_state(
        bd: &xdg::BaseDirectories,
        path: Option<PathBuf>,
    ) -> crate::errors::Result<PathBuf> {
        use crate::errors::new_io;
        if let Some(path) = path {
            Self::get_canon_path_or_parent(path)
                .map_err(|e| new_io("locating path to state file/directory: ".into(), e))
        } else {
            bd.place_state_file(STATE_FILE)
                .map_err(|e| new_io("creating state file/directory: ".into(), e))
        }
    }

    pub fn parse() -> Cli {
        Cli::parse()
    }

    pub fn new(args: Cli) -> crate::errors::Result<Self> {
        let xdg_dirs = Pargs::new_xdg_with_prefix();
        Ok(Self {
            config: Self::resolve_config(&xdg_dirs, args.config)?,
            state: Self::resolve_state(&xdg_dirs, args.state_dir)?,
            command: args.command,
            test: args.test,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_state_valid_path() {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Pargs::resolve_state(&xdg_dirs, Some(PathBuf::from("/home"))).unwrap();
    }
    #[test]
    #[should_panic]
    fn resolve_state_invalid_path() {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Pargs::resolve_state(&xdg_dirs, Some(PathBuf::from("/940h/df98"))).unwrap();
    }

    #[test]
    fn resolve_config_valid_path() {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Pargs::resolve_config(&xdg_dirs, Some(PathBuf::from("/dev/null"))).unwrap();
    }
    #[test]
    #[should_panic]
    fn resolve_config_invalid_path() {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Pargs::resolve_config(&xdg_dirs, Some(PathBuf::from("/940h/df98"))).unwrap();
    }
}
