//! API routes

use crate::api::handlers::media::stream::{
    get_hls_playlist, get_hls_segment, seek_hls_stream, stream_signed_chapter,
};
use crate::api::handlers::{
    add_favorite,
    apply_scrape_result,
    batch_delete_tasks,
    batch_update_chapters,
    // Cache management
    cache_chapter,
    call_plugin_route,
    call_public_plugin_route,
    cancel_task,
    check_update,
    clear_all_caches,
    clear_plugin_cache,
    clear_recent_progress,
    clear_system_logs,
    clear_tasks,
    create_book,
    create_library,
    create_notification_webhook,
    create_playlist,
    create_series,
    create_user,
    delete_book,
    delete_chapter_cache,
    delete_library,
    delete_notification_webhook,
    delete_playlist,
    delete_progress_history,
    delete_series,
    delete_task,
    delete_user,
    export_system_logs,
    find_content_processors,
    find_event_handlers,
    find_task_handlers,
    find_tool_providers,
    generate_regex,
    get_admin_statistics,
    get_book,
    get_book_chapters,
    get_book_progress,
    get_cache_list,
    get_config,
    // Favorites management
    get_favorites,
    get_metrics,
    get_playlist,
    get_plugin_asset,
    get_plugin_config,
    get_plugin_detail,
    // Progress management
    get_recent_progress,
    get_scraper_sources,
    get_series,
    get_stats,
    get_storage_folders,
    get_storage_roots,
    get_store_plugins,
    get_system_logs,
    get_tags,
    get_task,
    // User settings
    get_user_settings,
    // System management endpoints
    health_check,
    install_plugin,
    install_store_plugin,
    invoke_plugin_capability,
    invoke_plugin_host,
    list_books,
    list_libraries,
    list_notification_events,
    list_notification_webhooks,
    list_playlists,
    // Library management
    list_plugin_capabilities,
    list_plugins,
    // Series management
    list_series,
    list_tasks,
    // User management (admin)
    list_users,
    merge_books,
    move_chapters,
    // Proxy API
    proxy_cover,
    reload_plugin,
    remove_favorite,
    scan_library,
    scrape_book_diff,
    scraper_search,
    search_books,
    // Audio streaming
    sign_plugin_route,
    stream_chapter,
    test_notification_webhook,
    test_webdav_connection,
    uninstall_plugin,
    update_book,
    update_chapter,
    update_config,
    update_library,
    update_notification_webhook,
    update_playlist,
    update_plugin_config,
    update_progress,
    update_series,
    update_user,
    update_user_settings,
    write_book_metadata_to_files,
    AppState,
};
use crate::auth::handlers::{get_me, update_me};
use crate::auth::middleware::authenticate;
use axum::{
    middleware,
    routing::{any, get, patch, post, put},
    Router,
};

use axum::extract::DefaultBodyLimit;

/// Helper macro to register a route on both /api/v1 and /api prefixes
macro_rules! api_route {
    ($router:expr, $path:expr, $($method:ident($handler:expr)),+ $(,)?) => {
        $(
            $router = $router
                .route(concat!("/api/v1", $path), $method($handler))
                .route(concat!("/api", $path), $method($handler));
        )+
    };
}

