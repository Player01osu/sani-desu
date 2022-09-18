mod setup;

use std::process::{Command, Stdio};
use setup::EnvironmentVars;

fn main() {
    // Setup stage:
    // 1. Grab or set environment variables
    //    - Anime location
    //    - Dmenu settings
    // 2. Parse command line arguments
    // 3. Ensure all require programs exist
    //    - Mpv
    //    - Dmenu
    //    - Ls
    // 4. Check for cache folder
    //    - First $SANI_CACHE
    //    - Then $XDG_CACHE_HOME/sani
    //    - Create if not exist
    // 5. Check for locally saved anime.json
    //    - First $SANI_ANIME_JSON
    //    - Then $XDG_DATA_HOME/sani/anime.json
    //    - Create if not exist
    // 6. Check for config folder
    //    - First $SANI_CONFIG
    //    - Then $XDG_CONFIG_HOME/sani/config
    //    - Then $XDG_CONFIG_HOME/sani_config
    //    - Lastly $HOME/.sani_config
    //    - Create if not exist
    // 7. Validate config
    //    - Warn invalid config
    //    - Log
    //    - Guide to wiki
    // 8. Start program
    let env = EnvironmentVars::new();

    let ls = Command::new("/bin/ls")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
        .stdout
        .unwrap();
    let child = Command::new("dmenu").stdin(ls).spawn().unwrap();
}
