use crate::{cache::EpisodeSeason, REG_EP, REG_PARSE_OUT, REG_S, REG_SPECIAL};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum EpisodeSpecial {
    EpS(EpisodeSeason),
    Special(String),
}

impl Default for EpisodeSpecial {
    fn default() -> Self {
        EpisodeSpecial::EpS(EpisodeSeason::default())
    }
}

#[derive(Debug, Default)]
pub struct Episode {
    pub dir_name: String,
    pub ep: EpisodeSpecial,
}

impl PartialEq for Episode {
    fn eq(&self, other: &Self) -> bool {
        if let EpisodeSpecial::EpS(ref ep_s) = self.ep {
            if let EpisodeSpecial::EpS(ref ep_s_other) = other.ep {
                return ep_s == ep_s_other;
            }
        }
        false
    }
}

impl PartialOrd for Episode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        if let EpisodeSpecial::EpS(ref ep_s) = self.ep {
            if let EpisodeSpecial::EpS(ref ep_s_other) = other.ep {
                return ep_s.partial_cmp(&ep_s_other);
            }
        }
        if let EpisodeSpecial::Special(ref special) = self.ep {
            if let EpisodeSpecial::Special(ref special_other) = other.ep {
                return special.partial_cmp(&special_other);
            }
        }
        None

        //if self.season > other.season {
        //    Some(Ordering::Greater)
        //} else if self.season < other.season {
        //    Some(Ordering::Less)
        //} else if self.episode > other.episode {
        //    Some(Ordering::Greater)
        //} else if self.episode < other.episode {
        //    Some(Ordering::Less)
        //} else {
        //    Some(Ordering::Equal)
        //}
    }
}

impl Eq for Episode {}

impl Ord for Episode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if let EpisodeSpecial::EpS(ref ep_s) = &self.ep {
            if let EpisodeSpecial::EpS(ref ep_s_other) = &other.ep {
                return ep_s.cmp(&ep_s_other);
            }
        }
        if let EpisodeSpecial::Special(ref special) = self.ep {
            if let EpisodeSpecial::Special(ref special_other) = other.ep {
                return special.cmp(&special_other);
            }
        }
        match self.ep {
            EpisodeSpecial::EpS(_) => std::cmp::Ordering::Greater,
            EpisodeSpecial::Special(_) => std::cmp::Ordering::Less,
        }

        //use std::cmp::Ordering;
        //if self.season > other.season {
        //    Ordering::Greater
        //} else if self.season < other.season {
        //    Ordering::Less
        //} else if self.episode > other.episode {
        //    Ordering::Greater
        //} else if self.episode < other.episode {
        //    Ordering::Less
        //} else {
        //    Ordering::Equal
        //}
    }
}

impl Episode {
    pub fn from_filename(filename: &str) -> Episode {
        let mut ep = 1u32;
        let mut s = 1u32;

        let special_iter = REG_SPECIAL.find(filename);
        if let Some(i) = special_iter {
            let episode_str = i.as_str().to_owned();
            let ep = EpisodeSpecial::Special(episode_str);
            return Episode {
                dir_name: filename.to_owned(),
                ep,
            };
        }

        let ep_iter = REG_EP.find(filename);
        let s_iter = REG_S.find(filename);

        if let Some(i) = ep_iter {
            if !REG_PARSE_OUT.is_match(i.as_str()) {
                let ep_n = i
                    .as_str()
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap();
                ep = ep_n;
            }
        }

        if let Some(i) = s_iter {
            if !REG_PARSE_OUT.is_match(i.as_str()) {
                let s_n = i
                    .as_str()
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap();
                s = s_n;
            }
        }

        Episode {
            dir_name: filename.to_owned(),
            ep: EpisodeSpecial::EpS(EpisodeSeason {
                ep,
                s,
            })
        }
    }
}
