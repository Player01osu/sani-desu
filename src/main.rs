mod args;
mod cache;
mod episode;
mod setup;

use anyhow::Result;
use args::Args;
use cache::{Cache, CacheAnimeInfo, EpisodeSeason};
use episode::EpisodeSpecial;
use lazy_static::lazy_static;
use regex::Regex;
use setup::{Config, EnvVars};
use std::process::Output;
use std::{
    borrow::Cow,
    io::Write,
    process::{self, Command, Stdio},
};

lazy_static! {
    static ref ARGS: Args = Args::from(&CONFIG.dmenu_settings);
    static ref CONFIG: Config = Config::generate(&ENV);
    static ref ENV: EnvVars = EnvVars::new();
    static ref REG_EP: Regex =
        Regex::new(r#"((_|x|E|e|EP|ep| )\d{2}(.bits|_| |-|\.|v|$))"#).unwrap();
    static ref REG_S: Regex = Regex::new(r#"((^|S|s)\d{2}(.bits|x|X|E|e|_)|( \d{2}(e|E|_|-|x|X)|^(S|s)\d{2} ))"#).unwrap();
    static ref REG_PARSE_OUT: Regex = Regex::new(r#"(x256|x265|\d{4}|\d{3})|10.bits"#).unwrap();
    static ref REG_SPECIAL: Regex =
    Regex::new(r#"(.*OVA.*\.|NCED.*? |NCOP.*? |(-|_| )ED.*?(-|_| )|(-|_| )OP.*?)"#).unwrap();
}

pub fn dmenu(args: &[String], pipe: &str) -> Output {
    let mut dmenu = Command::new("dmenu")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .args(args)
        .spawn()
        .unwrap();

    dmenu
        .stdin
        .as_mut()
        .unwrap()
        .write_all(pipe.as_bytes())
        .unwrap();
    dmenu.wait_with_output().unwrap()
}

struct Sani<'setup> {
    cache: Cache<'setup>,
    anime_sel: Option<Cow<'setup, String>>,
    ep_sel: Vec<String>,
    ep_sel_path: String,
    state: AppState,
    episode: u32,
    season: u32,
    string_buf: String,
}

pub enum AppState {
    ShowSelect,
    EpSelect(bool),
    Watching,
    WriteCache,
    Quit(exitcode::ExitCode),
}

impl<'setup> Sani<'setup> {
    fn new() -> Self {
        Self {
            cache: Cache::new(ENV.cache.as_str()),
            state: AppState::ShowSelect,
            anime_sel: None,
            ep_sel: Vec::default(),
            episode: 0,
            season: 0,
            ep_sel_path: Default::default(),
            string_buf: String::new(),
        }
    }

    pub fn start() -> Result<i32, i32> {
        let mut app = Sani::new();

        loop {
            match app.state {
                AppState::ShowSelect => app.select_show(),
                AppState::EpSelect(watched) => app.select_ep(watched),
                AppState::Watching => app.watching(),
                AppState::WriteCache => app.write_cache(),
                AppState::Quit(exitcode) => return app.quit(exitcode),
            }
        }
    }

    fn select_show(&mut self) {
        let anime_list = &self.cache.read_list().unwrap();
        let output = dmenu(&ARGS.args, anime_list);

        let binding = String::from_utf8(output.stdout).unwrap();
        let show_sel = binding.trim();

        self.anime_sel = Some(Cow::Owned(show_sel.to_owned()));

        if show_sel.is_empty() {
            self.state = AppState::Quit(exitcode::OK);
        } else if anime_list.contains(show_sel) {
            self.state = AppState::EpSelect(false);
        } else {
            self.state = AppState::ShowSelect;
        }
    }

    fn select_ep(&mut self, watched: bool) {
        self.string_buf.clear();
        let anime_sel = self.anime_sel.as_ref().unwrap();

        let episode_vec = self.cache.read_ep(anime_sel).unwrap();

        self.fill_string(&episode_vec, watched);
        let ep_list = self.string_buf.trim();

        let output = dmenu(&ARGS.args, ep_list);

        let binding = String::from_utf8(output.stdout).unwrap();
        let ep_sel = binding.trim();

        if ep_sel.is_empty() {
            self.state = AppState::ShowSelect;
        } else if let Some((Some(file_path), episode_special)) = self.file_path(ep_sel) {
            match episode_special {
                EpisodeSpecial::EpS(ep_s) => {
                    self.season = ep_s.s;
                    self.episode = ep_s.ep;
                }
                _ => (),
            }
            self.ep_sel = file_path;

            self.state = AppState::Watching
        } else {
            self.state = AppState::EpSelect(true);
        }
    }

    fn watching(&mut self) {
        for episode in self.ep_sel.iter() {
            let args: Vec<&str> = vec![episode];

            let current_ep = EpisodeSeason {
                ep: self.episode,
                s: self.season,
            };

            let next_ep = self
                .cache
                .next_ep(self.anime_sel.as_ref().unwrap().as_ref(), &current_ep);

            self.cache.write_finished(current_ep, next_ep);

            let status = Command::new("mpv")
                .args(&args)
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            if !status.success() {
                continue;
            }
            self.ep_sel_path = episode.to_owned();

            self.state = AppState::EpSelect(true);
            self.write_cache();
            break;
        }
    }

    fn quit(self, exitcode: i32) -> Result<i32, i32> {
        self.cache.close();
        match exitcode {
            exitcode::OK => Ok(exitcode::OK),
            _ => Err(exitcode),
        }
    }
}

impl<'cache> Sani<'cache> {
    fn file_path(&self, episode_chosen: &str) -> Option<(Option<Vec<String>>, EpisodeSpecial)> {
        self.parse_str(episode_chosen).map(|episode_special| {
            (
                self.cache
                    .find_ep(self.anime_sel.as_ref().unwrap().as_ref(), &episode_special),
                episode_special,
            )
        })
    }

    fn parse_str(&self, episode_chosen: &str) -> Option<EpisodeSpecial> {
        match episode_chosen {
            "Current Episode:" => {
                let season = self.cache.current_ep_s.s;
                let episode = self.cache.current_ep_s.ep;
                Some(EpisodeSpecial::EpS(EpisodeSeason {
                    ep: episode,
                    s: season,
                }))
            }
            "Next Episode:" => {
                let Some(ref ep_s) = self.cache.next_ep_s else {
                    return None;
                };
                let episode = ep_s.ep;
                let season = ep_s.s;
                Some(EpisodeSpecial::EpS(EpisodeSeason {
                    ep: episode,
                    s: season,
                }))
            }
            str => {
                let Some((s, ep)) = str.split_once(' ') else {
                    return Some(EpisodeSpecial::Special(str.to_string()));
                };
                if s.chars().next().unwrap() == 'S' && ep.chars().next().unwrap() == 'E' {
                    let s = s
                        .chars()
                        .filter(|c| c.is_digit(10))
                        .collect::<String>()
                        .parse()
                        .unwrap();
                    let ep = ep
                        .chars()
                        .filter(|c| c.is_digit(10))
                        .collect::<String>()
                        .parse()
                        .unwrap();
                    return Some(EpisodeSpecial::EpS(EpisodeSeason { ep, s }));
                }
                return Some(EpisodeSpecial::Special(str.to_owned()));
            }
        }
    }

    fn fill_string(&mut self, episode_vec: &[EpisodeSpecial], watched: bool) {
        if !watched {
            let relative_ep = self
                .cache
                .read_relative_ep(self.anime_sel.as_ref().unwrap())
                .unwrap();
            self.cache.current_ep_s = relative_ep.current_ep;
            self.cache.next_ep_s = relative_ep.next_ep;
        }
        self.string_buf.push_str("Current Episode:\n");
        let binding = format!(
            "S{:02} E{:02}\n",
            self.cache.current_ep_s.s, self.cache.current_ep_s.ep
        );
        self.string_buf.push_str(&binding);

        let next_ep = self.cache.next_ep(
            self.anime_sel.as_ref().unwrap().as_ref(),
            &self.cache.current_ep_s,
        );
        if let Some(next_ep_s) = &next_ep {
            self.string_buf.push_str("Next Episode:\n");
            let binding = format!("S{:02} E{:02}\n", next_ep_s.s, next_ep_s.ep);
            self.string_buf.push_str(&binding);
        }

        for episode in episode_vec.iter() {
            match episode {
                EpisodeSpecial::Special(special) => {
                    self.string_buf.push_str(&format!("{special}\n",));
                }
                EpisodeSpecial::EpS(ep_s) => {
                    self.string_buf
                        .push_str(&format!("S{:02} E{:02}\n", &ep_s.s, &ep_s.ep));
                }
            }
        }
    }

    fn write_cache(&mut self) {
        let info = CacheAnimeInfo {
            dir_name: self.anime_sel.as_ref().unwrap().to_string(),
            location: self.ep_sel_path.clone(),
            episode: self.episode,
            season: self.season,
        };
        self.cache.write(info).unwrap();
    }
}

fn main() -> Result<()> {
    match Sani::start() {
        Ok(v) => process::exit(v),
        Err(e) => process::exit(e),
    };
}
