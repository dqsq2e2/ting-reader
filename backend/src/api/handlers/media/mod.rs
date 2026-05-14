//! Media handlers: audio streaming, caching, and cover proxying

pub mod cache;
pub mod proxy;
pub mod stream;

pub use cache::{cache_chapter, get_cache_list, delete_chapter_cache, clear_all_caches};
pub use proxy::{proxy_cover, ProxyCoverQuery};
pub use stream::{stream_chapter, StreamQuery};
