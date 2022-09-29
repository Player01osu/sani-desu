use std::{fs, ops::Sub, thread};

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{episode::Episode, CONFIG, ENV};
type Directory = String;

impl PartialEq for EpisodeSeason {
    fn eq(&self, other: &Self) -> bool {
        self.episode == other.episode && self.season == other.season
    }
}

impl PartialOrd for EpisodeSeason {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        if self.season > other.season {
            Some(Ordering::Greater)
        } else if self.season < other.season {
            Some(Ordering::Less)
        } else if self.episode > other.episode {
            Some(Ordering::Greater)
        } else if self.episode < other.episode {
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
        if self.season > other.season {
            Ordering::Greater
        } else if self.season < other.season {
            Ordering::Less
        } else if self.episode > other.episode {
            Ordering::Greater
        } else if self.episode < other.episode {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

pub struct EpisodeSeason {
    pub episode: u32,
    pub season: u32,
}

pub struct Cache<'cache> {
    pub directory: &'cache str,
    pub current_ep_s: EpisodeLayout,
    pub next_ep_s: EpisodeLayout,
    sqlite_conn: Connection,
}

#[derive(Serialize, Deserialize)]
struct CacheInfo<'cache> {
    #[serde(borrow)]
    cached_ani: Vec<CacheAnimeInfo<'cache>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CacheAnimeInfo<'cache> {
    pub directory: &'cache str,
    pub fullpath: &'cache str,
    pub episode: u32,
    pub season: u32,
    pub timestamp: u64,
}

#[derive(Default, Debug)]
pub struct EpisodeLayout {
    pub episode: u32,
    pub season: u32,
    pub fullpath: String,
}

#[derive(Debug)]
pub struct RelativeEpisode {
    pub next_ep: EpisodeLayout,
    pub current_ep: EpisodeLayout,
}

impl<'cache> Cache<'cache> {
    pub fn new(cache: &str) -> Self {
        let db_file = format!("{cache}/sani.db");

        let sqlite_conn = Connection::open(&db_file)
            .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
            .unwrap();

        sqlite_conn
            .execute_batch(
                r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = normal;
            PRAGMA temp_store = memory;
            PRAGMA mmap_size = 30000000000;
            CREATE TABLE IF NOT EXISTS anime (
                directory TEXT PRIMARY KEY UNIQUE NOT NULL,
                current_ep INT DEFAULT 1 NOT NULL,
                current_s INT DEFAULT 1 NOT NULL,
                next_ep INT DEFAULT 2 NOT NULL,
                next_s INT DEFAULT 1 NOT NULL,
                last_watched INT
            );
            CREATE TABLE IF NOT EXISTS episode (
                fullpath TEXT PRIMARY KEY UNIQUE NOT NULL,
                directory TEXT NOT NULL,
                episode INT DEFAULT 1 NOT NULL,
                season INT DEFAULT 1 NOT NULL,
                timestamp INT DEFAULT 0 NOT NULL,

                CONSTRAINT fk_directory
                FOREIGN KEY (directory)
                REFERENCES anime (directory)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS filename_idx
            ON anime(directory);

            CREATE INDEX IF NOT EXISTS episode_season_idx
            ON episode(episode, season);
            "#,
            )
            .unwrap();

        thread::spawn(|| {
            let cache = &ENV.cache;
            let db_file = format!("{cache}/sani.db");

            let sqlite_conn = Connection::open(&db_file)
                .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
                .unwrap();

            let mut stmt = sqlite_conn
                .prepare_cached(
                    r#"
                INSERT OR IGNORE INTO anime (directory)
                VALUES (?)
            "#,
                )
                .unwrap();
            let list = CONFIG
                .anime_dir
                .iter()
                .flat_map(|v| fs::read_dir(&v).unwrap().map(|d| d.unwrap().file_name()));
            for i in list {
                stmt.execute([i.to_string_lossy()]).unwrap();
            }

            let mut stmt = sqlite_conn
                .prepare_cached(
                    r#"
                INSERT OR IGNORE INTO episode (fullpath, directory, episode, season)
                VALUES (?1, ?2, ?3, ?4)
            "#,
                )
                .unwrap();
            let list = CONFIG.anime_dir.iter().flat_map(|v| {
                WalkDir::new(v)
                    .max_depth(3)
                    .min_depth(2)
                    .into_iter()
                    .map(|d| {
                        let dir = d.as_ref().unwrap();
                        let episode = Episode::parse_ep(dir.file_name().to_str().unwrap());
                        let mut anime_directory = dir.path().parent().unwrap();
                        dbg!(anime_directory);
                        dbg!(dir.depth());
                        for _ in 0..dir.depth().sub(2) {
                            anime_directory = anime_directory.parent().unwrap();
                            dbg!(anime_directory);
                        }
                        (
                            dir.path().to_str().unwrap().to_owned(),
                            anime_directory
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            episode.episode,
                            episode.season,
                        )
                    })
            });
            for i in list {
                dbg!(&i);
                stmt.execute(params![i.0, i.1, i.2, i.3]).unwrap();
            }
        });

        Self {
            sqlite_conn,
            directory: Default::default(),
            current_ep_s: Default::default(),
            next_ep_s: Default::default(),
        }
    }

    pub fn find_ep(&self, directory: &str, episode: EpisodeSeason) -> Option<String> {
        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            SELECT fullpath
            FROM episode
            WHERE episode = ?1 AND season = ?2 AND directory = ?3
            "#,
            )
            .unwrap();
        stmt.query_row(params![episode.episode, episode.season, directory], |row| {
            let directory: String = row.get_unwrap(0);
            Ok(directory)
        })
        .ok()
    }

    pub fn write_finished(&mut self, current_ep: EpisodeLayout, next_ep: EpisodeLayout) {
        self.current_ep_s = current_ep;
        self.next_ep_s = next_ep;
    }

    pub fn read_ep(&self, anime_dir: &str) -> Result<Vec<EpisodeSeason>> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT episode.episode, episode.season
            FROM anime
            INNER JOIN episode
            ON anime.directory = episode.directory
            WHERE anime.directory = ?
        "#,
        )?;
        let records = stmt.query_map([anime_dir], |row| {
            Ok(EpisodeSeason {
                episode: row.get_unwrap(0),
                season: row.get_unwrap(1),
            })
        });
        let list = records
            .unwrap()
            .map(|v| v.unwrap())
            .collect::<Vec<EpisodeSeason>>();

        Ok(list)
    }

