//! Plugin HTTP endpoints grouped by workflow. Internal policies stay within this subsystem.

mod assets;
mod authorization;
mod capabilities;
mod installation;
mod management;
mod routes;
mod scraper;

pub use assets::get_plugin_asset;
pub use capabilities::{
    find_content_processors, find_event_handlers, find_task_handlers, find_tool_providers,
    invoke_plugin_capability, invoke_plugin_host, list_plugin_capabilities,
};
pub use installation::{
    StorePluginsQuery, clear_plugin_cache, get_store_plugins, install_plugin, install_store_plugin,
};
pub use management::{
    get_plugin_config, get_plugin_detail, list_plugins, reload_plugin, uninstall_plugin,
    update_plugin_config,
};
pub use routes::{call_plugin_route, call_public_plugin_route, sign_plugin_route};
pub use scraper::{add_source_book, get_scraper_sources, scraper_search};

#[cfg(test)]
#[path = "../../../../tests/unit/api/plugins.rs"]
mod tests;
