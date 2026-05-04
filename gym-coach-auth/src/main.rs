use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

const AUTH_PORT: u16 = 8081;
const JWT_SECRET: &str = "gym-coach-super-secret";
const DEMO_COACH_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEMO_CLIENT_ID: &str = "22222222-2222-2222-2222-222222222222";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum UserRole {
    Coach,
    Client,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserAccount {
    id: Uuid,
    full_name: String,
    email: String,
    password_hash: String,
    role: UserRole,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PublicUser {
    id: Uuid,
    full_name: String,
    email: String,
    role: UserRole,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    email: String,
    role: UserRole,
    exp: usize,
    iat: usize,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    full_name: String,
    email: String,
    password: String,
    role: UserRole,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
    user: PublicUser,
}

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: String,
    service: String,
}

#[derive(Clone)]
struct AppState {
    users: Arc<Mutex<HashMap<String, UserAccount>>>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        users: Arc::new(Mutex::new(seed_users())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/me", get(me))
        .with_state(state);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{AUTH_PORT}").parse().unwrap();
    println!("Auth service listening on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "AuthService".into(),
    })
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), (StatusCode, Json<serde_json::Value>)> {
    let mut users = state.users.lock().await;
    let email = payload.email.trim().to_lowercase();

    if users.contains_key(&email) {
        return Err(error(
            StatusCode::CONFLICT,
            "User with this email already exists",
        ));
    }

    let account = UserAccount {
        id: Uuid::new_v4(),
        full_name: payload.full_name.trim().to_string(),
        email: email.clone(),
        password_hash: hash_password(&payload.password),
        role: payload.role,
        created_at: Utc::now(),
    };

    let token = token_for(&account).map_err(internal_error)?;
    let response = AuthResponse {
        token,
        user: public_user(&account),
    };

    users.insert(email, account);
    Ok((StatusCode::CREATED, Json(response)))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<serde_json::Value>)> {
    let users = state.users.lock().await;
    let email = payload.email.trim().to_lowercase();
    let account = users
        .get(&email)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    if account.password_hash != hash_password(&payload.password) {
        return Err(error(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let token = token_for(account).map_err(internal_error)?;
    Ok(Json(AuthResponse {
        token,
        user: public_user(account),
    }))
}

async fn me(headers: HeaderMap) -> impl IntoResponse {
    let Some(token) = bearer_token(&headers) else {
        return Err(error(StatusCode::UNAUTHORIZED, "Missing bearer token"));
    };

    let claims = decode_claims(token).map_err(|_| error(StatusCode::UNAUTHORIZED, "Invalid token"))?;
    Ok(Json(claims))
}

fn public_user(user: &UserAccount) -> PublicUser {
    PublicUser {
        id: user.id,
        full_name: user.full_name.clone(),
        email: user.email.clone(),
        role: user.role.clone(),
    }
}

fn token_for(user: &UserAccount) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::hours(24)).timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
}

fn decode_claims(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let header_value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    header_value.strip_prefix("Bearer ")
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn seed_users() -> HashMap<String, UserAccount> {
    let coach = UserAccount {
        id: Uuid::parse_str(DEMO_COACH_ID).unwrap(),
        full_name: "Mina Coach".into(),
        email: "coach@gymcoach.rs".into(),
        password_hash: hash_password("coach123"),
        role: UserRole::Coach,
        created_at: Utc::now(),
    };
    let client = UserAccount {
        id: Uuid::parse_str(DEMO_CLIENT_ID).unwrap(),
        full_name: "Nikola Client".into(),
        email: "client@gymcoach.rs".into(),
        password_hash: hash_password("client123"),
        role: UserRole::Client,
        created_at: Utc::now(),
    };

    HashMap::from([
        (coach.email.clone(), coach),
        (client.email.clone(), client),
    ])
}

fn error(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": message
        })),
    )
}

fn internal_error(_: jsonwebtoken::errors::Error) -> (StatusCode, Json<serde_json::Value>) {
    error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to issue token")
}
