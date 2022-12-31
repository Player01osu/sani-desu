pub const IMPORTS: &str = r#"
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = normal;
    PRAGMA temp_store = memory;
    PRAGMA mmap_size = 30000000000;

    CREATE TABLE IF NOT EXISTS anime (
        dir_name TEXT NOT NULL,
        current_ep INT,
        current_s INT,
        last_watched INT,

        PRIMARY KEY (dir_name)
    );

    CREATE TABLE IF NOT EXISTS location (
        location TEXT PRIMARY KEY NOT NULL,
        dir_name TEXT NOT NULL,

        CONSTRAINT fk_dir_name
        FOREIGN KEY (dir_name)
        REFERENCES anime (dir_name)
    );

    CREATE TABLE IF NOT EXISTS episode (
        path TEXT PRIMARY KEY UNIQUE NOT NULL,
        dir_name TEXT NOT NULL,
        ep INT,
        s INT,
        special TEXT,

        CONSTRAINT fk_dir_name
        FOREIGN KEY (dir_name)
        REFERENCES anime (dir_name)
    );

    CREATE UNIQUE INDEX IF NOT EXISTS filename_idx
    ON anime(dir_name);

    CREATE INDEX IF NOT EXISTS episode_season_idx
    ON episode(ep, s);
"#;
