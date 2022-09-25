use std::{any::Any, fs::File, rc::Rc};

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub struct Cache {
    pub path: Rc<String>,
    sqlite_conn: Connection,
}

#[derive(Serialize, Deserialize)]
struct CacheInfo<'cache> {
    #[serde(borrow)]
    cached_ani: Vec<CacheAnimeInfo<'cache>>,
}

#[derive(Serialize, Deserialize)]
pub struct CacheAnimeInfo<'cache> {
    pub anime_name: &'cache str,
    pub filename: &'cache str,
    pub current_ep: u32,
    pub timestamp: u64,
}

impl Cache {
    pub fn new(cache: &str) -> Self {
        let db_file = format!("{cache}/sani.db");

        let sqlite_conn = Connection::open(&db_file)
            .map_err(|e| eprintln!("Failed to connect to sqlite database: {e}"))
            .unwrap();

        match sqlite_conn.execute(
            r#"
            CREATE TABLE anime (
                filename TEXT PRIMARY KEY NOT NULL,
                timestamp INT
            )"#,
            (),
        ) {
            Ok(_) => (),
            Err(_) => (),
        }

        match sqlite_conn.execute(
            r#"
                CREATE UNIQUE INDEX filename_idx
                ON anime(filename);
            )"#,
            (),
        ) {
            Ok(_) => (),
            Err(_) => (),
        }

        sqlite_conn
            .prepare_cached(
                r#"
            INSERT INTO anime (filename, timestamp)
            VALUES (?1, ?2)
        "#,
            )
            .unwrap();

        sqlite_conn
            .prepare_cached(
                r#"
            SELECT timestamp
            FROM anime
            WHERE 'filename' = ?1
        "#,
            )
            .unwrap();

        let cache = Rc::new(db_file);

        Self {
            path: cache,
            sqlite_conn,
        }
    }

    pub fn write(&self, info: CacheAnimeInfo) -> Result<()> {
        let mut stmt = self.sqlite_conn.prepare_cached(
            r#"
            SELECT timestamp
            FROM anime
            WHERE filename = ?
        "#,
        )?;
        dbg!(info.filename);
        let timestamp: Result<u64, rusqlite::Error> =
            stmt.query_row([info.filename], |row| row.get(0));
        match timestamp {
            Ok(_) => {
                eprintln!("UPDATING...");
                let mut stmt = self.sqlite_conn.prepare_cached(
                    r#"
                UPDATE anime SET timestamp = ?1 WHERE filename = ?2
            "#,
                )?;
                match stmt.execute((info.timestamp, info.filename)) {
                    Ok(_) => {
                        ();
                    }
                    Err(e) => {
                        dbg!(e);
                    }
                }
            }
            Err(_) => {
                eprintln!("INSERTING...");
                let mut stmt = self.sqlite_conn.prepare_cached(
                    r#"
                INSERT INTO anime (filename, timestamp)
                VALUES (?1, ?2)
            "#,
                )?;
                match stmt.execute((info.filename, info.timestamp)) {
                    Ok(_) => {
                        ();
                    }
                    Err(e) => {
                        dbg!(e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn read_timestamp(&self, filename: &str) -> Result<u64> {
        let mut stmt = self
            .sqlite_conn
            .prepare_cached(
                r#"
            SELECT timestamp
            FROM anime
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
