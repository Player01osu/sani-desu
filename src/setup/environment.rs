use std::{
    fs::{self, File},
    path::Path,
};

use super::{get_env, EnvVars};

/// Checks if directory exists within array,
/// if not, return first.
fn init_path(check_path: Vec<String>) -> String {
    for p in check_path.iter() {
        match Path::new(&p).exists() {
            true => return p.to_owned(),
            false => continue,
        }
    }

    // Default: Creates directory and/or file.
    let default_path = check_path.first().unwrap();
    // String parses to create directory, then file when needed.
    let default_path = if default_path.contains(".json") || default_path.contains(".yaml") {
        let n = default_path.as_str().chars().fold(0, |mut i, c| {
            if c == '/' {
                i += 1;
                i
            } else {
                i
            }
        });

        let mut dir = String::new();
        let mut i = 0;
        for c in default_path.as_str().chars() {
            if i == n {
                break;
            }
            if c == '/' {
                i += 1;
            }
            dir.push(c);
        }
        fs::create_dir_all(Path::new(&dir)).unwrap();
        File::create(&default_path).unwrap();
        default_path.to_owned()
    } else {
        fs::create_dir_all(Path::new(&default_path)).unwrap();
        default_path.to_owned()
    };

    default_path
}

impl EnvVars {
    pub fn new() -> Self {
        let home = get_env("HOME");

        let xdg_data_home = get_env("XDG_DATA_HOME");
        let xdg_cache_home = get_env("XDG_CACHE_HOME");
        let xdg_config_home = get_env("XDG_CONFIG_HOME");

        let cache = match option_env!("SANI_CACHE") {
            Some(v) => v.to_owned(),
            None => init_path(vec![format!("{xdg_cache_home}/sani")]),
        };

        // Check dir existence, then check file existence
        let anime_json = match option_env!("SANI_ANIME_JSON") {
            Some(v) => v.to_owned(),
            None => init_path(vec![format!("{xdg_data_home}/sani/anime.json")]),
        };

        let config = match option_env!("SANI_CONFIG") {
            Some(v) => v.to_owned(),
            None => init_path(vec![
                format!("{xdg_config_home}/sani/config.yaml"),
                format!("{xdg_config_home}/sani_config.yaml"),
                format!("{home}/sani_config.yaml"),
            ]),
        };

        Self {
            anime_json,
            cache,
            config,
        }
    }
}
