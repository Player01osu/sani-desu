pub const IMPORTS: &str = r#"
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

        CONSTRAINT fk_directory
        FOREIGN KEY (directory)
        REFERENCES anime (directory)
    );

    CREATE UNIQUE INDEX IF NOT EXISTS filename_idx
    ON anime(directory);

    CREATE INDEX IF NOT EXISTS episode_season_idx
    ON episode(episode, season);
"#;
