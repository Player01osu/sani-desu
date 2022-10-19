mod args;
mod cache;
mod episode;
mod setup;

use anyhow::Result;
use args::Args;
use cache::{Cache, CacheAnimeInfo, EpisodeSeason};
use lazy_static::lazy_static;
use regex::Regex;
use setup::{Config, EnvVars};
use std::process::Output;
use std::{
    borrow::Cow,
    io::{BufRead, BufReader, Write},
    process::{self, Command, Stdio},
};

lazy_static! {
    static ref ARGS: Args = Args::from(&CONFIG.dmenu_settings);
    static ref CONFIG: Config = Config::generate(&ENV);
    static ref ENV: EnvVars = EnvVars::new();
    static ref REG_EP: Regex =
        Regex::new(r#"(x256|x265| \d\d |E\d\d|x\d\d| \d\d.|_\d\d_)"#).unwrap();
    static ref REG_S: Regex = Regex::new(r#"(x256| \d\dx|S\d\d)"#).unwrap();
    static ref REG_PARSE_OUT: Regex = Regex::new(r#"(x256|x265|\d{4})"#).unwrap();
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
    state: AppState,
    episode: u32,
    season: u32,
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
            anime_sel: None,
            ep_sel: Vec::default(),
            state: AppState::ShowSelect,
            episode: 0,
            season: 0,
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
        // FIXME: Make more efficient
        let mut ep_list = String::new();
        let anime_sel = self.anime_sel.as_ref().unwrap();

        let mut episode_vec = self.cache.read_ep(anime_sel).unwrap();

        self.fill_string(&mut ep_list, &mut episode_vec, watched);
        let ep_list = ep_list.trim();

        let output = dmenu(&ARGS.args, ep_list);

        let binding = String::from_utf8(output.stdout).unwrap();
        let ep_sel = binding.trim();

        if ep_sel.is_empty() {
            self.state = AppState::ShowSelect;
        } else if let Some((Some(file_path), episode_season)) = self.file_path(ep_sel) {
            self.season = episode_season.season;
            self.episode = episode_season.episode;
            self.ep_sel = file_path;

            self.state = AppState::Watching
        } else {
            self.state = AppState::EpSelect(true);
        }
    }

    fn watching(&mut self) {
        for episode in self.ep_sel.iter() {
            let args: Vec<&str> = vec![&episode];

            let current_ep = EpisodeSeason {
                episode: self.episode,
                season: self.season,
            };

            let next_ep = EpisodeSeason {
                episode: self.episode + 1,
                season: self.season,
            };

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
    fn file_path(&self, episode_chosen: &str) -> Option<(Option<Vec<String>>, EpisodeSeason)> {
        if let Some(episode_season) = self.parse_str(episode_chosen) {
            Some((
                self.cache
                    .find_ep(self.anime_sel.as_ref().unwrap().as_ref(), &episode_season),
                episode_season,
            ))
        } else {
            None
        }
    }

    fn parse_str(&self, str: &str) -> Option<EpisodeSeason> {
        match str {
            "Current Episode:" => {
                let season = self.cache.current_ep_s.season;
                let episode = self.cache.current_ep_s.episode;
                return Some(EpisodeSeason { episode, season });
            }
            "Next Episode:" => {
                let season = self.cache.next_ep_s.season;
                let episode = self.cache.next_ep_s.episode;
                return Some(EpisodeSeason { episode, season });
            }
            str => {
                let ep = REG_EP.find(str);
                let s = REG_S.find(str);

                if let Some(ep) = ep {
                    if let Some(s) = s {
                        let episode = ep
                            .as_str()
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .collect::<String>()
                            .parse()
                            .unwrap();
                        let season = s
                            .as_str()
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .collect::<String>()
                            .parse()
                            .unwrap();
                        Some(EpisodeSeason { episode, season })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }

    fn fill_string(&mut self, ep_list: &mut String, episode_vec: &[EpisodeSeason], watched: bool) {
        if !watched {
            let relative_ep = self
                .cache
                .read_relative_ep(self.anime_sel.as_ref().unwrap())
                .unwrap();
            self.cache.current_ep_s = relative_ep.current_ep;
            self.cache.next_ep_s = relative_ep.next_ep;
        }
        ep_list.push_str("Current Episode:\n");
        let binding = format!(
            "S{:02} E{:02}\n",
            self.cache.current_ep_s.season, self.cache.current_ep_s.episode
        );
        ep_list.push_str(&binding);
        ep_list.push_str("Next Episode:\n");
        let binding = format!(
            "S{:02} E{:02}\n",
            self.cache.next_ep_s.season, self.cache.next_ep_s.episode
        );
        ep_list.push_str(&binding);

        for episode in episode_vec.iter() {
            ep_list.push_str(&format!(
                "S{:02} E{:02}\n",
                &episode.season, &episode.episode
            ));
        }
    }

    fn write_cache(&mut self) {
        let info = CacheAnimeInfo {
            directory: self.anime_sel.as_ref().unwrap().to_string(),
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
