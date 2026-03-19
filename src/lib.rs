use std::{collections::HashMap, path::PathBuf};

use crate::files::{Config, ConfigKey, State};

mod args;
mod files;

pub(crate) mod constants {
    use constcat::concat as constcat;
    use std::sync::LazyLock;

    pub(crate) const APP_NAME: &str = "worst-switcher";
    pub(crate) const CONFIG_NAME: &str = "config.toml";
    pub(crate) const STATE_FILE: &str = "state.cbor";

    pub(crate) const SUB_LANG: &str = "%{lang}";
    pub(crate) const SUB_NAME: &str = "%{name}";
    pub(crate) const SUB_DIR: &str = "%{base_dir}";
    pub(crate) const SUB_PATH: &str = "%{path}";

    pub(crate) const NEW_DEFAULT: &str = constcat!("mkdir ", SUB_PATH);
    pub(crate) const GO_DEFAULT: &str = constcat!("cd ", SUB_PATH);
    pub(crate) const OPEN_DEFAULT: &str = "nvim .";
    pub(crate) const BASE_DIR_DEFAULT: LazyLock<String> =
        LazyLock::new(|| std::env::var("HOME").unwrap() + "/Documents");
}

pub(crate) mod errors {
    pub type Result<T> = core::result::Result<T, Error>;

    type SerErr = ciborium::ser::Error<std::io::Error>;
    type DeErr = ciborium::de::Error<std::io::Error>;

    pub fn new_ser(s: String, e: SerErr) -> Error {
        Error::Serialize(s, e)
    }
    pub fn new_de(s: String, e: DeErr) -> Error {
        Error::Deserialize(s, e)
    }
    pub fn new_config(s: String) -> Error {
        Error::Config(s)
    }
    pub fn new_config_parse(s: String, e: toml::de::Error) -> Error {
        Error::ConfigParse(s, e)
    }
    pub fn new_user_var_replace(s: String, e: aho_corasick::BuildError) -> Error {
        Error::UserVarReplace(s, e)
    }
    pub fn new_raw(s: String) -> Error {
        Error::Raw(s)
    }
    pub fn new_io(s: String, e: std::io::Error) -> Error {
        Error::Io(s, e)
    }

    pub enum Error {
        Serialize(String, SerErr),
        Deserialize(String, DeErr),
        Config(String),
        ConfigParse(String, toml::de::Error),
        UserVarReplace(String, aho_corasick::BuildError),
        Raw(String),
        Io(String, std::io::Error),
    }
    impl Error {
        fn string_name(&self) -> &'static str {
            match self {
                Self::Serialize(..) => "Serialize",
                Self::Deserialize(..) => "Deserialize",
                Self::Config(..) => "Config",
                Self::ConfigParse(..) => "ConfigParse",
                Self::UserVarReplace(..) => "UserVarReplace",
                Self::Raw(..) => "Raw",
                Self::Io(..) => "Io",
            }
        }
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
            if !matches!(self, Error::Raw(_)) {
                f.write_str(self.string_name())?;
                f.write_str(" Error: ")?;
            }

            match self {
                Self::Serialize(s, e) => {
                    f.write_str(s)?;
                    e.fmt(f)
                }
                Self::Deserialize(s, e) => {
                    f.write_str(s)?;
                    e.fmt(f)
                }
                Self::Config(e) => e.fmt(f),
                Self::ConfigParse(s, e) => {
                    f.write_str(s)?;
                    e.fmt(f)
                }
                Self::UserVarReplace(s, e) => {
                    f.write_str(s)?;
                    e.fmt(f)
                }
                Self::Raw(e) => e.fmt(f),
                Self::Io(s, e) => {
                    f.write_str(s)?;
                    e.fmt(f)
                }
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
struct Output {
    string: String,
    test: bool,
}

fn value_to_string(val: &toml::Value, quote_strings: bool) -> errors::Result<String> {
    use toml::Value as v;
    Ok(match val {
        v::String(s) => {
            if quote_strings {
                format!("\"{}\"", s)
            } else {
                s.to_string()
            }
        }
        v::Integer(s) => s.to_string(),
        v::Float(s) => s.to_string(),
        v::Boolean(s) => s.to_string(),
        v::Datetime(s) => s.to_string(),
        v::Array(s) => {
            let mut str = s.iter().try_fold(String::from("["), |acc, v| {
                Ok(acc + &value_to_string(v, true)? + ",")
            })?;
            str.pop();
            str + "]"
        }
        v::Table(_) => Err(errors::new_config(
            "invalid 'vars': 'vars' is not allowed to be a table".into(),
        ))?,
    })
}

fn build_command(args: BuildCmdArgs) -> errors::Result<String> {
    use constants::{SUB_DIR, SUB_LANG, SUB_NAME, SUB_PATH};

    let base_path_str = args.base_dir.to_string_lossy();
    let full_path = {
        let mut path = args.base_dir.join(args.lang);
        path.push(args.name);
        path.to_string_lossy().to_string()
    };
    let vars = args
        .vars
        .iter()
        .map(|(k, v)| -> Result<(String, String), errors::Error> {
            Ok((format!("%{{{}}}", k), value_to_string(v, false)?))
        })
        .collect::<Result<Vec<(String, String)>, _>>()?;

    let (search, replace) = [
        (SUB_LANG, args.lang),
        (SUB_NAME, args.name),
        (SUB_DIR, &base_path_str),
        (SUB_PATH, &full_path),
    ]
    .into_iter()
    .chain(vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
    .collect::<(Vec<&str>, Vec<&str>)>();

    let patterns = aho_corasick::AhoCorasick::new(search).map_err(|e| {
        errors::new_user_var_replace("building search for user provided variables: ".into(), e)
    })?;
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

fn get_output_str() -> errors::Result<Output> {
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

    Ok(Output {
        string: result,
        test: args.test,
    })
}

pub fn run() {
    match get_output_str() {
        Ok(Output {
            string,
            test: false,
        }) => {
            print!("{}", format!("EXEC::{}", string));
        }
        Ok(Output { string, test: true }) => {
            print!("{}", format!("EXEC::echo '{}'", string.escape_default()));
        }
        Err(e) => eprint!("{}", e.to_string()),
    };
}
