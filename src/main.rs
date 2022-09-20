mod cache;
mod setup;

use anyhow::Result;
use cache::{Cache, CacheAnimeInfo};
use serde::{Deserialize, Serialize};
use setup::{Config, DmenuSettings, EnvVars};
use std::{
    fs,
    path::Path,
    process::{self, Command, ExitCode, Stdio},
};

struct Sani<'setup> {
    config: &'setup Config,
    env: &'setup EnvVars,
    cache: Cache<'setup>,
    state: AppState,
}

struct Args {
    args: Vec<String>,
}

impl From<&DmenuSettings> for Args {
    fn from(dmenu_settings: &DmenuSettings) -> Self {
        let mut args: Vec<String> = Vec::new();

        // FIXME: A lot of cloning and allocation here
        args.push("-p".to_string());
        args.push("Select anime".to_string());

        args.push("-l".to_string());
        args.push(dmenu_settings.lines.to_string());

        if dmenu_settings.bottom == true {
            args.push("-b".to_string());
        }

        if let Some(font) = &dmenu_settings.font {
            args.push("-fn".to_string());
            args.push(font.to_owned());
        }
        if let Some(norm_fg) = &dmenu_settings.norm_fg {
            args.push("-nf".to_string());
            args.push(norm_fg.to_owned());
        }

        if let Some(norm_bg) = &dmenu_settings.norm_bg {
            args.push("-nb".to_string());
            args.push(norm_bg.to_owned());
        }

        if let Some(sel_fg) = &dmenu_settings.sel_fg {
            args.push("-sf".to_string());
            args.push(sel_fg.to_owned());
        }

        if let Some(sel_bg) = &dmenu_settings.sel_bg {
            args.push("-sb".to_string());
            args.push(sel_bg.to_owned());
        }
        Args { args }
    }
}

pub enum AppState {
    ShowSelect,
    EpSelect,
}

impl<'setup> Sani<'setup> {
    fn new(config: &'setup Config, env: &'setup EnvVars) -> Self {
        Self {
            config: &config,
            env: &env,
            cache: Cache::new(env.cache.as_str()),
            state: AppState::ShowSelect,
        }
    }

    pub fn start(config: &Config, env: &EnvVars) -> Result<(), ExitCode> {
        let app = Sani::new(config, env);

        // FIXME: Make more efficient
        let mut anime_list = String::new();
        let list = app
            .config
            .anime_dir
            .iter()
            .map(|v| fs::read_dir(v).unwrap().map(|d| d.unwrap().file_name()))
            .flatten();
        for i in list {
            anime_list.push_str(&format!("{}\n", i.to_str().unwrap()));
        }
        let mut anime_list = anime_list.trim();

        //dbg!(&anime_list);

        // FIXME: Pipe directly from string rather than calling echo.
        let pipe = Command::new("echo")
            .arg(&mut anime_list)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
            .stdout
            .unwrap();
        let dmenu_settings = &app.config.dmenu_settings;

        // FIXME: Dont clone the args
        let args = Args::from(dmenu_settings);

        let show_selection = Command::new("dmenu")
            .stdin(pipe)
            .args(args.args.clone())
            .output()
            .unwrap();

        // FIXME: Pipe directly from string rather than calling echo.
        let sel = String::from_utf8(show_selection.stdout).unwrap();
        let sel = sel.trim();

        dbg!(&sel);
        // FIXME: Make more efficient
        let mut ep_list = String::new();
        let list = app
            .config
            .anime_dir
            .iter()
            .map(|v| {
                fs::read_dir(&format!("{v}/{sel}"))
                    .unwrap()
                    .map(|d| d.unwrap().file_name())
            })
            .flatten();
        for i in list {
            ep_list.push_str(&format!("{}\n", i.to_str().unwrap()));
        }
        let mut ep_list = ep_list.trim();
        dbg!(ep_list);

        let pipe = Command::new("echo")
            .arg(&mut ep_list)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
            .stdout
            .unwrap();

        let ep_sel = Command::new("dmenu")
            .stdin(pipe)
            .args(args.args)
            .output()
            .unwrap();
        let ep_sel = String::from_utf8(ep_sel.stdout).unwrap();
        let ep_sel = format!(
            "{}/{}/{}",
            app.config.anime_dir.first().unwrap(),
            sel,
            ep_sel.trim()
        );

        Command::new("mpv").arg(ep_sel).spawn().unwrap();

        // Cache anime dir
        //
        Ok(())
    }

    fn cache(&self) {
        let f = Path::new(&self.env.cache);
        //self.config
    }
}

fn main() {
    // Setup stage:
    // 1. Grab or set environment variables
    //    - Anime location
    //    - Dmenu settings
    // 2. Parse command line arguments
    // 3. Ensure all require programs exist
    //    - Mpv
    //    - Dmenu
    //    - Ls
    // 4. Check for cache folder
    //    - First $SANI_CACHE
    //    - Then $XDG_CACHE_HOME/sani
    //    - Create if not exist
    // 5. Check for locally saved anime.json
    //    - First $SANI_ANIME_JSON
    //    - Then $XDG_DATA_HOME/sani/anime.json
    //    - Create if not exist
    // 6. Check for config folder
    //    - First $SANI_CONFIG
    //    - Then $XDG_CONFIG_HOME/sani/config
    //    - Then $XDG_CONFIG_HOME/sani_config
    //    - Lastly $HOME/.sani_config
    //    - Create if not exist
    // 7. Validate config
    //    - Warn invalid config
    //    - Log
    //    - Guide to wiki
    // 8. Start program
    let env = EnvVars::new();
    let config = Config::generate(&env);

    match Sani::start(&config, &env) {
        Ok(_) => (),
        Err(_) => (),
    };
    //let ls = Command::new("/bin/ls")
    //    .stdout(Stdio::piped())
    //    .spawn()
    //    App::start();
    //    .unwrap()
    //    .stdout
    //    .unwrap();
    //let child = Command::new("dmenu").stdin(ls).spawn().unwrap();
}
