fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Range {:?}", 10..1);
    // proj::run()
}

// use std::{collections::HashMap, path::PathBuf};
//
// use proj::args;
//
// pub struct ConfigKey<'a> {
//     language: &'a str,
//     shell: &'a str,
//     platform: &'a str,
// }
// impl<'a> ConfigKey<'a> {
//     pub fn new(language: &'a str, shell: &'a str, platform: &'a str) -> Self {
//         Self {
//             language,
//             shell,
//             platform,
//         }
//     }
// }
//
// pub struct Config {
//     pub base_dir: Option<PathBuf>,
//     pub go_cmd: Option<String>,
//     pub new_cmd: Option<String>,
//     pub open_cmd: Option<String>,
//
//     pub name: Option<String>,
//
//     pub table: HashMap<String, Config>,
//     pub user_defined: HashMap<String, String>,
// }
// impl Config {
//     pub const NEW_DEFAULT: &str = constcat!("mkdir ", sub::SUB_DIR);
//     pub const GO_DEFAULT: &str = constcat!("cd ", sub::SUB_DIR);
//     pub const OPEN_DEFAULT: &str = "nvim .";
//
//     pub fn get_config(&self, key: &ConfigKey) -> Option<&Config> {
//         let mut keys = vec![key.language, key.shell, key.platform];
//         let mut config = {
//             let mut v = Vec::with_capacity(3);
//             v.push(self);
//             v
//         };
//         while let Some(key) = keys.first() {
//             let top = config.pop().unwrap_or(self);
//
//             if let Some(conf) = top.table.get(*key) {
//                 keys.pop();
//                 config.push(top);
//                 config.push(conf);
//             } else if config.len() == 0 {
//                 keys.pop();
//                 config.push(top);
//             }
//         }
//
//         config.pop()
//     }
//
//     pub fn base_dir(&self, key: &ConfigKey) -> Option<&PathBuf> {
//         self.get_config(key).unwrap_or(self).base_dir.as_ref()
//     }
//
//     pub fn go_cmd(&self, key: &ConfigKey) -> &str {
//         self.get_config(key)
//             .unwrap_or(self)
//             .go_cmd
//             .as_ref()
//             .map_or(Self::GO_DEFAULT, |s| &s)
//     }
//
//     pub fn new_cmd(&self, key: &ConfigKey) -> &str {
//         self.get_config(key)
//             .unwrap_or(self)
//             .new_cmd
//             .as_ref()
//             .map_or(Self::NEW_DEFAULT, |s| &s)
//     }
//
//     pub fn open_cmd(&self, key: &ConfigKey) -> &str {
//         self.get_config(key)
//             .unwrap_or(self)
//             .open_cmd
//             .as_ref()
//             .map_or(Self::OPEN_DEFAULT, |s| &s)
//     }
//
//     pub fn name(&self, key: &ConfigKey) -> Option<&String> {
//         self.get_config(key).and_then(|c| c.name.as_ref())
//     }
// }
