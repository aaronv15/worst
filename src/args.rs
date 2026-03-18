use std::{io, path::PathBuf};

use crate::constants::{APP_NAME, CONFIG_NAME, STATE_FILE};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, propagate_version = true)]
struct Cli {
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
    command: Command,
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

    fn resolve_config(
        bd: &xdg::BaseDirectories,
        path: Option<PathBuf>,
    ) -> crate::errors::Result<PathBuf> {
        use crate::errors::{new_io, new_raw};
        if let Some(path) = path {
            path
        } else {
            bd.get_config_file(CONFIG_NAME)
                .ok_or_else(|| new_raw("could not work out path to config file".into()))?
        }
        // Config file must exist
        .canonicalize()
        .map_err(|e| new_io("locating config file: ".into(), e))
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

    pub fn parse() -> crate::errors::Result<Self> {
        let args = Cli::parse();

        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Ok(Self {
            config: Self::resolve_config(&xdg_dirs, args.config)?,
            state: Self::resolve_state(&xdg_dirs, args.state_dir)?,
            command: args.command,
            test: args.test,
        })
    }
}
