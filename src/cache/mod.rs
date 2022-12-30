mod auto;

use std::{
    fs,
    ops::Sub,
    path::Path,
    thread,
};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use std::hash::Hash;

use crate::{
    episode::{Episode, EpisodeSpecial},
    CONFIG, ENV,
};

use self::auto::IMPORTS;

type Directory = String;

#[derive(Clone, Debug)]
pub struct EpisodeSeason {
    pub ep: u32,
    pub s: u32,
}


impl PartialEq for EpisodeSeason {
    fn eq(&self, other: &Self) -> bool {
        self.ep == other.ep && self.s == other.s
    }
}

impl PartialOrd for EpisodeSeason {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        if self.s > other.s {
            Some(Ordering::Greater)
        } else if self.s < other.s {
            Some(Ordering::Less)
        } else if self.ep > other.ep {
            Some(Ordering::Greater)
        } else if self.ep < other.ep {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl Eq for EpisodeSeason {}

impl Ord for EpisodeSeason {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.s > other.s {
            Ordering::Greater
        } else if self.s < other.s {
            Ordering::Less
        } else if self.ep > other.ep {
            Ordering::Greater
        } else if self.ep < other.ep {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

impl Hash for EpisodeSeason {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.s.hash(state);
        self.ep.hash(state);
    }
}

impl Default for EpisodeSeason {
    fn default() -> Self {
        Self { ep: 1, s: 1 }
    }
}

pub struct Cache<'cache> {
    pub directory: &'cache str,
    pub current_ep_s: EpisodeSeason,
    pub next_ep_s: Option<EpisodeSeason>,
    sqlite_conn: Connection,
}

#[derive(Serialize, Deserialize)]
struct CacheInfo {
    cached_ani: Vec<CacheAnimeInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CacheAnimeInfo {
    pub dir_name: String,
    pub episode: u32,
    pub season: u32,
}

#[derive(Debug)]
pub struct RelativeEpisode {
    pub current_ep: EpisodeSeason,
    pub next_ep: Option<EpisodeSeason>,
}

impl<'cache> Cache<'cache> {
    pub fn new(cache: &str) -> Self {
        let db_file = format!("{cache}/sani.db");

        let mut join_thread = false;
        if !Path::new(&db_file).is_file() {
            join_thread = true;
        }

        let sqlite_conn = Connection::open(&db_file)
            .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
            .unwrap();

        sqlite_conn.execute_batch(IMPORTS).unwrap();

        let thread = thread::spawn(|| {
            let cache = &ENV.cache;
            let db_file = format!("{cache}/sani.db");

            let sqlite_conn = Connection::open(db_file)
                .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
                .unwrap();

            let mut stmt_anime = sqlite_conn
                .prepare_cached(
                    r#"
                INSERT OR IGNORE INTO anime (dir_name)
                VALUES (?1)
            "#,
                )
                .unwrap();
            let mut stmt_location = sqlite_conn
                .prepare_cached(
                    r#"
                INSERT OR IGNORE INTO anime (location)
                VALUES (?1)
            "#,
                )
                .unwrap();
            let list = CONFIG
                .anime_dir
                .iter()
                .flat_map(|v| fs::read_dir(v).unwrap().map(|d| d.unwrap().path()));
            for i in list {
                stmt_anime.execute(params![
                    i.file_name().unwrap().to_string_lossy(),
                ])
                .unwrap();
                stmt_location.execute(params![
                    i.to_string_lossy()
                ])
                .unwrap();
            }

            let mut stmt = sqlite_conn
                .prepare_cached(
                    r#"
                INSERT OR IGNORE INTO episode (path, dir_name, ep, s, special)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                )
                .unwrap();
            let list = CONFIG.anime_dir.iter().flat_map(|v| {
                WalkDir::new(v)
                    .max_depth(5)
                    .min_depth(2)
                    .into_iter()
                    .map(|d| {
                        let path = d.as_ref().unwrap();
                        let episode = Episode::from_filename(path.file_name().to_str().unwrap());
                        let mut anime_directory = path.path().parent().unwrap();

                        // Walk to parent directory
                        for _ in 0..path.depth().sub(2) {
                            anime_directory = anime_directory.parent().unwrap();
                        }
                        (
                            path.path().to_str().unwrap().to_owned(),
                            anime_directory
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            episode.ep,
                        )
                    })
            });
            for i in list {
                match i.2 {
                    EpisodeSpecial::EpS(ep_s) => {
                        stmt.execute(params![i.0, i.1, ep_s.ep, ep_s.s, None::<String>])
                            .unwrap();
                    }
                    EpisodeSpecial::Special(special) => {
                        stmt.execute(params![i.0, i.1, None::<u32>, None::<u32>, special.trim()])
                            .unwrap();
                    }
                }
            }
        });

        // Wait for thread if database has not been created yet.
        if join_thread {
            thread.join().unwrap();
        }

        Self {
            sqlite_conn,
            directory: Default::default(),
            current_ep_s: Default::default(),
            next_ep_s: Default::default(),
        }
    }

    pub fn find_ep(&self, directory: &str, episode: &EpisodeSpecial) -> Option<Vec<String>> {
        match episode {
            EpisodeSpecial::EpS(ep_s) => {
                let mut stmt = self
                    .sqlite_conn
                    .prepare_cached(
                        r#"
            SELECT path
            FROM episode
            WHERE ep = ?1 AND s = ?2 AND dir_name = ?3
            "#,
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(params![ep_s.ep, ep_s.s, directory], |rows| rows.get(0))
                    .ok();
                rows.map(|rows| {
                    rows.into_iter()
                        .map(|r| r.unwrap())
                        .collect::<Vec<String>>()
                })
            }
            EpisodeSpecial::Special(special) => {
                let mut stmt = self
                    .sqlite_conn
                    .prepare_cached(
                        r#"
            SELECT path
            FROM episode
            WHERE special = ?1 AND dir_name = ?2
            "#,
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(params![special, directory], |rows| rows.get(0))
                    .ok();
                let paths = rows.map(|rows| {
                    rows.into_iter()
                        .map(|r| r.unwrap())
                        .collect::<Vec<String>>()
                });
                if let Some(ref paths) = paths {
                    if paths.is_empty() {
                        return None;
                    }
                }
                paths
            }
        }
    }

    pub fn write_finished(&mut self, current_ep: EpisodeSeason, next_ep: Option<EpisodeSeason>) {
        self.current_ep_s = current_ep;
        self.next_ep_s = next_ep;
    }

    pub fn read_ep(&self, anime_dir: &str) -> Result<Vec<EpisodeSpecial>> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT episode.ep, episode.s, episode.special
            FROM anime
            INNER JOIN episode
            ON anime.dir_name = episode.dir_name
            WHERE anime.dir_name = ?
        "#,
        )?;
        let records = stmt.query_map([anime_dir], |row| {
            match row.get_unwrap::<_, Option<String>>(2) {
                Some(special) => Ok(EpisodeSpecial::Special(special)),
                None => Ok(EpisodeSpecial::EpS(EpisodeSeason {
                    ep: row.get_unwrap(0),
                    s: row.get_unwrap(1),
                })),
            }
        });
        use itertools::Itertools;

        let mut list = records
            .unwrap()
            .map(|v| v.unwrap())
            .unique()
            .collect::<Vec<EpisodeSpecial>>();
        list.sort();

        Ok(list)
    }

    pub fn write(&self, info: CacheAnimeInfo) -> Result<()> {
        thread::spawn(move || {
            let cache = &ENV.cache;
            let db_file = format!("{cache}/sani.db");

            let sqlite_conn = Connection::open(db_file)
                .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
                .unwrap();

            use chrono::prelude::Utc;
            let unix = Utc::now().timestamp();

            let mut stmt = sqlite_conn
                .prepare_cached(
                    r#"
            INSERT OR REPLACE
            INTO anime (dir_name, current_ep, current_s, last_watched)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
                )
                .unwrap();
            stmt.execute(params![
                info.dir_name,
                info.episode,
                info.season,
                unix
            ])
            .unwrap();
        });

        Ok(())
    }

    pub fn check_ep_s_exist_cache(
        &self,
        directory: impl AsRef<str> + std::fmt::Debug,
        ep_s: &EpisodeSeason,
    ) -> Result<bool> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT 1
            FROM episode
            WHERE dir_name = ?1 AND ep = ?2 AND s = ?3
            "#,
        )?;
        stmt.exists(params![directory.as_ref(), ep_s.ep, ep_s.s])
            .map_err(|e| anyhow!(e))
    }

    pub fn next_ep(
        &self,
        directory: impl AsRef<str>,
        current_ep_s: &EpisodeSeason,
    ) -> Option<EpisodeSeason> {
        let next_ep_s = EpisodeSeason {
            ep: current_ep_s.ep + 1,
            s: current_ep_s.s,
        };

        if self
            .check_ep_s_exist_cache(directory.as_ref(), &next_ep_s)
            .unwrap()
        {
            return Some(next_ep_s);
        }
        // Check next season
        let next_ep_s = EpisodeSeason {
            ep: 1,
            s: current_ep_s.s + 1,
        };
        if self
            .check_ep_s_exist_cache(directory.as_ref(), &next_ep_s)
            .unwrap()
        {
            return Some(next_ep_s);
        }
        None
    }

    pub fn read_relative_ep(&self, directory: &str) -> Result<RelativeEpisode> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.current_ep, anime.current_s
            FROM anime
            INNER JOIN episode
            ON episode.ep = anime.current_ep AND episode.s = anime.current_s
            WHERE anime.dir_name = ?
            "#,
        )?;
        let binding: Result<EpisodeSeason, rusqlite::Error> = stmt.query_row([directory], |row| {
            Ok(EpisodeSeason {
                ep: row.get(0).unwrap(),
                s: row.get(1).unwrap(),
            })
        });
        let current_ep = match binding {
            Ok(v) => v,
            Err(_e) => EpisodeSeason { ep: 1, s: 1 },
        };

        let next_ep = self.next_ep(directory, &current_ep);

        Ok(RelativeEpisode {
            current_ep,
            next_ep,
        })
    }

    pub fn close(self) {
        self.sqlite_conn.execute(r"pragma optimize", []).unwrap();
    }

    pub fn read_list(&self) -> Result<Directory> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.dir_name, location.location
            FROM anime
            INNER JOIN location ON anime.dir_name = location.dir_name
            ORDER BY anime.last_watched DESC, anime.dir_name DESC
        "#,
        )?;
        let directory = stmt
            .query_map([], |row| {
                let directory: Directory = row.get_unwrap(0);
                let location: Directory = row.get_unwrap(1);
                if Path::new(&location).exists() {
                    return Ok(directory);
                }

                // Filler error
                Err(rusqlite::Error::InvalidQuery)
            })?
            .into_iter()
            .filter_map(|v| v.ok())
            .collect::<Vec<Directory>>();
        let directory = directory.join("\n");

        Ok(directory)
    }
}