    pub fn write(&self, info: CacheAnimeInfo) -> Result<()> {
        use chrono::prelude::Utc;
        let unix = Utc::now().timestamp();

        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            INSERT OR REPLACE
            INTO anime (directory, current_ep, current_s, next_ep, next_s, last_watched)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            )
            .unwrap();
        let next_ep = info.episode + 1;
        stmt.execute(params![
            info.directory,
            info.episode,
            info.season,
            next_ep,
            info.season,
            unix
        ])
        .unwrap();
        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            INSERT OR REPLACE
            INTO episode (directory, fullpath, episode, season, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            )
            .unwrap();
        stmt.execute(params![
            info.directory,
            info.fullpath,
            info.episode,
            info.season,
            info.timestamp,
        ])
        .unwrap();

        Ok(())
    }

    pub fn read_relative_ep(&self, directory: &str) -> Result<RelativeEpisode> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.current_ep, anime.current_s, episode.fullpath
            FROM anime
            INNER JOIN episode
            ON episode.episode = anime.current_ep AND episode.season = anime.current_s
            WHERE anime.directory = ?
            "#,
        )?;
        let binding: Result<EpisodeLayout, rusqlite::Error> = stmt.query_row([directory], |row| {
            Ok(EpisodeLayout {
                episode: row.get(0).unwrap(),
                season: row.get(1).unwrap(),
                fullpath: row.get(2).unwrap(),
            })
        });
        let current_ep = match binding {
            Ok(v) => v,
            Err(e) => {
                dbg!(&e);
                EpisodeLayout {
                    episode: 1,
                    season: 1,
                    fullpath: String::default(),
                }
            }
        };

        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.next_ep, anime.next_s, episode.fullpath
            FROM anime
            INNER JOIN episode
            ON episode.episode = anime.next_ep AND episode.season = anime.next_s
            WHERE anime.directory = ?
            "#,
        )?;
        let binding: Result<EpisodeLayout, rusqlite::Error> = stmt.query_row([directory], |row| {
            Ok(EpisodeLayout {
                episode: row.get(0).unwrap(),
                season: row.get(1).unwrap(),
                fullpath: row.get(2).unwrap(),
            })
        });
        let next_ep = match binding {
            Ok(v) => v,
            Err(e) => {
                dbg!(&e);
                EpisodeLayout {
                    episode: 1,
                    season: 1,
                    fullpath: String::default(),
                }
            }
        };
        Ok(RelativeEpisode {
            current_ep,
            next_ep,
        })
    }

    pub fn close(self) {
        self.sqlite_conn.execute(r"pragma optimize", []).unwrap();
    }

    pub fn read_current(&self, directory: &str) -> Result<String> {
        let relative = self.read_relative_ep(directory)?;
        Ok(relative.current_ep.fullpath)
    }

    pub fn read_next(&self, directory: &str) -> Result<String> {
        let relative = self.read_relative_ep(directory)?;
        Ok(relative.next_ep.fullpath)
    }

    pub fn read_list(&self) -> Result<Directory> {
        let mut stmt = self.sqlite_conn.prepare_cached(r#"
            SELECT directory
            FROM anime
            ORDER BY last_watched DESC, directory DESC
        "#)?;
        let directory = stmt.query_map([], |row| {
            let directory: Directory = row.get_unwrap(0);
            Ok(directory)
        })?.into_iter().map(|v| {
            v.unwrap()
        }).collect::<Vec<Directory>>();
        let directory = directory.join("\n");

        Ok(directory)
    }

    pub fn read_timestamp(&self, filename: &str) -> Result<u64> {
        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            SELECT timestamp
            FROM episode
            WHERE fullpath = ?
        "#,
            )
            .unwrap();
        dbg!(filename);

        let timestamp: Result<u64, rusqlite::Error> = stmt.query_row([filename], |row| row.get(0));
        match timestamp {
            Ok(v) => Ok(v),
            Err(e) => {
                dbg!(e);
                Ok(0)
            }
        }
    }
}
