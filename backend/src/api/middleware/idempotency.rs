use axum::{
    body::{to_bytes, Body, Bytes},
    extract::Request,
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::{watch, Mutex};

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(120);
const MAX_CACHED_RESPONSE_SIZE: usize = 2 * 1024 * 1024;

#[derive(Clone)]
struct CachedResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    expires_at: Instant,
}

enum Entry {
    InFlight(watch::Sender<bool>),
    Complete(CachedResponse),
}

fn entries() -> &'static Mutex<HashMap<String, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn supports_idempotency(method: &Method) -> bool {
    matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    )
}

fn response_from_cache(cached: CachedResponse) -> Response {
    let mut response = Response::new(Body::from(cached.body));
    *response.status_mut() = cached.status;
    *response.headers_mut() = cached.headers;
    response
}

pub async fn idempotency(request: Request, next: Next) -> Response {
    if !supports_idempotency(request.method()) {
        return next.run(request).await;
    }

    let Some(idempotency_key) = request
        .headers()
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
    else {
        return next.run(request).await;
    };

    let authorization = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let cache_key = format!(
        "{}|{}|{}|{}",
        authorization,
        request.method(),
        request
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or_default(),
        idempotency_key
    );

    loop {
        let wait_for = {
            let mut entries = entries().lock().await;
            entries.retain(|_, entry| match entry {
                Entry::InFlight(_) => true,
                Entry::Complete(cached) => cached.expires_at > Instant::now(),
            });

            match entries.get(&cache_key) {
                Some(Entry::Complete(cached)) => return response_from_cache(cached.clone()),
                Some(Entry::InFlight(sender)) => Some(sender.subscribe()),
                None => {
                    let (sender, _) = watch::channel(false);
                    entries.insert(cache_key.clone(), Entry::InFlight(sender));
                    None
                }
            }
        };

        if let Some(mut completion) = wait_for {
            if !*completion.borrow() {
                let _ = completion.changed().await;
            }
            continue;
        }
        break;
    }

    let response = next.run(request).await;
    let status = response.status();
    let headers = response.headers().clone();
    let (parts, body) = response.into_parts();
    let body = match to_bytes(body, MAX_CACHED_RESPONSE_SIZE).await {
        Ok(body) => body,
        Err(_) => {
            let completion = {
                let mut entries = entries().lock().await;
                match entries.remove(&cache_key) {
                    Some(Entry::InFlight(sender)) => Some(sender),
                    _ => None,
                }
            };
            if let Some(completion) = completion {
                let _ = completion.send(true);
            }
            return Response::from_parts(parts, Body::empty());
        }
    };
    let response = Response::from_parts(parts, Body::from(body.clone()));

    let completion = {
        let mut entries = entries().lock().await;
        match entries.remove(&cache_key) {
            Some(Entry::InFlight(sender)) => {
                if status.is_success() {
                    entries.insert(
                        cache_key,
                        Entry::Complete(CachedResponse {
                            status,
                            headers,
                            body,
                            expires_at: Instant::now() + IDEMPOTENCY_TTL,
                        }),
                    );
                }
                Some(sender)
            }
            _ => None,
        }
    };
    if let Some(completion) = completion {
        let _ = completion.send(true);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{middleware, routing::post, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    async fn mutation() -> &'static str {
        CALLS.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        "created"
    }

    #[tokio::test]
    async fn concurrent_requests_with_same_key_execute_once() {
        CALLS.store(0, Ordering::SeqCst);
        entries().lock().await.clear();
        let app = Router::new()
            .route("/resource", post(mutation))
            .layer(middleware::from_fn(idempotency));

        let request = || {
            Request::builder()
                .method(Method::POST)
                .uri("/resource")
                .header("idempotency-key", "same-key")
                .body(Body::empty())
                .unwrap()
        };
        let (first, second) = tokio::join!(
            app.clone().oneshot(request()),
            app.clone().oneshot(request())
        );

        assert_eq!(first.unwrap().status(), StatusCode::OK);
        assert_eq!(second.unwrap().status(), StatusCode::OK);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }
}
