use super::CacheAnimeInfo;

#[derive(Default)]
pub struct CacheAnimeInfoBuilder<'cache> {
    anime_name: Option<&'cache str>,
    filename: Option<&'cache str>,
    current_ep: Option<u32>,
    timestamp: Option<&'cache str>,
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

    pub fn finalize(self) -> CacheAnimeInfo<'cache> {
        CacheAnimeInfo {
            anime_name: self.anime_name.unwrap(),
            filename: self.filename.unwrap(),
            current_ep: self.current_ep.unwrap(),
            timestamp: self.timestamp.unwrap(),
        }
    }
}

