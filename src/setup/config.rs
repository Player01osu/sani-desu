use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use serde::{Deserialize, Serialize};

use super::{get_env, Config, EnvVars};

impl Config {
    fn parse(&mut self) {
        if let Some(v) = &self.dmenu_settings.font {
            if v.is_empty() {
                self.dmenu_settings.font = None;
            }
        }

        if let Some(v) = &self.dmenu_settings.sel_fg {
            if v.is_empty() {
                self.dmenu_settings.sel_fg = None;
            }
        }

        if let Some(v) = &self.dmenu_settings.sel_bg {
            if v.is_empty() {
                self.dmenu_settings.sel_bg = None;
            }
        }

        if let Some(v) = &self.dmenu_settings.norm_fg {
            if v.is_empty() {
                self.dmenu_settings.norm_fg = None;
            }
        }

        if let Some(v) = &self.dmenu_settings.norm_bg {
            if v.is_empty() {
                self.dmenu_settings.norm_bg = None;
            }
        }
    }
    pub fn generate(env: &EnvVars) -> Self {
        dbg!(&env.config);
        let mut conf_file = File::options()
            .create(true)
            .write(true)
            .read(true)
            .open(Path::new(&env.config))
            .unwrap();
        let mut buf = Vec::new();
        conf_file.read_to_end(&mut buf).unwrap();

        let d = match serde_yaml::from_slice::<Config>(&mut buf) {
            Ok(mut v) => {
                v.parse();
                v
            },
            Err(_) => {
                Config::default()
            }
        };
        dbg!(&d);
        d
    }
}

