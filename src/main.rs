mod cache;
mod setup;

use anyhow::Result;
use cache::{Cache, CacheAnimeInfo};
use interprocess::local_socket::LocalSocketStream;
use nix::{sys, unistd::Pid};
use serde_json::Value;
use setup::{Config, DmenuSettings, EnvVars};
use std::thread;
use std::{
    borrow::Cow,
    cell::RefCell,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{self, Command, Stdio},
    rc::Rc,
    time::Duration,
};

struct Sani<'setup> {
    cache: Cache,
    config: &'setup Config,
    env: &'setup EnvVars,
    anime_sel: Option<Cow<'setup, String>>,
    ep_sel: Option<String>,
    state: AppState,
    mpv_socket: RefCell<io::Result<LocalSocketStream>>,
    timestamp: u64,
    child_pid: i32,
}

struct Args {
    args: Vec<String>,
}

impl From<&DmenuSettings> for Args {
    fn from(dmenu_settings: &DmenuSettings) -> Self {
        let mut args: Vec<String> = Vec::with_capacity(16);

        // FIXME: A lot of cloning and allocation here
        args.push("-p".to_string());
        args.push("Select anime".to_string());

        args.push("-l".to_string());
        args.push(dmenu_settings.lines.to_string());

        if dmenu_settings.bottom {
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
    WriteCache,
    Quit(exitcode::ExitCode),
}

impl<'setup> Sani<'setup> {
    fn new(config: &'setup Config, env: &'setup EnvVars) -> Self {
        Self {
            config,
            env,
            cache: Cache::new(env.cache.as_str()),
            anime_sel: None,
            ep_sel: None,
            state: AppState::ShowSelect,
            mpv_socket: RefCell::new(Err(std::io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "Poops",
            ))),
            timestamp: 0,
            child_pid: 0,
        }
    }

    fn select_show(&mut self, anime_list: &str, args: Rc<Args>) {
        let mut dmenu = Command::new("dmenu")
            .args(&args.args)
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
        self.anime_sel = Some(Cow::Owned(show_sel.to_owned()));

        dbg!(&show_sel);

        if show_sel.is_empty() {
            self.state = AppState::Quit(exitcode::OK);
        } else {
            self.state = AppState::EpSelect;
        }
    }

    fn select_ep(&mut self, args: Rc<Args>) {
        // FIXME: Make more efficient
        let mut ep_list = String::new();
        let binding = self.anime_sel.as_ref().unwrap();
        let anime_sel = binding;
        let list = self.config.anime_dir.iter().flat_map(|v| {
            fs::read_dir(&format!("{v}/{}", anime_sel))
                .unwrap()
                .map(|d| d.unwrap().file_name())
        });
        for i in list {
            ep_list.push_str(&format!("{}\n", i.to_str().unwrap()));
        }
        let ep_list = ep_list.trim();

        let mut dmenu = Command::new("dmenu")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .args(&args.args)
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
            self.ep_sel = Some(ep_sel.to_owned());
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
            // Parent process run mpv
            Some(ep) => {
                let timestamp = self
                    .cache
                    .read_timestamp(&self.ep_sel.as_ref().unwrap())
                    .unwrap_or_default();
                let timestamp_arg = format!("--start={timestamp}");
                dbg!(timestamp);
                let args: Vec<&str> = vec![ep, "--input-ipc-server=/tmp/mpvsocket", &timestamp_arg];

                Command::new("mpv")
                    .args(&args)
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();

                let pid = Pid::from_raw(self.child_pid);
                if sys::signal::kill(pid, sys::signal::SIGTERM).is_ok() {
                    thread::spawn(|| {
                        if sys::wait::wait().is_ok() {
                            ()
                        }
                    });
                }

                true
            }
            // Child process checks for mpv timestamp
            None => {
                use std::sync::atomic::{AtomicBool, Ordering};
                use std::sync::Arc;
                let term = Arc::new(AtomicBool::new(false));
                signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))
                    .unwrap();
                while !term.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::new(1, 0));

                    let socket = self.mpv_socket.get_mut();
                    if socket.is_err() {
                        self.mpv_socket =
                            RefCell::new(LocalSocketStream::connect("/tmp/mpvsocket"));
                    }

                    let socket = self.mpv_socket.get_mut();
                    let socket = socket.as_mut();
                    match socket {
                        Ok(conn) => {
                            if let Ok(_) = conn.write_all(
                                br#"{"command":["get_property","playback-time"],"request_id":1}"#,
                            ) {
                                conn.write_all(b"\n").unwrap();
                                conn.flush().unwrap();

                                let mut conn = BufReader::new(conn);
                                let mut buffer = String::new();
                                conn.read_line(&mut buffer).unwrap();

                                let e = serde_json::from_str::<Value>(&buffer).unwrap();
                                self.timestamp = match *&e["data"].as_f64() {
                                    Some(v) => v.trunc() as u64,
                                    None => {
                                        dbg!(buffer);
                                        return ();
                                    }
                                };
                            }
                        }
                        Err(e) => {
                            eprintln!("{e}")
                        }
                    }
                }
                self.state = AppState::WriteCache;
                false
            }
        };

        if f {
            self.state = AppState::EpSelect;
        }
    }

    fn write_cache(&mut self) {
        let info = CacheAnimeInfo::builder()
            .filename(&self.ep_sel.as_ref().unwrap())
            .timestamp(self.timestamp)
            .anime_name("D")
            .current_ep(0)
            .finalize()
            .unwrap();
        self.cache.write(info).unwrap();

        self.state = AppState::Quit(exitcode::OK);
    }

    pub fn start(config: &Config, env: &EnvVars) -> Result<i32, i32> {
        let mut app = Sani::new(config, env);

        // FIXME: Make more efficient
        let mut anime_list = String::new();
        let list = app
            .config
            .anime_dir
            .iter()
            .flat_map(|v| fs::read_dir(v).unwrap().map(|d| d.unwrap().file_name()));
        for i in list {
            anime_list.push_str(&format!("{}\n", i.to_str().unwrap()));
        }
        let anime_list = anime_list.trim();

        let dmenu_settings = &app.config.dmenu_settings;
        let args = Rc::new(Args::from(dmenu_settings));

        loop {
            match app.state {
                AppState::ShowSelect => app.select_show(anime_list, Rc::clone(&args)),
                AppState::EpSelect => app.select_ep(Rc::clone(&args)),
                AppState::Watching(ref mpv_id) => app.watching(Rc::clone(mpv_id)),
                AppState::WriteCache => app.write_cache(),
                AppState::Quit(exitcode) => match exitcode {
                    exitcode::OK => return Ok(exitcode::OK),
                    _ => return Err(exitcode::USAGE),
                },
            }
        }
    }
}

fn main() -> Result<()> {
    let env = EnvVars::new();
    let config = Config::generate(&env);

    match Sani::start(&config, &env) {
        Ok(v) => process::exit(v),
        Err(e) => process::exit(e),
    };
}
