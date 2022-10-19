use std::{
    fs::File,
    io::Read,
    path::Path,
};

use super::{Config, EnvVars};

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
        let mut conf_file = File::options()
            .create(true)
            .write(true)
            .read(true)
            .open(Path::new(&env.config))
            .unwrap();
        let mut buf = Vec::new();
        conf_file.read_to_end(&mut buf).unwrap();

        match serde_yaml::from_slice::<Config>(&buf) {
            Ok(mut v) => {
                v.parse();
                v
            }
            Err(_) => Config::default(),
        }
    }
}
