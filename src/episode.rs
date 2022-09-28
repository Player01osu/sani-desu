use crate::{REG_EP, REG_S, REG_PARSE_OUT};


#[derive(Debug, Default)]
pub struct Episode {
    pub fullpath: String,
    pub episode: u32,
    pub season: u32,
}

impl PartialEq for Episode {
    fn eq(&self, other: &Self) -> bool {
        self.episode == other.episode && self.season == other.season
    }
}

impl PartialOrd for Episode {
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

impl Eq for Episode {}

impl Ord for Episode {
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

impl Episode {
    pub fn parse_ep(filename: &str) -> Episode {
        let ep_iter = REG_EP.find(filename);
        let s_iter = REG_S.find(filename);

        let mut episode = 0u32;

        if let Some(i) = ep_iter {
            if !REG_PARSE_OUT.is_match(i.as_str()) {
                let episode_str = i
                    .as_str()
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>();
                episode = episode_str.parse::<u32>().unwrap();
            }
        }

        let mut season = 0u32;
        if let Some(i) = s_iter {
            if !REG_PARSE_OUT.is_match(i.as_str()) {
                let season_str = i
                    .as_str()
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>();
                season = season_str.parse::<u32>().unwrap();
            }
        }

        if episode != 0 && season == 0 {
            season = 1;
        }

        Episode {
            fullpath: filename.to_owned(),
            episode,
            season,
        }
    }
}

