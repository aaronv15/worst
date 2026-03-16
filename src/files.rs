use constcat::concat as constcat;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::constants as sub;
use crate::errors as err;

pub struct ConfigKey<'a> {
    language: &'a str,
    shell: String,
    platform: String,
}
impl<'a> ConfigKey<'a> {
    pub fn new(language: &'a str, shell: String, platform: String) -> Self {
        Self {
            language,
            shell,
            platform,
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    pub base_dir: Option<PathBuf>,
    pub go_cmd: Option<String>,
    pub new_cmd: Option<String>,
    pub open_cmd: Option<String>,

    pub name: Option<String>,

    pub table: HashMap<String, Config>,
    pub user_vars: HashMap<String, toml::Value>,
}
impl Config {
    // Removes a string value from a toml::Table, or returns an error if the value does not exist or
    // is not a String.
    fn get_str_value(table: &mut toml::Table, key: &str) -> err::Result<Option<String>> {
        match table.remove(key) {
            Some(toml::Value::String(s)) => Ok(Some(s)),
            None => Ok(None),
            Some(val) => Err(err::Error::Config(format!(
                "Malformed Config: Invalid type {} for key {}",
                val.type_str(),
                key
            ))),
        }
    }

    fn from_table(table: &mut toml::Table) -> err::Result<Self> {
        Ok(Self {
            base_dir: Self::get_str_value(table, "base_dir")?.map(|s| PathBuf::from(s)),
            go_cmd: Self::get_str_value(table, "go_cmd")?,
            new_cmd: Self::get_str_value(table, "new_cmd")?,
            open_cmd: Self::get_str_value(table, "open_cmd")?,
            name: Self::get_str_value(table, "name")?,
            user_vars: table.remove("vars").map_or_else(
                || Ok(HashMap::new()),
                |v| match v {
                    toml::Value::Table(t) => Ok(t.into_iter().collect()),
                    _ => Err(err::Error::Config("vars must be a table".into())),
                },
            )?,
            table: table
                .into_iter()
                .map(|(k, v)| match v {
                    toml::Value::Table(v) => Ok((k.to_string(), Self::from_table(v)?)),
                    _ => Err(err::Error::Config(format!("Unkown key {} in config", k))),
                })
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn new(path: PathBuf) -> err::Result<Self> {
        let config = std::fs::read_to_string(&path)?;
        let table: Result<toml::Table, _> = config.parse();

        match table {
            Ok(mut table) => Self::from_table(&mut table),
            Err(e) => Err(err::Error::ConfigParse(e)),
        }
    }

    pub fn new_config_key(&self, lang: &str) -> ConfigKey {
        // ConfigKey::new(lang, )
    }
}
impl Config {
    pub const NEW_DEFAULT: &str = constcat!("mkdir ", sub::SUB_DIR);
    pub const GO_DEFAULT: &str = constcat!("cd ", sub::SUB_DIR);
    pub const OPEN_DEFAULT: &str = "nvim .";

    pub fn get_config(&self, key: &ConfigKey) -> Option<&Config> {
        let mut keys = vec![key.language, key.shell, key.platform];
        let mut config = {
            let mut v = Vec::with_capacity(3);
            v.push(self);
            v
        };
        while let Some(key) = keys.first() {
            let top = config.pop().unwrap_or(self);

            if let Some(conf) = top.table.get(*key) {
                keys.pop();
                config.push(top);
                config.push(conf);
            } else if config.len() == 0 {
                keys.pop();
                config.push(top);
            }
        }

        config.pop()
    }

    pub fn base_dir(&self, key: &ConfigKey) -> Option<&PathBuf> {
        self.get_config(key).unwrap_or(self).base_dir.as_ref()
    }

    pub fn go_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .unwrap_or(self)
            .go_cmd
            .as_ref()
            .map_or(Self::GO_DEFAULT, |s| &s)
    }

    pub fn new_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .unwrap_or(self)
            .new_cmd
            .as_ref()
            .map_or(Self::NEW_DEFAULT, |s| &s)
    }

    pub fn open_cmd(&self, key: &ConfigKey) -> &str {
        self.get_config(key)
            .unwrap_or(self)
            .open_cmd
            .as_ref()
            .map_or(Self::OPEN_DEFAULT, |s| &s)
    }

    pub fn name(&self, key: &ConfigKey) -> Option<&String> {
        self.get_config(key).and_then(|c| c.name.as_ref())
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProjStateObj {
    pub proj: String,
    pub language: String,
    pub base_dir: std::ffi::OsString,
}

pub struct State {
    path: PathBuf,
    projects: Vec<ProjStateObj>,
}

impl State {
    /// # Errors:
    /// Raises a Error::Deserialize on deserialisation failure
    /// Raises an Error::Io on failure to read from file (this will not get thrown if the file does
    /// not exist)
    pub fn de(path: PathBuf) -> err::Result<State> {
        let projects = if let Ok(fd) = std::fs::File::open(&path) {
            ciborium::de::from_reader(fd)?
        } else {
            vec![]
        };

        Ok(Self { path, projects })
    }

    /// # Errors:
    /// Raises an Error::Io on failure to write to file
    /// Raises an Error::Serialize on serialisation failure
    pub fn ser(&self) -> err::Result<()> {
        ciborium::into_writer(
            &self.projects,
            std::fs::OpenOptions::new().write(true).open(&self.path)?,
        );
        Ok(())
    }

    pub fn insert(&mut self, proj: String, language: String, base_dir: PathBuf) {
        if let Some(index) = self
            .projects
            .iter()
            .position(|p| p.proj == proj && p.language == language)
        {
            if index != 0 {
                self.projects[0..index].rotate_right(1);
            }
        } else {
            self.projects.insert(
                0,
                ProjStateObj {
                    proj,
                    language,
                    base_dir: base_dir.into_os_string(),
                },
            )
        }
    }

    pub fn latest(&self) -> Option<&ProjStateObj> {
        self.projects.last()
    }

    pub fn latest_by_lang(&self, language: &str) -> Option<&ProjStateObj> {
        self.projects.iter().find(|p| language == &p.language)
    }

    pub fn latest_by_name(&self, name: &str) -> Option<&ProjStateObj> {
        self.projects.iter().find(|p| name == &p.proj)
    }
}
