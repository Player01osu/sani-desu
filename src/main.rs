mod args;
mod setup;

use anime_database_lib::database::{Database, EpisodeMap};
use anime_database_lib::episode::Episode;
use anyhow::Result;
use args::Args;
use lazy_static::lazy_static;
use setup::{Config, EnvVars};
use std::collections::BTreeSet;
use std::fs::read_dir;
use std::path::Path;
use std::process::Output;
use std::{
    io::Write,
    process::{self, Command, Stdio},
};

lazy_static! {
    static ref ARGS: Args = Args::from(&CONFIG.dmenu_settings);
    static ref CONFIG: Config = Config::generate(&ENV);
    static ref ENV: EnvVars = EnvVars::new();
}

type Exit = Result<i32, i32>;

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

fn parse_episode(s: &str, current: &Episode, next: &Option<Episode>) -> Option<Episode> {
    match s {
        "Current Episode:" => Some(current.clone()),
        "Next Episode:" => next.clone(),
        _ => s.parse::<Episode>().ok(),
    }
}

struct Anime {
    name: String,
    episodes: EpisodeMap,
    current: Episode,
    next: Option<Episode>,
}

struct Sani {
    database: Database,
    current_anime: Option<Anime>,
    string_buf: String,
    valid_anime: BTreeSet<String>,
}

impl Sani {
    fn new() -> Self {
        let db_file = format!("{}/anime-database.db", ENV.cache.as_str());
        let wait_thread = !Path::new(&db_file).exists();
        let database = Database::new(&db_file, CONFIG.anime_dir.clone()).unwrap();
        let valid_anime = CONFIG
            .anime_dir
            .iter()
            .filter_map(|s| read_dir(s).ok())
            .fold(BTreeSet::new(), |mut acc, d| {
                acc.append(
                    &mut d
                        .filter_map(|v| {
                            v.ok()
                                .and_then(|v| Some(v.file_name().to_str().unwrap().to_owned()))
                        })
                        .collect(),
                );
                acc
            });

        if wait_thread {
            database.init_db().unwrap();
            database.update().unwrap();
        } else {
            database.threaded_update();
        }

        Self {
            database,
            current_anime: None,
            string_buf: String::new(),
            valid_anime,
        }
    }

    fn lock_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn start() -> Exit {
        let mut app = Sani::new();
        match app.lock_file() {
            Ok(()) => (),
            Err(e) => {
                eprintln!("{e}");
                return Err(exitcode::OSFILE);
            }
        }

        app.select_show()
    }

    fn current_anime(&mut self, show_name: String) {
        let episodes = self.database.episodes(&show_name).unwrap();
        let current = self.database.current_episode(&show_name).unwrap();
        let next = self
            .database
            .next_episode_raw(current, &episodes)
            .map(|v| (*v).clone());

        self.current_anime = Some(Anime {
            name: show_name,
            episodes,
            current: current.into(),
            next,
        });
    }

    fn select_show(&mut self) -> Exit {
        let anime_list = self
            .database
            .animes()
            .unwrap()
            .into_iter()
            .filter(|v| self.valid_anime.contains(*v))
            .map(|v| v.to_string())
            .collect::<Box<[String]>>();
        let anime_str = anime_list.join("\n");
        let output = dmenu(&ARGS.args, anime_str.trim());

        let binding = String::from_utf8(output.stdout).unwrap();
        let show_name = binding.trim().to_string();

        if show_name.is_empty() {
            self.quit(exitcode::OK)
        } else if anime_list.contains(&show_name) {
            self.current_anime(show_name);
            self.select_ep()
        } else {
            self.select_show()
        }
    }

    fn select_ep(&mut self) -> Exit {
        self.string_buf.clear();
        let current_anime = match self.current_anime {
            Some(ref v) => v,
            None => {
                return self.select_show();
            }
        };
        let episodes = &current_anime.episodes;
        let current = &current_anime.current;
        let next = &current_anime.next;
        let buf = &mut self.string_buf;

        fill_string(buf, episodes.keys(), current, next.as_ref());
        let episodes_string = self.string_buf.trim();
        let output = dmenu(&ARGS.args, episodes_string);
        let binding = String::from_utf8(output.stdout).unwrap();
        let selected = binding.trim();

        if selected.is_empty() {
            return self.select_show();
        }

        match parse_episode(selected, current, next) {
            Some(v) => match episodes.get(&v) {
                Some(_) => {
                    let current_anime = self.current_anime.as_mut().expect("Should not be empty");
                    let episodes = &current_anime.episodes;
                    match &v {
                        Episode::Numbered { season, episode } => {
                            current_anime.next = self
                                .database
                                .next_episode_raw((*season, *episode), &episodes)
                                .map(|v| (*v).clone());
                        }
                        _ => (),
                    }
                    current_anime.current = v;
                    self.watching()
                }
                None => self.select_ep(),
            },
            None => self.select_ep(),
        }
    }

    fn watching(&mut self) -> Exit {
        let current_anime = match self.current_anime {
            Some(ref v) => v,
            None => {
                eprintln!("Should not be possible: current anime should exist");
                return self.select_show();
            }
        };

        for episode in &current_anime.episodes[&current_anime.current] {
            let args: Vec<&str> = vec![&episode];

            let status = Command::new("mpv")
                .args(&args)
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            if !status.success() {
                continue;
            }

            self.write_cache();
            break;
        }
        self.select_ep()
    }

    fn quit(&self, exitcode: i32) -> Exit {
        match exitcode {
            exitcode::OK => Ok(exitcode::OK),
            _ => Err(exitcode),
        }
    }
}

fn fill_string<'a>(
    buf: &mut String,
    episodes: impl Iterator<Item = &'a Episode>,
    current: &Episode,
    next: Option<&Episode>,
) {
    buf.push_str("Current Episode:\n");
    let binding = format!("{current}\n");
    buf.push_str(&binding);

    if let Some(next) = next {
        buf.push_str("Next Episode:\n");
        let binding = format!("{next}\n");
        buf.push_str(&binding);
    }

    for episode in episodes {
        buf.push_str(&format!("{episode}\n",));
    }
}

impl Sani {
    fn write_cache(&mut self) {
        let current_anime = match self.current_anime {
            Some(ref v) => v,
            None => {
                eprintln!("Should not be possible: current anime should exist");
                return;
            }
        };
        let anime = &current_anime.name;
        let watched = &current_anime.current;
        let episode_map = &current_anime.episodes;

        self.database
            .update_watched(&anime, watched.clone(), episode_map)
            .unwrap();
    }
}

fn main() -> Result<()> {
    match Sani::start() {
        Ok(v) => process::exit(v),
        Err(e) => process::exit(e),
    };
}
