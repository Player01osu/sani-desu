mod args;
mod setup;

use anime_database_lib::database::Database;
use anime_database_lib::episode::Episode;
use anyhow::Result;
use args::Args;
use lazy_static::lazy_static;
use setup::{Config, EnvVars};
use std::collections::BTreeSet;
use std::fs::read_dir;
use std::process::Output;
use std::{
    io::Write,
    process::{self, Command, Stdio},
};

use rayon::prelude::*;

lazy_static! {
    static ref ARGS: Args = Args::from(&CONFIG.dmenu_settings);
    static ref CONFIG: Config = Config::generate(&ENV);
    static ref ENV: EnvVars = EnvVars::new();
    static ref DB_FILE: String = format!("{}/anime-database-migrating.db", ENV.cache.as_str());
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

struct Sani {
    database: Database,
    string_buf: String,
    anime_str: String,
    anime_exist: BTreeSet<String>,
}

fn anime_exist_list(anime_dir: &[String]) -> BTreeSet<String> {
    anime_dir
        .iter()
        .filter_map(|s| read_dir(s).ok())
        .fold(BTreeSet::new(), |mut acc, d| {
            acc.append(
                &mut d
                    .par_bridge()
                    .filter_map(|v| {
                        v.ok()
                            .and_then(|v| Some(v.file_name().to_str().unwrap().to_owned()))
                    })
                    .collect(),
            );
            acc
        })
}

impl Sani {
    fn new() -> Self {
        let mut database = Database::new(DB_FILE.as_str(), CONFIG.anime_dir.clone()).unwrap();
        let anime_exist = anime_exist_list(CONFIG.anime_dir.as_slice());
        let anime_str = database
            .animes()
            .unwrap()
            .iter()
            .filter(|(s, _)| anime_exist.contains(*s))
            .map(|(name, _)| (*name).to_owned())
            .collect::<Vec<_>>() // TODO: Maybe don't collect here
            .join("\n");
        Self {
            database,
            string_buf: String::new(),
            anime_str,
            anime_exist,
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

    fn select_show(&mut self) -> Exit {
        let anime_str = &self.anime_str;
        let output = dmenu(&ARGS.args, anime_str.trim());

        let binding = String::from_utf8(output.stdout).unwrap();
        let show_name = binding.trim().to_string();

        if show_name.is_empty() {
            self.quit(exitcode::OK)
        } else {
            self.select_ep(show_name)
        }
    }

    fn select_ep(&mut self, show_name: String) -> Exit {
        self.string_buf.clear();
        let anime = self.database.animes().unwrap();
        let anime = match anime
            .iter()
            .filter(|(s, _)| self.anime_exist.contains(*s))
            .into_iter()
            .find(|(v, _)| show_name.eq(*v))
        {
            Some((_, v)) => v,
            None => {
                return self.select_show();
            }
        };
        let episodes = &anime.episodes();
        let current = &anime.current_episode();
        let next = &anime.next_episode().unwrap();
        let buf = &mut self.string_buf;

        fill_string(buf, episodes.iter().map(|(v, _)| v), current, next.as_ref());
        let episodes_string = self.string_buf.trim();
        let output = dmenu(&ARGS.args, episodes_string);
        let binding = String::from_utf8(output.stdout).unwrap();
        let selected = binding.trim();

        if selected.is_empty() {
            return self.select_show();
        }

        match parse_episode(selected, current, next) {
            Some(v) => match episodes.iter().find(|(ep, _)| v.eq(ep)) {
                Some((_, paths)) => {
                    let paths = paths.to_owned();
                    self.watching(show_name, v, &paths)
                }
                None => self.select_ep(show_name),
            },
            None => self.select_ep(show_name),
        }
    }

    fn watching(&mut self, show_name: String, episode: Episode, paths: &[String]) -> Exit {
        for path in paths {
            let args: Vec<&str> = vec![&path];

            let status = Command::new("mpv")
                .args(&args)
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            if !status.success() {
                continue;
            }

            // TODO: It has to look up again when it already
            // did in `select_ep`.
            //
            // Can't pass `&mut Anime` into here because it's
            // being mutably borrowed by `self` as well.
            //
            // Would preferrably not have to keep looking up
            // `Anime` again.
            self.database
                .animes()
                .unwrap()
                .iter_mut()
                .find(|(s, _)| show_name.eq(*s))
                .expect("You just played it...")
                .1
                .update_watched(episode)
                .unwrap();
            break;
        }
        self.select_ep(show_name)
    }

    fn quit(&mut self, exitcode: i32) -> Exit {
        match exitcode {
            exitcode::OK => {
                self.database.write(DB_FILE.as_str()).unwrap();
                Ok(exitcode::OK)
            }
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

fn main() -> Result<()> {
    match Sani::start() {
        Ok(v) => process::exit(v),
        Err(e) => process::exit(e),
    };
}
