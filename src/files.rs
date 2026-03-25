use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::constants::{BASE_DIR_DEFAULT, GO_DEFAULT, NEW_DEFAULT, OPEN_DEFAULT};
use crate::errors as err;

/// Used to get a specific `Config` from the config file
pub struct ConfigKey<'a> {
    language: &'a str,
    variant: Option<&'a String>,
}
impl<'a> ConfigKey<'a> {
    pub fn new(language: &'a str, variant: Option<&'a String>) -> Self {
        Self { language, variant }
    }
}

#[derive(Debug)]
pub struct Config {
    // Root directory that all langauge folders get put in
    base_dir: Option<PathBuf>,
    // Shell script to execute when the 'go' subcommand is invoked
    go_cmd: Option<String>,
    // Shell script to execute when the 'new' subcommand is invoked
    new_cmd: Option<String>,
    // Shell script to execute when the 'open' subcommand is invoked
    open_cmd: Option<String>,

    // The table of sub-configs mapping a table name to a config.
    // For the root TOML table, this is a map of either version key to config, or language name to
    // config
    // For version keys, this is a map of language name to config
    // For languages, this is an empty hashmap.
    table: HashMap<String, Config>,
    // A hashmap of user variables to TOML values. these variables are visible only in the table
    // that they are defined, and not inherited by children as the other config values are.
    user_vars: HashMap<String, toml::Value>,
}
impl Config {
    // Removes a string value from a toml::Table, or returns an error if the value does not exist or
    // is not a String.
    fn get_str_value(table: &mut toml::Table, key: &str) -> err::Result<Option<String>> {
        match table.remove(key) {
            Some(toml::Value::String(s)) => Ok(Some(s)),
            None => Ok(None),
            Some(val) => Err(err::new_config(format!(
                "Malformed Config: Invalid type {} for key {}",
                val.type_str(),
                key
            ))),
        }
    }

    // Builds a new config from a toml::Table
    // # Errors:
    // will return a crate::errors::Error if keys that are expected to be strings exist and are not
    // strings, if the 'vars' key exists and does not map to a toml::Table, and if there are any
    // unknown keys that do not map to a toml::Table
    fn from_table(table: &mut toml::Table) -> err::Result<Self> {
        Ok(Self {
            base_dir: Self::get_str_value(table, "base_dir")?.map(|s| PathBuf::from(s)),
            go_cmd: Self::get_str_value(table, "go")?,
            new_cmd: Self::get_str_value(table, "new")?,
            open_cmd: Self::get_str_value(table, "open")?,
            user_vars: table.remove("vars").map_or_else(
                || Ok(HashMap::new()),
                |v| match v {
                    toml::Value::Table(t) => Ok(t.into_iter().collect()),
                    _ => Err(err::new_config(
                        "parsing config: 'vars' must be a table".into(),
                    )),
                },
            )?,
            // Map remaining keys to configs or raise an error if that cannot be done
            table: table
                .into_iter()
                .map(|(k, v)| match v {
                    toml::Value::Table(t) => Ok((k.to_string(), Self::from_table(t)?)),
                    _ => Err(err::new_config(format!(
                        "parsing config: Unkown key '{}' in config",
                        k
                    ))),
                })
                .collect::<Result<_, _>>()?,
        })
    }

    /// Returns a new `Config` corresponding to the config file at `path`
    /// # Errors:
    /// // type E = crate::errors::Error;
    ///
    /// Returns an E::Io(String, io::Error) if the path cannot be read
    /// Returns an E::ConfigParse(String, toml::de::Error) if the TOML parser cannot parse the config
    /// Returns an E::Config(String) if the provided config is invalid
    pub fn new(path: PathBuf) -> err::Result<Self> {
        let config = std::fs::read_to_string(&path)
            .map_err(|e| err::new_io("reading config: ".into(), e))?;
        let table: Result<toml::Table, _> = config.parse();

        match table {
            Ok(mut table) => Self::from_table(&mut table),
            Err(e) => Err(err::new_config_parse("parsing config toml: ".into(), e)),
        }
    }
}
impl Config {
    /// Retreives the config associatied with the provided key, or returns None.
    /// The resolution works as follows:
    /// If key has a variant set:
    ///     if config = self[key.variant]:
    ///         if config = config[key.lang]:
    ///             return config
    /// If config = self[key.lang]:
    ///     return config
    /// return None
    pub fn get_config(&self, key: &ConfigKey) -> Option<&Config> {
        // set up key stack
        let mut keys = {
            let mut v = Vec::with_capacity(2);
            v.push(key.language);
            key.variant.map(|var| v.push(var));
            v
        };
        // set up config stack
        let mut config = {
            let mut v = Vec::with_capacity(keys.len());
            v.push(self);
            v
        };

        // while there are keys on the stack
        while let Some(key) = keys.last() {
            let top = *config.last().unwrap_or(&self);

            // If the config at the top of the stack contains the current key,
            // pop the current key, we have found the associated table
            // push the retreived config onto the stack
            // like so [configs] <- retreived
            //         [keys] -> current key
            if let Some(conf) = top.table.get(*key) {
                keys.pop();
                config.push(conf);
            }
            // Otherwise, pop the current config to search the parent config
            else {
                config.pop();
            }
            // If we are at the root config and we cannot find the current key, pop it and search
            // for the next key
            if config.len() == 0 {
                keys.pop();
            }
        }

        // return the config at the top of the stack
        config.pop()
    }

