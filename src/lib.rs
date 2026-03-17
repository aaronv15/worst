use std::{collections::HashMap, path::PathBuf};

use crate::files::{Config, ConfigKey, State};

mod args;
mod files;

pub(crate) mod constants {
    pub(crate) const APP_NAME: &str = "worst-switcher";
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

    #[allow(dead_code)]
    pub fn new_ser(s: String) -> Error {
        Error::Serialize(s)
    }
    #[allow(dead_code)]
    pub fn new_de(s: String) -> Error {
        Error::Deserialize(s)
    }
    #[allow(dead_code)]
    pub fn new_config(s: String) -> Error {
        Error::Config(s)
    }
    #[allow(dead_code)]
    pub fn new_raw(s: String) -> Error {
        Error::Raw(s)
    }

    pub enum Error {
        Serialize(String),
        Deserialize(String),
        Config(String),
        ConfigParse(toml::de::Error),
        UserVarReplace(aho_corasick::BuildError),
        Raw(String),
        Io(std::io::Error),
    }
    impl Error {
        fn string_name(&self) -> &'static str {
            match self {
                Self::Serialize(_) => "Serialize",
                Self::Deserialize(_) => "Deserialize",
                Self::Config(_) => "Config",
                Self::ConfigParse(_) => "ConfigParse",
                Self::UserVarReplace(_) => "UserVarReplace",
                Self::Raw(_) => "Raw",
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
    impl From<aho_corasick::BuildError> for Error {
        fn from(e: aho_corasick::BuildError) -> Self {
            Self::UserVarReplace(e)
        }
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
            if !matches!(self, Error::Raw(_)) {
                f.write_str(self.string_name())?;
                f.write_str(" Error: ")?;
            }

            match self {
                Self::Serialize(e) => e.fmt(f),
                Self::Deserialize(e) => e.fmt(f),
                Self::Config(e) => e.fmt(f),
                Self::ConfigParse(e) => e.fmt(f),
                Self::UserVarReplace(e) => e.fmt(f),
                Self::Raw(e) => e.fmt(f),
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

struct BuildCmdArgs<'a> {
    lang: &'a str,
    name: &'a str,
    cmd: &'a str,
    vars: &'a HashMap<String, toml::Value>,
    base_dir: &'a PathBuf,
}
struct UpdateState {
    lang: String,
    name: String,
    base_dir: PathBuf,
}

fn build_command(args: BuildCmdArgs) -> errors::Result<String> {
    use constants::{SUB_DIR, SUB_LANG, SUB_NAME};

    let base_path_str = args.base_dir.to_string_lossy();
    let vars: Vec<(&str, String)> = args
        .vars
        .iter()
        .map(|(k, v)| (k.as_str(), String::from(v.type_str())))
        .collect();

    let (search, replace) = [
        (SUB_LANG, args.lang),
        (SUB_NAME, args.name),
        (SUB_DIR, &base_path_str),
    ]
    .into_iter()
    .chain(vars.iter().map(|(k, v)| (*k, v.as_str())))
    .collect::<(Vec<&str>, Vec<&str>)>();

    let patterns = aho_corasick::AhoCorasick::new(search)?;
    let cmd_str = patterns.replace_all(args.cmd, &replace);

    Ok(cmd_str)
}

fn get_go_cmd_build_cmd_args<'a>(
    cfg: &'a Config,
    get_cfg_cmd: impl FnOnce(&'a Config, &ConfigKey) -> &'a str,
    st: &mut State,
    cmd: args::Go,
) -> errors::Result<(UpdateState, String)> {
    let (lang, name) = cmd.get_lang_name();
    let build_cmd_args: BuildCmdArgs = match (lang, name) {
        (Some(lang), Some(name)) => {
            let key = &ConfigKey::new(lang, cmd.var.as_ref());

            BuildCmdArgs {
                lang,
                name,
                cmd: get_cfg_cmd(cfg, key),
                vars: cfg.user_vars(key),
                base_dir: cfg.base_dir(key),
            }
        }
        (Some(lang), None) => {
            let key = &ConfigKey::new(lang, cmd.var.as_ref());
            let pso = st.latest_by_lang(lang).ok_or_else(|| {
                errors::new_raw(format!("No stored projects for language {}", lang))
            })?;

            BuildCmdArgs {
                lang,
                name: &pso.proj,
                cmd: get_cfg_cmd(cfg, key),
                vars: cfg.user_vars(key),
                base_dir: &pso.base_dir,
            }
        }
        (None, Some(name)) => {
            let pso = st
                .latest_by_name(name)
                .ok_or_else(|| errors::new_raw(format!("No project named '{}' found", name)))?;
            let key = &ConfigKey::new(&pso.language, cmd.var.as_ref());

            BuildCmdArgs {
                lang: &pso.language,
                name,
                cmd: get_cfg_cmd(cfg, key),
                vars: cfg.user_vars(key),
                base_dir: &pso.base_dir,
            }
        }
        (None, None) => {
            let pso = st.latest().ok_or(errors::new_raw(
                "No projects have been registered to go to".into(),
            ))?;
            let key = &ConfigKey::new(&pso.language, cmd.var.as_ref());

            BuildCmdArgs {
                lang: &pso.language,
                name: &pso.proj,
                cmd: get_cfg_cmd(cfg, key),
                vars: cfg.user_vars(key),
                base_dir: &pso.base_dir,
            }
        }
    };

    Ok((
        UpdateState {
            lang: build_cmd_args.lang.into(),
            name: build_cmd_args.name.into(),
            base_dir: build_cmd_args.base_dir.into(),
        },
        build_command(build_cmd_args)?,
    ))
}

fn handle_go(cfg: &Config, st: &mut State, cmd: args::Go) -> errors::Result<(UpdateState, String)> {
    get_go_cmd_build_cmd_args(cfg, Config::go_cmd, st, cmd)
}

fn handle_open(
    cfg: &Config,
    st: &mut State,
    cmd: args::Go,
) -> errors::Result<(UpdateState, String)> {
    get_go_cmd_build_cmd_args(cfg, Config::open_cmd, st, cmd)
}

fn handle_new(
    cfg: &Config,
    _st: &mut State,
    cmd: &args::New,
) -> errors::Result<(UpdateState, String)> {
    let (lang, name) = (&cmd.lang, &cmd.name);
    let key = &ConfigKey::new(lang, cmd.var.as_ref());

    let build_cmd_args: BuildCmdArgs = BuildCmdArgs {
        lang,
        name,
        cmd: cfg.new_cmd(key),
        vars: cfg.user_vars(key),
        base_dir: cfg.base_dir(key),
    };

    Ok((
        UpdateState {
            lang: build_cmd_args.lang.into(),
            name: build_cmd_args.name.into(),
            base_dir: build_cmd_args.base_dir.into(),
        },
        build_command(build_cmd_args)?,
    ))
}

fn handle_go_new(
    cfg: &Config,
    st: &mut State,
    cmd: args::New,
) -> errors::Result<(UpdateState, String)> {
    let (update_state, output) = handle_new(cfg, st, &cmd)?;
    Ok((
        update_state,
        format!("{{{}\n{}}}", output, handle_go(cfg, st, cmd.into())?.1),
    ))
}

fn handle_open_new(
    cfg: &Config,
    st: &mut State,
    cmd: args::New,
) -> errors::Result<(UpdateState, String)> {
    let (update_state, output) = handle_new(cfg, st, &cmd)?;
    Ok((
        update_state,
        format!("{{{}\n{}}}", output, handle_open(cfg, st, cmd.into())?.1),
    ))
}

fn get_output_str() -> errors::Result<String> {
    let args = args::Pargs::parse()?;
    let conf = Config::new(args.config)?;
    let mut state = State::de(args.state)?;

    let (update_state, result) = {
        use args::Command::*;
        match args.command {
            Go(cmd) => handle_go(&conf, &mut state, cmd),
            Open(cmd) => handle_open(&conf, &mut state, cmd),
            New(cmd) => handle_new(&conf, &mut state, &cmd),
            GoNew(cmd) => handle_go_new(&conf, &mut state, cmd),
            OpenNew(cmd) => handle_open_new(&conf, &mut state, cmd),
        }
    }?;

    state.insert(update_state.name, update_state.lang, update_state.base_dir);
    state.ser()?;

    Ok(result)
}

pub fn run() {
    let output = match get_output_str() {
        Ok(s) => s,
        Err(e) => format!("{{echo '{}'}}", e.to_string().escape_default()),
    };
    print!("{}", output);
}
