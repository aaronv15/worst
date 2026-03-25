use std::{collections::HashMap, io::Write, path::PathBuf};

use crate::{
    args::Pargs,
    files::{Config, ConfigKey, State},
};

mod args;
mod files;

pub(crate) mod constants {
    use constcat::concat as constcat;
    use std::sync::LazyLock;

    // Application folder name
    pub(crate) const APP_NAME: &str = "worst-switcher";
    // Config file name
    pub(crate) const CONFIG_NAME: &str = "config.toml";
    // State file (to be renamed data file)
    pub(crate) const STATE_FILE: &str = "state.cbor";

    // Substitutions
    // Language name
    pub(crate) const SUB_LANG: &str = "%{lang}";
    // Project name
    pub(crate) const SUB_NAME: &str = "%{name}";
    // Directory that project resides in
    pub(crate) const SUB_DIR: &str = "%{base_dir}";
    // Full path to project root
    pub(crate) const SUB_PATH: &str = "%{path}";

    // Defaults
    pub(crate) const NEW_DEFAULT: &str = constcat!("mkdir '", SUB_PATH, "'");
    pub(crate) const GO_DEFAULT: &str = constcat!("cd '", SUB_PATH, "'");
    pub(crate) const OPEN_DEFAULT: &str = "nvim .";
    pub(crate) const BASE_DIR_DEFAULT: LazyLock<String> =
        LazyLock::new(|| std::env::var("HOME").unwrap() + "/Documents");
}

pub(crate) mod errors {
    //! Contains all errors raised by the program as well as a Result type

    // Result type
    pub type Result<T> = core::result::Result<T, Error>;

    // Serialisation errors aliases
    type SerErr = ciborium::ser::Error<std::io::Error>;
    type DeErr = ciborium::de::Error<std::io::Error>;

    /// Create a new `Serialize` error. See Errors for usage
    pub fn new_ser(s: String, e: SerErr) -> Error {
        Error::Serialize(s, e)
    }
    /// Create a new `Deserialize` error. See Errors for usage
    pub fn new_de(s: String, e: DeErr) -> Error {
        Error::Deserialize(s, e)
    }
    /// Create a new `Config` error. See Errors for usage
    pub fn new_config(s: String) -> Error {
        Error::Config(s)
    }
    /// Create a new `ConfigParse` error. See Errors for usage
    pub fn new_config_parse(s: String, e: toml::de::Error) -> Error {
        Error::ConfigParse(s, e)
    }
    /// Create a new `UserVarReplace` error. See Errors for usage
    pub fn new_user_var_replace(s: String, e: aho_corasick::BuildError) -> Error {
        Error::UserVarReplace(s, e)
    }
    /// Create a new `Raw` error. See Errors for usage
    pub fn new_raw(s: String) -> Error {
        Error::Raw(s)
    }
    /// Create a new `Io` error. See Errors for usage
    pub fn new_io(s: String, e: std::io::Error) -> Error {
        Error::Io(s, e)
    }

    pub enum Error {
        /// Serialising data
        Serialize(String, SerErr),
        /// Deserialising data
        Deserialize(String, DeErr),
        /// Invalid config values
        Config(String),
        /// Config parsing errors
        ConfigParse(String, toml::de::Error),
        /// Invalid user provided variables
        UserVarReplace(String, aho_corasick::BuildError),
        /// Io errors
        Io(String, std::io::Error),
        /// Used for printing just the error message and nothing else
        Raw(String),
    }
    impl Error {
        /// Get enum member as literal string
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
        /// Formats error as  "{Error name} Error: {Error string}{(Contained Error).fmt()}" unless
        /// error is of type Error::Raw, in which case error is formatted as "{Error string}"
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
}

/// Args for build_command function
struct BuildCmdArgs<'a> {
    // Project language
    lang: &'a str,
    // Project name
    name: &'a str,
    // Command to run
    cmd: &'a str,
    // User variables
    vars: &'a HashMap<String, toml::Value>,
    // Language directory
    base_dir: &'a PathBuf,
}
/// Contains all information needed to update state
struct UpdateState {
    lang: String,
    name: String,
    base_dir: PathBuf,
}
/// Output string and if this is a test run
struct Output {
    string: String,
    test: bool,
}

/// Turn a toml::Value into a String provided it is not a toml::Table and does not contain a
/// toml::Table. If 'quote_strings' is true, this will return a toml::String surrounded by
/// additional quotes.
///
/// # Errors:
/// Returns Error::Config if 'val' is/contains a toml::Table
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

