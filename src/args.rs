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
    pub lang: Option<String>,
    pub name: Option<String>,
}
impl Go {
    // Returns (lang, name)
    pub fn get_lang_name(&self) -> (Option<&String>, Option<&String>) {
        (
            if self.name.is_some() {
                self.lang.as_ref()
            } else {
                self.name.as_ref()
            },
            self.name.as_ref().or(self.lang.as_ref()),
        )
    }
}
#[derive(Args)]
pub struct New {
    pub lang: String,
    pub name: String,

    /// Create in cwd instead of <LANG>
    #[arg(long, short = 'H')]
    pub here: bool,

    /// Pass to init procedure
    #[arg(last = true)]
    pub passthrough: Vec<String>,
}

pub struct Pargs {
    pub config: PathBuf,
    pub state: PathBuf,
    pub command: Command,
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

    fn resolve_config(bd: &xdg::BaseDirectories, path: Option<PathBuf>) -> io::Result<PathBuf> {
        if let Some(path) = path {
            path
        } else {
            bd.get_config_file(CONFIG_NAME).ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                "config file not found",
            ))?
        }
        // Config file must exist
        .canonicalize()
    }

    fn resolve_state(bd: &xdg::BaseDirectories, path: Option<PathBuf>) -> io::Result<PathBuf> {
        if let Some(path) = path {
            Self::get_canon_path_or_parent(path)
        } else {
            bd.place_state_file(STATE_FILE)
        }
    }

    pub fn parse() -> io::Result<Self> {
        let args = Cli::parse();

        let xdg_dirs = xdg::BaseDirectories::with_prefix(APP_NAME);
        Ok(Self {
            config: Self::resolve_config(&xdg_dirs, args.config)?,
            state: Self::resolve_state(&xdg_dirs, args.state_dir)?,
            command: args.command,
        })
    }
}