    /// Return base_dir from config or default
    /// Lookup is a follows:
    /// base_dir = if config = self.get_config(key) {
    ///     config.base_dir
    /// } else {
    ///     self.base_dir
    /// };
    /// return base_dir or default
    pub fn base_dir(&self, key: &ConfigKey) -> &PathBuf {
        use std::sync::LazyLock;

        static BASE_DIR_DEF: LazyLock<PathBuf> =
            LazyLock::new(|| PathBuf::from(&*BASE_DIR_DEFAULT));

        self.get_config(key)
            .and_then(|c| c.base_dir.as_ref())
            .or(self.base_dir.as_ref())
            .unwrap_or(&*BASE_DIR_DEF)

        // .canonicalize()
        // .map_err(|e| err::new_io("resolving path to base_dir: ".into(), e))
    }

    /// Return go_cmd from config or default
    /// Lookup is a follows:
    /// go_cmd = if config = self.get_config(key) {
    ///     config.go_cmd
    /// } else {
    ///     self.go_cmd
    /// };
    /// return go_cmd or default
    pub fn go_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .and_then(|c| c.go_cmd.as_ref())
            .or(self.go_cmd.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(GO_DEFAULT)
    }

    /// Return new_cmd from config or default
    /// Lookup is a follows:
    /// new_cmd = if config = self.get_config(key) {
    ///     config.new_cmd
    /// } else {
    ///     self.new_cmd
    /// };
    /// return new_cmd or default
    pub fn new_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .and_then(|c| c.new_cmd.as_ref())
            .or(self.new_cmd.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(NEW_DEFAULT)
        // .and_then(|c| c.new_cmd.as_ref().map(String::as_str))
        // .unwrap_or_else(|| self.new_cmd.as_ref().map_or(sub::NEW_DEFAULT, |s| &s))
    }

    /// Return open_cmd from config or default
    /// Lookup is a follows:
    /// open_cmd = if config = self.get_config(key) {
    ///     config.open_cmd
    /// } else {
    ///     self.open_cmd
    /// };
    /// return open_cmd or default
    pub fn open_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .and_then(|c| c.open_cmd.as_ref())
            .or(self.open_cmd.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(OPEN_DEFAULT)
    }

    pub fn user_vars(&self, key: &ConfigKey) -> &HashMap<String, toml::Value> {
        &self.get_config(key).unwrap_or(self).user_vars
    }
}

/// Represents a project's location. // base_dir/language/proj
#[derive(Serialize, Deserialize)]
pub struct ProjStateObj {
    /// Project name
    pub proj: String,
    /// Language directory
    pub language: String,
    /// Path to parent dir of `language`
    pub base_dir: PathBuf,
}

/// Stores all known projects
pub struct State {
    path: PathBuf,
    projects: Vec<ProjStateObj>,
}

impl State {
    /// Create a new State object and populate with data deserialized from the file pointed to by
    /// `path`
    /// # Errors:
    /// Raises a Error::Deserialize on deserialisation failure
    /// Raises an Error::Io on failure to read from file (this will not get thrown if the file does
    /// not exist)
    pub fn de(path: PathBuf) -> err::Result<State> {
        let projects = if let Ok(fd) = std::fs::File::open(&path) {
            ciborium::de::from_reader(fd)
                .map_err(|e| err::new_de("deserializing state file: ".into(), e))?
        } else {
            vec![]
        };

        Ok(Self { path, projects })
    }

