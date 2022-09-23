mod cache;
mod setup;

use anyhow::Result;
use cache::{Cache, CacheAnimeInfo};
use interprocess::local_socket::LocalSocketStream;
use nix::{sys, unistd::Pid};
use serde_json::Value;
use setup::{Config, DmenuSettings, EnvVars};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{self, Command, Stdio},
    rc::Rc,
    time::Duration,
};

struct Sani<'setup> {
    cache: Cache<'setup>,
    config: &'setup Config,
    env: &'setup EnvVars,
    anime_sel: Option<String>,
    ep_sel: Option<String>,
    state: AppState,
    timestamp: u64,
    child_pid: i32,
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

        if dmenu_settings.case_insensitive {
            args.push("-i".to_string());
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
    Watching(Rc<Option<String>>),
    Quit(exitcode::ExitCode),
}

impl<'setup> Sani<'setup> {
    fn new(config: &'setup Config, env: &'setup EnvVars) -> Self {
        Self {
            config: &config,
            env: &env,
            cache: Cache::new(env.cache.as_str()),
            anime_sel: None,
            ep_sel: None,
            state: AppState::ShowSelect,
            timestamp: 0,
            child_pid: 0,
        }
    }

    fn select_show(&mut self, anime_list: &str, args: &Args) {
        let mut dmenu = Command::new("dmenu")
            .args(args.args.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        dmenu
            .stdin
            .as_mut()
            .unwrap()
            .write_all(anime_list.as_bytes())
            .unwrap();
        let output = dmenu.wait_with_output().unwrap();

        let binding = String::from_utf8(output.stdout).unwrap();
        let show_sel = binding.trim();
        self.anime_sel = Some(show_sel.to_owned());

        dbg!(&show_sel);

        if show_sel.is_empty() {
            self.state = AppState::Quit(exitcode::OK);
        } else {
            self.state = AppState::EpSelect;
        }
    }

    fn select_ep(&mut self, args: &Args) {
        // FIXME: Make more efficient
        let mut ep_list = String::new();
        let anime_sel = self.anime_sel.as_ref().unwrap();
        let list = self
            .config
            .anime_dir
            .iter()
            .map(|v| {
                fs::read_dir(&format!("{v}/{}", &anime_sel))
                    .unwrap()
                    .map(|d| d.unwrap().file_name())
            })
            .flatten();
        for i in list {
            ep_list.push_str(&format!("{}\n", i.to_str().unwrap()));
        }
        let ep_list = ep_list.trim();

        let mut dmenu = Command::new("dmenu")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .args(args.args.clone())
            .spawn()
            .unwrap();

        dmenu
            .stdin
            .as_mut()
            .unwrap()
            .write_all(ep_list.as_bytes())
            .unwrap();

        let output = dmenu.wait_with_output().unwrap();
        let binding = String::from_utf8(output.stdout).unwrap();
        let ep_sel = binding.trim();

        if ep_sel.is_empty() {
            self.state = AppState::ShowSelect;
        } else {
            let ep_sel = format!(
                "{}/{}/{}",
                self.config.anime_dir.first().unwrap(),
                self.anime_sel.as_ref().unwrap(),
                ep_sel
            );

            match fork::fork() {
                Ok(fork::Fork::Parent(child)) => {
                    self.child_pid = child;
                    self.state = AppState::Watching(Rc::new(Some(ep_sel)))
                }
                Ok(fork::Fork::Child) => self.state = AppState::Watching(Rc::new(None)),
                Err(e) => eprintln!("{e}"),
            };
        }
    }

    fn watching(&mut self, handle: Rc<Option<String>>) {
        let f = match &*handle {
            Some(ep) => {
                let mut args: Vec<&str> = Vec::new();
                args.push(&ep);
                args.push("--input-ipc-server=/tmp/mpvsocket");

                Command::new("mpv")
                    .args(&args)
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();

                let pid = Pid::from_raw(self.child_pid);
                match sys::signal::kill(pid, sys::signal::SIGKILL) {
                    Ok(_) => match sys::wait::wait() {
                        Ok(_) => (),
                        Err(_) => (),
                    },
                    Err(_) => (),
                }

                true
            }
            None => {
                std::thread::sleep(Duration::new(2, 0));

                let socket = LocalSocketStream::connect("/tmp/mpvsocket");
                match socket {
                    Ok(mut conn) => {
                        conn.write_all(
                            br#"{"command":["get_property","playback-time"],"request_id":1}"#,
                        )
                        .unwrap();
                        conn.write_all(b"\n").unwrap();
                        conn.flush().unwrap();

                        let mut conn = BufReader::new(conn);
                        let mut buffer = String::new();
                        conn.read_line(&mut buffer).unwrap();

                        dbg!(&buffer);
                        let e = serde_json::from_str::<Value>(&buffer).unwrap();
                        dbg!(*&e["data"].as_f64().unwrap() as u64);
                    }
                    Err(e) => eprintln!("{e}"),
                }
                false
            }
        };

        if f {
            self.state = AppState::EpSelect;
        }
    }

    pub fn start(config: &Config, env: &EnvVars) -> Result<i32, i32> {
        let mut app = Sani::new(config, env);

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
        let anime_list = anime_list.trim();

        let dmenu_settings = &app.config.dmenu_settings;
        // FIXME: Dont clone the args
        let args = Args::from(dmenu_settings);

        loop {
            match app.state {
                AppState::ShowSelect => app.select_show(anime_list, &args),
                AppState::EpSelect => app.select_ep(&args),
                AppState::Watching(ref mpv_id) => app.watching(Rc::clone(&mpv_id)),
                AppState::Quit(exitcode) => match exitcode {
                    exitcode::OK => return Ok(exitcode::OK),
                    _ => return Err(exitcode::USAGE),
                },
            }
        }
        // Cache anime dir
        //
    }

    fn cache(&self) {
        let f = Path::new(&self.env.cache);
        //self.config
    }
}

fn main() -> Result<()> {
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
        Ok(v) => process::exit(v),
        Err(e) => process::exit(e),
    };
}
