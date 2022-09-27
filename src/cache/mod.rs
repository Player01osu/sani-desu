use std::{
    any::Any,
    fs::{self, File},
    path::Path,
    rc::Rc,
    thread,
};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{AppState, Episode, Sani, CONFIG, ENV};

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
    pub filename: &'cache str,
    pub fullpath: &'cache str,
    pub episode: u32,
    pub season: u32,
    pub timestamp: u64,
}

#[derive(Default, Debug)]
pub struct EpisodeLayout {
    pub episode: u32,
    pub season: u32,
    pub filename: String,
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
                next_s INT DEFAULT 1 NOT NULL
            );
            CREATE TABLE IF NOT EXISTS episode (
                filename TEXT PRIMARY KEY NOT NULL,
                directory TEXT NOT NULL,
                fullpath TEXT NOT NULL,
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
                INSERT OR IGNORE INTO episode (filename, directory, fullpath, episode, season)
                VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
                )
                .unwrap();
            let list = CONFIG.anime_dir.iter().flat_map(|v| {
                WalkDir::new(v)
                    .max_depth(2)
                    .min_depth(2)
                    .into_iter()
                    .map(|d| {
                        let dir = d.as_ref().unwrap();
                        let episode = Episode::parse_ep(dir.file_name().to_str().unwrap());
                        (
                            dir.file_name().to_str().unwrap().to_owned(),
                            dir.path()
                                .parent()
                                .unwrap()
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            dir.path().to_str().unwrap().to_owned(),
                            episode.episode,
                            episode.season,
                        )
                    })
            });
            for i in list {
                dbg!(&i);
                stmt.execute(params![i.0, i.1, i.2, i.3, i.4]).unwrap();
            }
        });

        Self {
            sqlite_conn,
            directory: Default::default(),
            current_ep_s: Default::default(),
            next_ep_s: Default::default(),
        }
    }

    pub fn write_finished(&mut self, current_ep: EpisodeLayout, next_ep: EpisodeLayout) {
        self.current_ep_s = current_ep;
        self.next_ep_s = next_ep;
    }

    pub fn write(&self, info: CacheAnimeInfo) -> Result<()> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            INSERT OR REPLACE
            INTO anime (directory, current_ep, current_s, next_ep, next_s)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )?;
        let next_ep = info.episode + 1;
        dbg!(stmt
            .execute(params![
                info.directory,
                info.episode,
                info.season,
                next_ep,
                info.season
            ])
            .unwrap());
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            INSERT OR REPLACE
            INTO episode (filename, directory, fullpath, episode, season, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )?;
        dbg!(&info);
        dbg!(info.filename);
        dbg!(stmt
            .execute(params![
                info.filename,
                info.directory,
                info.fullpath,
                info.episode,
                info.season,
                info.timestamp,
            ])
            .unwrap());

        Ok(())
    }

    pub fn read_relative_ep(&self, directory: &str) -> Result<RelativeEpisode> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.current_ep, anime.current_s, episode.filename
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
                filename: row.get(2).unwrap(),
            })
        });
        let current_ep = match binding {
            Ok(v) => v,
            Err(e) => {
                dbg!(&e);
                EpisodeLayout {
                    episode: 1,
                    season: 1,
                    filename: String::default(),
                }
            }
        };

        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT anime.next_ep, anime.next_s, episode.filename
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
                filename: row.get(2).unwrap(),
            })
        });
        let next_ep = match binding {
            Ok(v) => v,
            Err(e) => {
                dbg!(&e);
                EpisodeLayout {
                    episode: 1,
                    season: 1,
                    filename: String::default(),
                }
            }
        };
        Ok(RelativeEpisode {
            current_ep,
            next_ep,
        })
    }

    pub fn read_timestamp(&self, filename: &str) -> Result<u64> {
        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            SELECT timestamp
            FROM episode
            WHERE filename = ?
        "#,
            )
            .unwrap();
        dbg!(filename);

        let timestamp: Result<u64, rusqlite::Error> = stmt.query_row([filename], |row| row.get(0));
        match timestamp {
            Ok(v) => return Ok(v),
            Err(e) => {
                dbg!(e);
                return Ok(0);
            }
        }
    }
}
