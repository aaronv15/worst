use crate::files::{Config, ConfigKey, State};

mod args;
mod files;

pub(crate) mod constants {
    pub(crate) const APP_NAME: &str = "proj-switcher";
    pub(crate) const CONFIG_NAME: &str = "config.toml";
    pub(crate) const STATE_FILE: &str = "state";

    pub(crate) const SUB_LANG: &str = "%{lang}";
    pub(crate) const SUB_NAME: &str = "%{name}";
    pub(crate) const SUB_DIR: &str = "%{dir}";
}

pub(crate) mod errors {
    pub type Result<T> = core::result::Result<T, Error>;

    type SerErr = ciborium::ser::Error<std::io::Error>;
    type DeErr = ciborium::de::Error<std::io::Error>;

    pub enum Error {
        Serialize(String),
        Deserialize(String),
        Config(String),
        ConfigParse(toml::de::Error),
        Io(std::io::Error),
    }
    impl Error {
        fn string_name(&self) -> &'static str {
            match self {
                Self::Serialize(_) => "Serialize",
                Self::Deserialize(_) => "Deserialize",
                Self::Config(_) => "Config",
                Self::ConfigParse(_) => "ConfigParse",
                Self::Io(_) => "Io",
            }
        }
    }

    impl From<DeErr> for Error {
        fn from(e: DeErr) -> Self {
            match e {
                ciborium::de::Error::Io(e) => e.into(),
                e => Self::Deserialize(e.to_string()),
            }
        }
    }
    impl From<SerErr> for Error {
        fn from(e: SerErr) -> Self {
            match e {
                ciborium::ser::Error::Io(e) => e.into(),
                ciborium::ser::Error::Value(s) => Self::Serialize(s),
            }
        }
    }
    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            Self::Io(e)
        }
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
            f.write_str(self.string_name())?;
            f.write_str(" Error: ")?;

            match self {
                Self::Serialize(e) => e.fmt(f),
                Self::Deserialize(e) => e.fmt(f),
                Self::Config(e) => e.fmt(f),
                Self::ConfigParse(e) => e.fmt(f),
                Self::Io(e) => e.fmt(f),
            }
        }
    }
    impl std::fmt::Debug for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
            <Error as std::fmt::Display>::fmt(self, f)
        }
    }
    impl std::error::Error for Error {}
}

fn handle_go(cfg: &Config, st: &mut State, cmd: args::Go) -> errors::Result<()> {
    let (lang, name) = cmd.get_lang_name();
    match (lang, name) {
        (Some(lang), Some(name)) => {
            ConfigKey::new(language, shell, platform)
            cfg.base_dir()
            st.insert()
    }
}
fn handle_open(cfg: &Config, st: &mut State, cmd: args::Go) -> errors::Result<()> {}
fn handle_new(cfg: &Config, st: &mut State, cmd: args::New) -> errors::Result<()> {}
fn handle_go_new(cfg: &Config, st: &mut State, cmd: args::New) -> errors::Result<()> {}
fn handle_open_new(cfg: &Config, st: &mut State, cmd: args::New) -> errors::Result<()> {}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = args::Pargs::parse()?;
    let conf = Config::new(args.config)?;
    let mut state = State::de(args.state)?;

    {
        use args::Command::*;
        match args.command {
            Go(cmd) => handle_go(&conf, &mut state, cmd)?,
            Open(cmd) => handle_open(&conf, &mut state, cmd)?,
            New(cmd) => handle_new(&conf, &mut state, cmd)?,
            GoNew(cmd) => handle_go_new(&conf, &mut state, cmd)?,
            OpenNew(cmd) => handle_open_new(&conf, &mut state, cmd)?,
        }
    }

    Ok(())
}
