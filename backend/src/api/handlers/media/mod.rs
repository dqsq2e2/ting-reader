//! Media handlers: audio streaming, caching, and cover proxying

pub mod cache;
pub mod proxy;
pub mod stream;

pub use cache::{cache_chapter, clear_all_caches, delete_chapter_cache, get_cache_list};
pub use proxy::{proxy_cover, ProxyCoverQuery};
pub use stream::{stream_chapter, StreamQuery};