    /// Serialize internal state in `self.path`
    /// # Errors:
    /// Raises an Error::Io on failure to write to file
    /// Raises an Error::Serialize on serialisation failure
    pub fn ser(&self) -> err::Result<()> {
        ciborium::into_writer(
            &self.projects,
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&self.path)
                .map_err(|e| err::new_io("opening state file: ".into(), e))?,
        )
        .map_err(|e| err::new_ser("serializing state: ".into(), e))?;
        Ok(())
    }

    /// Insert a new record or move an existing record to the start of the project list
    pub fn insert(&mut self, proj: String, language: String, base_dir: PathBuf) {
        if let Some(index) = self
            .projects
            .iter()
            .position(|p| p.proj == proj && p.language == language)
        {
            if index != 0 {
                self.projects[0..=index].rotate_right(1);
            }
        } else {
            self.projects.insert(
                0,
                ProjStateObj {
                    proj,
                    language,
                    base_dir,
                },
            )
        }
    }

    /// Return the latest project accessed or None if there are none
    pub fn latest(&self) -> Option<&ProjStateObj> {
        self.projects.first()
    }

    /// Return the last project of language `lang` accessed or None if project does not exist
    pub fn latest_by_lang(&self, language: &str) -> Option<&ProjStateObj> {
        self.projects.iter().find(|p| language == &p.language)
    }

    /// Return the last project accessed called `name` or None if prject does not exist
    pub fn latest_by_name(&self, name: &str) -> Option<&ProjStateObj> {
        self.projects.iter().find(|p| name == &p.proj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::STATE_FILE;

    fn config_from_valid_table() -> Config {
        let mut table: toml::Table = r#"
            base_dir = "/root/Documents"
            new = "root-new"

            [rust]
            go = "rust-go"
            open = "rust-open"

            [junk.rust]
            base_dir = "/junk/Documents"
            new = "junk-new"

            [python]
            vars.env = ".env"
            new = "python-new"
        "#
        .parse()
        .unwrap();

        Config::from_table(&mut table).unwrap()
    }

    #[test]
    fn get_config_basic_language() {
        let cfg = config_from_valid_table();

        let key = ConfigKey::new("rust", None);
        let go = cfg.go_cmd(&key);

        assert_eq!(go, "rust-go");
    }

    #[test]
    fn get_config_nested_variant() {
        let cfg = config_from_valid_table();

        let variant = "junk".to_string();
        let key = ConfigKey::new("rust", Some(&variant));

        let new_cmd = cfg.new_cmd(&key);
        let base_dir = cfg.base_dir(&key);

        assert_eq!(new_cmd, "junk-new");
        assert_eq!(base_dir, &PathBuf::from("/junk/Documents"));
    }

    #[test]
    fn fallback_to_parent_config() {
        let cfg = config_from_valid_table();

        let key = ConfigKey::new("rust", None);

        // rust has no `new`, should fallback to root
        assert_eq!(cfg.new_cmd(&key), "root-new");
    }

    #[test]
    fn fallback_to_defaults() {
        let cfg = config_from_valid_table();

        let key = ConfigKey::new("nonexistent", None);

        assert_eq!(cfg.go_cmd(&key), GO_DEFAULT);
        assert_eq!(cfg.open_cmd(&key), OPEN_DEFAULT);
    }

    #[test]
    fn user_vars_are_loaded() {
        let cfg = config_from_valid_table();

        let key = ConfigKey::new("python", None);
        let vars = cfg.user_vars(&key);

        assert_eq!(vars.get("env").unwrap().as_str(), Some(".env"));
    }

    // ---------------- STATE TESTS ----------------

    #[test]
    fn state_insert_new_goes_to_front() {
        let mut state = State {
            path: PathBuf::new(),
            projects: vec![ProjStateObj {
                proj: "a".into(),
                language: "rust".into(),
                base_dir: PathBuf::from("/a"),
            }],
        };

        state.insert("proj1".into(), "rust".into(), PathBuf::from("/a"));

        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.projects[0].proj, "proj1");
    }

    #[test]
    fn state_insert_existing_moves_to_front() {
        let mut state = State {
            path: PathBuf::new(),
            projects: vec![
                ProjStateObj {
                    proj: "a".into(),
                    language: "rust".into(),
                    base_dir: PathBuf::from("/a"),
                },
                ProjStateObj {
                    proj: "b".into(),
                    language: "rust".into(),
                    base_dir: PathBuf::from("/b"),
                },
            ],
        };

        state.insert("b".into(), "rust".into(), PathBuf::from("/b"));

        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.projects[0].proj, "b");
    }

    #[test]
    fn latest_returns_first() {
        let state = State {
            path: PathBuf::new(),
            projects: vec![
                ProjStateObj {
                    proj: "a".into(),
                    language: "rust".into(),
                    base_dir: PathBuf::from("/a"),
                },
                ProjStateObj {
                    proj: "b".into(),
                    language: "python".into(),
                    base_dir: PathBuf::from("/b"),
                },
            ],
        };

        assert_eq!(state.latest().unwrap().proj, "a");
    }

    #[test]
    fn latest_by_lang() {
        let state = State {
            path: PathBuf::new(),
            projects: vec![
                ProjStateObj {
                    proj: "a".into(),
                    language: "rust".into(),
                    base_dir: PathBuf::from("/a"),
                },
                ProjStateObj {
                    proj: "b".into(),
                    language: "python".into(),
                    base_dir: PathBuf::from("/b"),
                },
            ],
        };

        assert_eq!(state.latest_by_lang("python").unwrap().proj, "b");
    }

    #[test]
    fn latest_by_name() {
        let state = State {
            path: PathBuf::new(),
            projects: vec![
                ProjStateObj {
                    proj: "a".into(),
                    language: "rust".into(),
                    base_dir: PathBuf::from("/a"),
                },
                ProjStateObj {
                    proj: "b".into(),
                    language: "python".into(),
                    base_dir: PathBuf::from("/b"),
                },
            ],
        };

        assert_eq!(state.latest_by_name("a").unwrap().language, "rust");
    }

    #[test]
    fn state_roundtrip_ser_de() {
        let tmp = tempfile::tempdir().expect("creating temp dir");
        let state_file = tmp.path().join(STATE_FILE);

        let mut state = State {
            path: state_file,
            projects: vec![],
        };

        state.insert("proj".into(), "rust".into(), PathBuf::from("/x"));
        state.ser().expect("serialize");

        let loaded = State::de(state.path).unwrap();

        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].proj, "proj");
    }
}
