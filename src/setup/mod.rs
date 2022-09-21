use std::env;

use serde::{Deserialize, Serialize};

pub mod config;
pub mod environment;

pub mod cache {
}

pub(self) fn get_env(env: &str) -> String {
    env::var(env)
        .map(|s| {
            let s = s.trim_end_matches('/');
            s.to_owned()
        })
        .unwrap_or_else(|_| String::new())
}

#[derive(Debug)]
pub struct EnvVars {
    pub cache: String,
    pub anime_json: String,
    config: String,
}

fn default_anime_dir() -> Vec<String> {
    let home = get_env("HOME");
    vec![format!("{home}/Videos")]
}

fn skip_empty(t: &Option<String>) -> bool {
    dbg!(t);
    if let Some(s) = t {
        return s.is_empty()
    } else {
        return false
    }
}

fn default_height() -> u32 {
    24
}

fn default_lines() -> u32 {
    15
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DmenuSettings {
    pub font: Option<String>,
    #[serde(default)]
    pub bottom: bool,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_lines")]
    pub lines: u32,
    #[serde(default)]
    pub monitor: u8,
    pub norm_bg: Option<String>,
    pub norm_fg: Option<String>,
    pub sel_bg: Option<String>,
    pub sel_fg: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    #[serde(default = "default_anime_dir")]
    pub anime_dir: Vec<String>,
    pub dmenu_settings: DmenuSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            anime_dir: default_anime_dir(),
            dmenu_settings: Default::default(),
        }
    }
}

impl Default for DmenuSettings {
    fn default() -> Self {
        Self {
            font: None,
            bottom: false,
            case_insensitive: false,
            height: 24,
            lines: 15,
            monitor: 0,
            norm_bg: None,
            norm_fg: None,
            sel_bg: None,
            sel_fg: None,
        }
    }
}