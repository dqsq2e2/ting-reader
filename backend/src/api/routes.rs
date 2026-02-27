//! API routes

use crate::api::handlers::{
    create_book, delete_book, get_book, list_books, update_book, scrape_book,
    search_books, get_scraper_sources, scraper_search, get_scraper_detail,
    list_plugins, get_plugin_detail, install_plugin, reload_plugin, uninstall_plugin,
    get_plugin_config, update_plugin_config,
    list_tasks, get_task, cancel_task,
    health_check, get_metrics,
    get_config, update_config,
    get_book_chapters, update_chapter,
    batch_update_chapters, scrape_book_diff, apply_scrape_result,
    merge_books, move_chapters,
    generate_regex,
    get_tags,
    get_stats,
    // Progress management
    get_recent_progress, get_book_progress, update_progress,
    // Favorites management
    get_favorites, add_favorite, remove_favorite,
    // User settings
    get_user_settings, update_user_settings,
    // User management (admin)
    list_users, create_user, update_user, delete_user,
    // Library management
    list_libraries, create_library, update_library, delete_library, scan_library, get_storage_folders,
    // Cache management
    cache_chapter, get_cache_list, delete_chapter_cache, clear_all_caches,
    // Proxy API
    proxy_cover,
    // Audio streaming
    stream_chapter,
    AppState,
};
use crate::auth::handlers::{register, login, get_me, update_me};
use crate::auth::middleware::authenticate;
use axum::{
    routing::{get, post, put, patch},
    Router,
    middleware,
};

/// Build the API routes
pub fn build_api_routes(state: AppState) -> Router {
    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        // Statistics endpoint (public)
        .route("/api/v1/stats", get(get_stats))
        .route("/api/stats", get(get_stats))
        // Health check (public)
        .route("/api/v1/health", get(health_check))
        .route("/api/health", get(health_check));

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // User endpoints
        .route("/api/me", get(get_me).patch(update_me))
        // Progress management endpoints
        .route("/api/progress/recent", get(get_recent_progress))
        .route("/api/progress/:bookId", get(get_book_progress))
        .route("/api/progress", post(update_progress))
        // Favorites management endpoints
        .route("/api/favorites", get(get_favorites))
        .route("/api/favorites/:bookId", post(add_favorite).delete(remove_favorite))
        // User settings endpoints
        .route("/api/settings", get(get_user_settings).post(update_user_settings))
        // User management endpoints (admin only)
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", patch(update_user).delete(delete_user))
        // Library management endpoints
        .route("/api/libraries", get(list_libraries).post(create_library))
        .route("/api/libraries/:id", patch(update_library).delete(delete_library))
        .route("/api/libraries/:id/scan", post(scan_library))
        .route("/api/storage/folders", get(get_storage_folders))
        // Book CRUD endpoints (with /v1 prefix)
        .route("/api/v1/books", get(list_books).post(create_book))
        .route(
            "/api/v1/books/:id",
            get(get_book).put(update_book).patch(update_book).delete(delete_book),
        )
        .route("/api/v1/books/:id/scrape", post(scrape_book))
        .route("/api/v1/books/:id/scrape-diff", post(scrape_book_diff))
        .route("/api/v1/books/:id/scrape-apply", post(apply_scrape_result))
        .route("/api/v1/books/merge", post(merge_books))
        .route("/api/v1/books/chapters/move", post(move_chapters))
        .route("/api/v1/tools/regex/generate", post(generate_regex))
        .route("/api/v1/books/:id/chapters", get(get_book_chapters))
        .route("/api/v1/books/:id/chapters/batch", put(batch_update_chapters).post(batch_update_chapters))
        // Chapter endpoints
        .route("/api/v1/chapters/:id", patch(update_chapter))
        // Tags endpoint
        .route("/api/v1/tags", get(get_tags))
        // Search and scraper endpoints
        .route("/api/v1/search", get(search_books))
        .route("/api/v1/scraper/sources", get(get_scraper_sources))
        .route("/api/v1/scraper/search", post(scraper_search))
        .route("/api/v1/scraper/detail/:source/:id", get(get_scraper_detail))
        // Plugin management endpoints
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/:id", get(get_plugin_detail).delete(uninstall_plugin))
        .route("/api/v1/plugins/install", post(install_plugin))
        .route("/api/v1/plugins/:id/reload", post(reload_plugin))
        .route("/api/v1/plugins/:id/config", get(get_plugin_config).put(update_plugin_config))
        // Task management endpoints
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/:id", get(get_task))
        .route("/api/v1/tasks/:id/cancel", post(cancel_task))
        // System management endpoints
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/config", get(get_config).put(update_config))
        // Book CRUD endpoints (without /v1 prefix for frontend compatibility)
        .route("/api/books", get(list_books).post(create_book))
        .route(
            "/api/books/:id",
            get(get_book).put(update_book).patch(update_book).delete(delete_book),
        )
        .route("/api/books/:id/scrape", post(scrape_book))
        .route("/api/books/:id/scrape-diff", post(scrape_book_diff))
        .route("/api/books/:id/scrape-apply", post(apply_scrape_result))
        .route("/api/books/merge", post(merge_books))
        .route("/api/books/chapters/move", post(move_chapters))
        .route("/api/tools/regex/generate", post(generate_regex))
        .route("/api/books/:id/chapters", get(get_book_chapters))
        .route("/api/books/:id/chapters/batch", put(batch_update_chapters).post(batch_update_chapters))
        // Chapter endpoints (without /v1)
        .route("/api/chapters/:id", patch(update_chapter))
        // Tags endpoint (without /v1)
        .route("/api/tags", get(get_tags))
        // Search and scraper endpoints (without /v1)
        .route("/api/search", get(search_books))
        .route("/api/scraper/sources", get(get_scraper_sources))
        .route("/api/scraper/search", post(scraper_search))
        .route("/api/scraper/detail/:source/:id", get(get_scraper_detail))
        // Plugin management endpoints (without /v1)
        .route("/api/plugins", get(list_plugins))
        .route("/api/plugins/:id", get(get_plugin_detail).delete(uninstall_plugin))
        .route("/api/plugins/install", post(install_plugin))
        .route("/api/plugins/:id/reload", post(reload_plugin))
        .route("/api/plugins/:id/config", get(get_plugin_config).put(update_plugin_config))
        // Task management endpoints (without /v1)
        .route("/api/tasks", get(list_tasks))
        .route("/api/tasks/:id", get(get_task))
        .route("/api/tasks/:id/cancel", post(cancel_task))
        // System management endpoints (without /v1)
        .route("/api/metrics", get(get_metrics))
        .route("/api/config", get(get_config).put(update_config))
        // Cache management endpoints
        .route("/api/cache/:chapterId", post(cache_chapter).delete(delete_chapter_cache))
        .route("/api/cache", get(get_cache_list).delete(clear_all_caches))
        // Proxy API endpoints
        .route("/api/proxy/cover", get(proxy_cover))
        // Audio streaming endpoints
        .route("/api/stream/:chapterId", get(stream_chapter))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate));

    // Combine public and protected routes
    public_routes
        .merge(protected_routes)
        .with_state(state)
}
