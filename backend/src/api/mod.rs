//! REST API module
//!
//! This module provides the HTTP server and REST API endpoints including:
//! - API routing and request handling
//! - Authentication and authorization middleware
//! - Rate limiting
//! - Error handling and response formatting

pub mod server;
pub mod routes;
pub mod middleware;
pub mod handlers;
pub mod models;

pub use server::ApiServer;
pub use models::{ErrorResponse, SearchQuery, SearchResponse};
pub use middleware::{trace_id_middleware, TraceId, TRACE_ID_HEADER};