/// Build the final output command, combining paths, substituting magic variables, and substituting
/// user variables
///
/// # Errors:
/// Returns Error::Config if a user variable is/contains a toml::Table
/// Returns Error::UserVarReplace if a variable is named in such a
/// way that the aho_corasick crate cannot parse it
fn build_command(args: BuildCmdArgs) -> errors::Result<String> {
    use constants::{SUB_DIR, SUB_LANG, SUB_NAME, SUB_PATH};

    let base_path_str = args.base_dir.to_string_lossy();
    // Build full path
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

/// Returns values need to update state, and output string for the 'go' subcommand
fn get_go_cmd_build_cmd_args<'a>(
    cfg: &'a Config,
    get_cfg_cmd: impl FnOnce(&'a Config, &ConfigKey) -> &'a str,
    st: &mut State,
    cmd: args::Go,
) -> errors::Result<(UpdateState, String)> {
    // get language, project name. Language, Project name, or both, may be None
    let (lang, name) = cmd.get_lang_name();
    let build_cmd_args: BuildCmdArgs = match (lang, name) {
        // Both have been supplied. build key and get information from config
        (Some(lang), Some(name)) => {
            // Key is used to get appropriate language instruction values
            let key = &ConfigKey::new(lang, cmd.var.as_ref());

            BuildCmdArgs {
                lang,
                name,
                cmd: get_cfg_cmd(cfg, key),
                vars: cfg.user_vars(key),
                base_dir: cfg.base_dir(key),
            }
        }
        // Only language has been supplied. We can build the key, but also need to get project name
        // from state
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
        // Only project name has been supplied. Get language from state and then build key
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
        // Nothing has been supplied. Use last project from state
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

/// Returns values need to update state, and output string for the 'go' subcommand
fn handle_go(cfg: &Config, st: &mut State, cmd: args::Go) -> errors::Result<(UpdateState, String)> {
    get_go_cmd_build_cmd_args(cfg, Config::go_cmd, st, cmd)
}

/// Returns values need to update state, and output string for the 'open' subcommand
fn handle_open(
    cfg: &Config,
    st: &mut State,
    cmd: args::Go,
) -> errors::Result<(UpdateState, String)> {
    get_go_cmd_build_cmd_args(cfg, Config::open_cmd, st, cmd)
}

/// Returns values need to update state, and output string for the 'new' subcommand
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

/// Returns values need to update state, and output string for the 'go-new' subcommand
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

/// Returns values need to update state, and output string for the 'open-new' subcommand
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

/// Handles setup
fn handle_setup(setup: crate::args::Setup) -> crate::errors::Result<crate::errors::Error> {
    use crate::errors::{new_io, new_raw};

    if setup.place_config {
        let xdg_dirs = Pargs::new_xdg_with_prefix();
        let config_file = Pargs::xdg_config_path(&xdg_dirs)?;

        // If this fails, we'll know later when tryin to create the file, and we don't care if it
        // fails if the directory already exists
        _ = std::fs::create_dir(
            config_file
                .parent()
                .ok_or(new_raw("Config path does not have a parent?".into()))?,
        );

        match std::fs::File::create_new(&config_file) {
            Err(e) => {
                if matches!(e.kind(), std::io::ErrorKind::AlreadyExists) {
                    Err(new_io(
                        "Not creating config as it already exists: ".into(),
                        e,
                    ))?
                } else {
                    Err(new_io("creating config file: ".into(), e))?
                }
            }
            Ok(mut file) => {
                write!(file, "").map_err(|e| new_io("writing config contents: ".into(), e))?;
                println!("Created config file {config_file:?}");
            }
        }
    }

    let current_executable =
        std::env::current_exe().map_err(|e| new_io("getting path to self: ".into(), e))?;
    let executable_path = setup
        .with_executable
        .unwrap_or(current_executable.to_string_lossy().into());

    let shell_script_path = match &setup.generate_shell_script {
        Some(Some(path)) => path,
        _ => &current_executable
            .parent()
            .ok_or(new_raw("Why is this root?".into()))?
            .join("worst.bash"),
    };

    let shell_script = format!(
        r###"bin_path="{executable_path}"
result=$("$bin_path" $*)
if [ -n "$result" ] && [ -z "${{result##EXEC::*}}" ]; then
   eval "${{result#EXEC::}}"
else
   echo "$result" 
fi"###
    );

    if setup.generate_shell_script.is_some() {
        let mut fs = std::fs::File::create_new(shell_script_path)
            .map_err(|e| new_io("creating shell script: ".into(), e))?;
        write!(fs, "{shell_script}").map_err(|e| new_io("writing shell script: ".into(), e))?;
        println!("Created script at {shell_script_path:?}");
    }

    if setup.print_shell_script {
        println!("{shell_script}");
    }

    Ok(new_raw("Setup complete".into()))
}

/// Builds output string based on subcommand specified
fn get_output_str() -> errors::Result<Output> {
    use crate::args::Command;

    let cli = Pargs::parse();
    if let Command::Setup(setup) = cli.command {
        return Err(handle_setup(setup)?);
    }

    let args = Pargs::new(cli)?;

    let conf = Config::new(args.config)?;
    let mut state = State::de(args.state)?;

    let (update_state, result) = {
        use Command::*;
        match args.command {
            Go(cmd) => handle_go(&conf, &mut state, cmd),
            Open(cmd) => handle_open(&conf, &mut state, cmd),
            New(cmd) => handle_new(&conf, &mut state, &cmd),
            GoNew(cmd) => handle_go_new(&conf, &mut state, cmd),
            OpenNew(cmd) => handle_open_new(&conf, &mut state, cmd),
            Setup(_) => unreachable!("handled above"),
        }
    }?;

    // Update state
    state.insert(update_state.name, update_state.lang, update_state.base_dir);
    // Serialize state
    state.ser()?;

    Ok(Output {
        string: result,
        test: args.test,
    })
}

/// Run worst project switcher, and returns a string as a shell command to execute
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
        Err(e) => eprintln!("{}", e.to_string()),
    };
}
