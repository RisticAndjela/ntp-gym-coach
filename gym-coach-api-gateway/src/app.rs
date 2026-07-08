use axum::{
    body::Bytes,
    extract::{OriginalUri, State},
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
    Json, Router,
};
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

const GATEWAY_PORT: u16 = 8080;
const JWT_SECRET: &str = "gym-coach-super-secret";

#[derive(Debug, Serialize)]
struct StatusResponse {
    status: String,
    service: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct VersionResponse {
    version: String,
    build_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum UserRole {
    Coach,
    Client,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    email: String,
    role: UserRole,
    exp: usize,
    iat: usize,
}

#[derive(Clone)]
struct AppState {
    client: Client,
    services: Arc<HashMap<&'static str, String>>,
}

pub async fn run() {
    let state = AppState {
        client: Client::new(),
        services: Arc::new(HashMap::from([
            (
                "auth",
                env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            ),
            (
                "users",
                env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8082".into()),
            ),
            (
                "trainings",
                env::var("TRAINING_SERVICE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8083".into()),
            ),
            (
                "programs",
                env::var("PROGRAM_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".into()),
            ),
            (
                "analytics",
                env::var("ANALYTICS_SERVICE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8085".into()),
            ),
        ])),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/version", get(get_version))
        .route("/api/auth", any(proxy_auth))
        .route("/api/auth/*path", any(proxy_auth))
        .route("/api/users", any(proxy_users))
        .route("/api/users/*path", any(proxy_users))
        .route("/api/trainings", any(proxy_trainings))
        .route("/api/trainings/*path", any(proxy_trainings))
        .route("/api/programs", any(proxy_programs))
        .route("/api/programs/*path", any(proxy_programs))
        .route("/api/analytics", any(proxy_analytics))
        .route("/api/analytics/*path", any(proxy_analytics))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .with_state(state);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{GATEWAY_PORT}").parse().unwrap();
    println!("API Gateway listening on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health_check() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "UP".into(),
        service: "Gym Coach API Gateway".into(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

async fn get_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: "1.0.0".into(),
        build_date: "2026-04-28".into(),
    })
}

async fn proxy_auth(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    proxy_request(state, "auth", method, headers, uri.0, body, false).await
}

async fn proxy_users(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    proxy_request(state, "users", method, headers, uri.0, body, true).await
}

async fn proxy_trainings(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    proxy_request(state, "trainings", method, headers, uri.0, body, true).await
}

async fn proxy_programs(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    proxy_request(state, "programs", method, headers, uri.0, body, true).await
}

async fn proxy_analytics(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    uri: OriginalUri,
    body: Bytes,
) -> Response {
    proxy_request(state, "analytics", method, headers, uri.0, body, true).await
}

async fn proxy_request(
    state: AppState,
    service_key: &str,
    method: Method,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
    require_auth: bool,
) -> Response {
    let claims = if require_auth {
        match authorize(&headers) {
            Ok(claims) => Some(claims),
            Err(response) => return response,
        }
    } else {
        None
    };

    let Some(base_url) = state.services.get(service_key) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Service not configured" })),
        )
            .into_response();
    };

    let downstream_path = uri
        .path_and_query()
        .map(|value| value.as_str().replacen("/api", "", 1))
        .unwrap_or_else(|| uri.path().replacen("/api", "", 1));
    let url = format!("{base_url}{downstream_path}");

    let mut request = state.client.request(method, url).body(body);
    for (name, value) in &headers {
        if name != header::HOST {
            request = request.header(name, value);
        }
    }

    if let Some(claims) = claims {
        request = request
            .header("x-user-id", claims.sub)
            .header("x-user-role", serde_json::to_string(&claims.role).unwrap().replace('"', ""));
    }

    match request.send().await {
        Ok(response) => to_axum_response(response).await,
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": "Failed to reach downstream service"
            })),
        )
            .into_response(),
    }
}

fn authorize(headers: &HeaderMap) -> Result<Claims, Response> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Missing bearer token" })),
            )
                .into_response()
        })?;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid token" })),
        )
            .into_response()
    })
}

async fn to_axum_response(response: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let headers = response.headers().clone();
    let body = response.bytes().await.unwrap_or_default();

    let mut builder = Response::builder().status(status);
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }

    builder.body(axum::body::Body::from(body)).unwrap()
}