/// Build the API routes
pub fn build_api_routes(state: AppState) -> Router {
    // Public routes (no authentication required)
    let mut public_routes = Router::new();
    api_route!(public_routes, "/stats", get(get_stats));
    api_route!(public_routes, "/health", get(health_check));
    public_routes = public_routes
        .route("/api/v1/plugin-assets/:id/*path", get(get_plugin_asset))
        .route("/api/plugin-assets/:id/*path", get(get_plugin_asset))
        .route(
            "/api/v1/public/plugin-routes/*path",
            any(call_public_plugin_route),
        )
        .route(
            "/api/public/plugin-routes/*path",
            any(call_public_plugin_route),
        );

    // HLS streaming endpoints (public - session ID provides security)
    public_routes = public_routes
        .route(
            "/api/stream/hls/:sessionId/playlist.m3u8",
            get(get_hls_playlist),
        )
        .route("/api/stream/hls/:sessionId/:filename", get(get_hls_segment))
        .route("/api/stream/hls/:sessionId/seek", post(seek_hls_stream))
        .route(
            "/api/v1/public/media/:chapterId",
            get(stream_signed_chapter).head(stream_signed_chapter),
        )
        .route(
            "/api/public/media/:chapterId",
            get(stream_signed_chapter).head(stream_signed_chapter),
        );

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // User endpoints
        .route("/api/me", get(get_me).patch(update_me))
        // Progress management endpoints
        .route(
            "/api/progress/recent",
            get(get_recent_progress).delete(clear_recent_progress),
        )
        .route("/api/progress/recent/delete", post(delete_progress_history))
        .route("/api/progress/:bookId", get(get_book_progress))
        .route("/api/progress", post(update_progress))
        // Favorites management endpoints
        .route("/api/favorites", get(get_favorites))
        .route(
            "/api/favorites/:bookId",
            post(add_favorite).delete(remove_favorite),
        )
        // Playlist endpoints
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/:id",
            get(get_playlist)
                .put(update_playlist)
                .delete(delete_playlist),
        )
        .route(
            "/api/v1/playlists",
            get(list_playlists).post(create_playlist),
        )
        .route(
            "/api/v1/playlists/:id",
            get(get_playlist)
                .put(update_playlist)
                .delete(delete_playlist),
        )
        // User settings endpoints
        .route(
            "/api/settings",
            get(get_user_settings).post(update_user_settings),
        )
        // User management endpoints (admin only)
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", patch(update_user).delete(delete_user))
        // Library management endpoints
        .route("/api/libraries", get(list_libraries).post(create_library))
        .route(
            "/api/libraries/:id",
            patch(update_library).delete(delete_library),
        )
        .route("/api/libraries/:id/scan", post(scan_library))
        .route(
            "/api/libraries/test-connection",
            post(test_webdav_connection),
        )
        .route("/api/storage/roots", get(get_storage_roots))
        .route("/api/storage/folders", get(get_storage_folders))
        // Series management endpoints
        .route("/api/v1/series", get(list_series).post(create_series))
        .route(
            "/api/v1/series/:id",
            get(get_series).put(update_series).delete(delete_series),
        )
        // Book CRUD endpoints (with /v1 prefix)
        .route("/api/v1/books", get(list_books).post(create_book))
        .route(
            "/api/v1/books/:id",
            get(get_book)
                .put(update_book)
                .patch(update_book)
                .delete(delete_book),
        )
        .route("/api/v1/books/:id/scrape-diff", post(scrape_book_diff))
        .route("/api/v1/books/:id/scrape-apply", post(apply_scrape_result))
        .route("/api/v1/books/merge", post(merge_books))
        .route("/api/v1/books/chapters/move", post(move_chapters))
        .route("/api/v1/tools/regex/generate", post(generate_regex))
        .route("/api/v1/books/:id/chapters", get(get_book_chapters))
        .route(
            "/api/v1/books/:id/chapters/batch",
            put(batch_update_chapters).post(batch_update_chapters),
        )
        // Chapter endpoints
        .route("/api/v1/chapters/:id", patch(update_chapter))
        // Tags endpoint
        .route("/api/v1/tags", get(get_tags))
        // Search and scraper endpoints
        .route("/api/v1/search", get(search_books))
        .route("/api/v1/scraper/sources", get(get_scraper_sources))
        .route("/api/v1/scraper/search", post(scraper_search))
        // Plugin management endpoints
        .route("/api/v1/plugins", get(list_plugins))
        .route(
            "/api/v1/plugins/:id",
            get(get_plugin_detail).delete(uninstall_plugin),
        )
        .route(
            "/api/v1/plugins/install",
            post(install_plugin).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        ) // 50MB limit for plugin upload
        .route("/api/v1/plugins/:id/reload", post(reload_plugin))
        .route(
            "/api/v1/plugins/:id/config",
            get(get_plugin_config).put(update_plugin_config),
        )
        .route(
            "/api/v1/plugins/:id/capabilities/:capabilityId/invoke",
            post(invoke_plugin_capability),
        )
        .route("/api/v1/plugin-host/invoke", post(invoke_plugin_host))
        .route("/api/v1/plugin-capabilities", get(list_plugin_capabilities))
        .route(
            "/api/v1/plugin-capabilities/content-processors",
            get(find_content_processors),
        )
        .route(
            "/api/v1/plugin-capabilities/tools",
            get(find_tool_providers),
        )
        .route(
            "/api/v1/plugin-capabilities/task-handlers",
            get(find_task_handlers),
        )
        .route(
            "/api/v1/plugin-capabilities/event-handlers",
            get(find_event_handlers),
        )
        .route("/api/v1/plugin-route-signatures", post(sign_plugin_route))
        .route("/api/v1/plugin-routes/*path", any(call_plugin_route))
        // Plugin store endpoints
        .route("/api/v1/store/plugins", get(get_store_plugins))
        .route("/api/v1/store/install", post(install_store_plugin))
        .route("/api/v1/store/cache/clear", post(clear_plugin_cache))
        // Task management endpoints
        .route("/api/v1/tasks", get(list_tasks).delete(clear_tasks))
        .route("/api/v1/tasks/:id", get(get_task).delete(delete_task))
        .route("/api/v1/tasks/:id/cancel", post(cancel_task))
        .route("/api/v1/tasks/batch-delete", post(batch_delete_tasks))
        // System management endpoints
        .route("/api/v1/system/statistics", get(get_admin_statistics))
        .route("/api/v1/system/metrics", get(get_metrics))
        .route("/api/v1/system/config", get(get_config).put(update_config))
        .route(
            "/api/v1/system/notifications",
            get(list_notification_webhooks).post(create_notification_webhook),
        )
        .route(
            "/api/v1/system/notifications/events",
            get(list_notification_events),
        )
        .route(
            "/api/v1/system/notifications/test",
            post(test_notification_webhook),
        )
        .route(
            "/api/v1/system/notifications/:id",
            put(update_notification_webhook).delete(delete_notification_webhook),
        )
        .route("/api/v1/system/check-update", get(check_update))
        .route(
            "/api/v1/system/logs",
            get(get_system_logs).delete(clear_system_logs),
        )
        .route("/api/v1/system/logs/export", get(export_system_logs))
        // Book CRUD endpoints (without /v1 prefix for frontend compatibility)
        .route("/api/books", get(list_books).post(create_book))
        .route(
            "/api/books/:id",
            get(get_book)
                .put(update_book)
                .patch(update_book)
                .delete(delete_book),
        )
        .route("/api/books/:id/scrape-diff", post(scrape_book_diff))
        .route("/api/books/:id/scrape-apply", post(apply_scrape_result))
        .route("/api/books/merge", post(merge_books))
        .route("/api/books/chapters/move", post(move_chapters))
        .route(
            "/api/books/:id/write-metadata",
            post(write_book_metadata_to_files),
        )
        .route("/api/tools/regex/generate", post(generate_regex))
        .route("/api/books/:id/chapters", get(get_book_chapters))
        .route(
            "/api/books/:id/chapters/batch",
            put(batch_update_chapters).post(batch_update_chapters),
        )
        // Chapter endpoints (without /v1)
        .route("/api/chapters/:id", patch(update_chapter))
        // Tags endpoint (without /v1)
        .route("/api/tags", get(get_tags))
        // Search and scraper endpoints (without /v1)
        .route("/api/search", get(search_books))
        .route("/api/scraper/sources", get(get_scraper_sources))
        .route("/api/scraper/search", post(scraper_search))
        // Plugin management endpoints (without /v1)
        .route("/api/plugins", get(list_plugins))
        .route(
            "/api/plugins/:id",
            get(get_plugin_detail).delete(uninstall_plugin),
        )
        .route(
            "/api/plugins/install",
            post(install_plugin).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        ) // 50MB limit for plugin upload
        .route("/api/plugins/:id/reload", post(reload_plugin))
        .route(
            "/api/plugins/:id/config",
            get(get_plugin_config).put(update_plugin_config),
        )
        .route(
            "/api/plugins/:id/capabilities/:capabilityId/invoke",
            post(invoke_plugin_capability),
        )
        .route("/api/plugin-host/invoke", post(invoke_plugin_host))
        .route("/api/plugin-capabilities", get(list_plugin_capabilities))
        .route(
            "/api/plugin-capabilities/content-processors",
            get(find_content_processors),
        )
        .route("/api/plugin-capabilities/tools", get(find_tool_providers))
        .route(
            "/api/plugin-capabilities/task-handlers",
            get(find_task_handlers),
        )
        .route(
            "/api/plugin-capabilities/event-handlers",
            get(find_event_handlers),
        )
        .route("/api/plugin-route-signatures", post(sign_plugin_route))
        .route("/api/plugin-routes/*path", any(call_plugin_route))
        // Plugin store endpoints (without /v1)
        .route("/api/store/plugins", get(get_store_plugins))
        .route("/api/store/install", post(install_store_plugin))
        .route("/api/store/cache/clear", post(clear_plugin_cache))
        // Task management endpoints (without /v1)
        .route("/api/tasks", get(list_tasks).delete(clear_tasks))
        .route("/api/tasks/:id", get(get_task).delete(delete_task))
        .route("/api/tasks/:id/cancel", post(cancel_task))
        .route("/api/tasks/batch-delete", post(batch_delete_tasks))
        // System management endpoints (without /v1)
        .route("/api/system/statistics", get(get_admin_statistics))
        .route("/api/system/metrics", get(get_metrics))
        .route("/api/system/config", get(get_config).put(update_config))
        .route(
            "/api/system/notifications",
            get(list_notification_webhooks).post(create_notification_webhook),
        )
        .route(
            "/api/system/notifications/events",
            get(list_notification_events),
        )
        .route(
            "/api/system/notifications/test",
            post(test_notification_webhook),
        )
        .route(
            "/api/system/notifications/:id",
            put(update_notification_webhook).delete(delete_notification_webhook),
        )
        .route("/api/system/check-update", get(check_update))
        .route(
            "/api/system/logs",
            get(get_system_logs).delete(clear_system_logs),
        )
        .route("/api/system/logs/export", get(export_system_logs))
        // Cache management endpoints
        .route(
            "/api/cache/:chapterId",
            post(cache_chapter).delete(delete_chapter_cache),
        )
        .route("/api/cache", get(get_cache_list).delete(clear_all_caches))
        // Proxy API endpoints
        .route("/api/proxy/cover", get(proxy_cover))
        // Audio streaming endpoints
        .route(
            "/api/stream/:chapterId",
            get(stream_chapter).head(stream_chapter),
        )
        .layer(middleware::from_fn_with_state(state.clone(), authenticate));

    // Combine public and protected routes
    public_routes.merge(protected_routes).with_state(state)
}
