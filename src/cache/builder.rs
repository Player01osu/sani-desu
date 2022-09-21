use super::CacheAnimeInfo;
use anyhow::Result;

#[derive(Default)]
pub struct CacheAnimeInfoBuilder<'cache> {
    anime_name: Option<&'cache str>,
    filename: Option<&'cache str>,
    current_ep: Option<u32>,
    timestamp: Option<&'cache str>,
}

pub enum CacheAnimeInfoBuilderError<'a> {
    MissingField(&'a str),
}

impl<'cache> CacheAnimeInfoBuilder<'cache> {
    pub fn anime_name(mut self, anime_name: &'cache str) -> Self {
        self.anime_name = Some(anime_name);
        self
    }

    pub fn filename(mut self, filename: &'cache str) -> Self {
        self.filename = Some(filename);
        self
    }

    pub fn current_ep(mut self, current_ep: u32) -> Self {
        self.current_ep = Some(current_ep);
        self
    }

    pub fn timestamp(mut self, timestamp: &'cache str) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    pub fn finalize(self) -> Result<CacheAnimeInfo<'cache>, CacheAnimeInfoBuilderError<'cache>> {
        let anime_name = match self.anime_name {
            Some(v) => v,
            None => {
                return Err(CacheAnimeInfoBuilderError::MissingField(
                    "Missing field: anime_name",
                ))
            }
        };

        let filename = match self.filename {
            Some(v) => v,
            None => {
                return Err(CacheAnimeInfoBuilderError::MissingField(
                    "Missing field: filename",
                ))
            }
        };

        let current_ep = match self.current_ep {
            Some(v) => v,
            None => {
                return Err(CacheAnimeInfoBuilderError::MissingField(
                    "Missing field: current_ep",
                ))
            }
        };

        let timestamp = match self.timestamp {
            Some(v) => v,
            None => {
                return Err(CacheAnimeInfoBuilderError::MissingField(
                    "Missing field: timestamp",
                ))
            }
        };

        Ok(CacheAnimeInfo {
            anime_name,
            filename,
            current_ep,
            timestamp,
        })
    }
}
