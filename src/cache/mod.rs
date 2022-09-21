use serde::{Serialize, Deserialize};
use anyhow::Result;
use self::builder::CacheAnimeInfoBuilder;

mod builder;
pub struct Cache<'setup> {
    pub cache: &'setup str,
}

#[derive(Serialize, Deserialize)]
struct CacheInfo<'cache> {
    #[serde(borrow)]
    cached_ani: Vec<CacheAnimeInfo<'cache>>,
}

#[derive(Serialize, Deserialize)]
pub struct CacheAnimeInfo<'cache> {
    anime_name: &'cache str,
    filename: &'cache str,
    current_ep: u32,
    timestamp: &'cache str,
}
impl<'cache> CacheAnimeInfo<'cache> {
    pub fn builder() -> CacheAnimeInfoBuilder<'cache> {
        CacheAnimeInfoBuilder::default()
    }
}

impl<'setup> Cache<'setup> {
    pub fn new(cache: &'setup str) -> Self {
        Self {
            cache: &cache
        }
    }

    pub fn write(&self, info: CacheAnimeInfo) -> Result<()>{
        Ok(())
    }

    pub fn read(&self) -> Result<()> {
        Ok(())
    }
}

