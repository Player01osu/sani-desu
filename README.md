
# Sani-Desu

Keep track of and watch locally stored shows with this program.

## Building

### Requirements

* rustc 1.65.0
* sqlite3
* dmenu
* mpv

run:

```sh
cargo b --release
```

Copy the [config.yaml](./config.yaml) into $XDG_CONFIG_HOME/sani

## About

Sani-Desu searches for directories in provided folders and determines episode
and season numbers from file names; the directory names are used as the show
name when selecting shows. Sani-Desu will also cache the most recently watched
show, as well as the current episode.
