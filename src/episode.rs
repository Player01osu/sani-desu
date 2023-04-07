use std::str::FromStr;

use crate::{cache::EpisodeNumbered, REG_PARSE_OUT, REG_SPECIAL, REG_EPS};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum EpisodeKind {
    Numbered(EpisodeNumbered),
    Special(String),
}

impl Default for EpisodeKind {
    fn default() -> Self {
        EpisodeKind::Numbered(EpisodeNumbered::default())
    }
}

#[derive(Debug, Default)]
pub struct Episode {
    pub dir_name: String,
    pub ep: EpisodeKind,
}

impl PartialEq for Episode {
    fn eq(&self, other: &Self) -> bool {
        if let EpisodeKind::Numbered(ref ep_s) = self.ep {
            if let EpisodeKind::Numbered(ref ep_s_other) = other.ep {
                return ep_s == ep_s_other;
            }
        }
        false
    }
}

impl PartialOrd for Episode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {

        if let EpisodeKind::Numbered(ref ep_s) = self.ep {
            if let EpisodeKind::Numbered(ref ep_s_other) = other.ep {
                return ep_s.partial_cmp(ep_s_other);
            }
        }
        if let EpisodeKind::Special(ref special) = self.ep {
            if let EpisodeKind::Special(ref special_other) = other.ep {
                return special.partial_cmp(special_other);
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
        if let EpisodeKind::Numbered(ref ep_s) = &self.ep {
            if let EpisodeKind::Numbered(ref ep_s_other) = &other.ep {
                return ep_s.cmp(ep_s_other);
            }
        }
        if let EpisodeKind::Special(ref special) = self.ep {
            if let EpisodeKind::Special(ref special_other) = other.ep {
                return special.cmp(special_other);
            }
        }
        match self.ep {
            EpisodeKind::Numbered(_) => std::cmp::Ordering::Greater,
            EpisodeKind::Special(_) => std::cmp::Ordering::Less,
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

impl FromStr for Episode {
    type Err = anyhow::Error;

    fn from_str(filename: &str) -> Result<Self, Self::Err> {
        let special_iter = REG_SPECIAL.find(filename);
        if let Some(i) = special_iter {
            let episode_str = i.as_str().to_owned();
            let ep = EpisodeKind::Special(episode_str);
            return Ok(Episode {
                dir_name: filename.to_owned(),
                ep,
            });
        }

        let parsed_out = REG_PARSE_OUT.replace_all(filename, "");
        let parsed_out = parsed_out.as_ref();

        let Some(captures) = REG_EPS.captures(parsed_out) else {
            return Ok(Self { dir_name: filename.to_owned(), ep: EpisodeKind::Numbered(EpisodeNumbered { ep: 1, s: 1 })});
        };

        let ep = captures.name("e").unwrap().as_str().parse().expect("Should not fail");
        let s = captures.name("s").map(|m| m.as_str().parse().expect("Should not fail")).unwrap_or(1);

        Ok(Episode {
            dir_name: filename.to_owned(),
            ep: EpisodeKind::Numbered(EpisodeNumbered { ep, s }),
        })
    }
}
