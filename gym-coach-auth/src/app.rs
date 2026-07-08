use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use crate::validate_register_input;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::time::{sleep, Duration as TokioDuration};
use tokio_postgres::{Client, NoTls, Row};
use uuid::Uuid;

const AUTH_PORT: u16 = 8081;
const JWT_SECRET: &str = "gym-coach-super-secret";
const DEFAULT_DATABASE_URL: &str = "postgres://gymcoach:gymcoach@127.0.0.1:5432/gymcoach";
const DEMO_COACH_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEMO_CLIENT_ID: &str = "22222222-2222-2222-2222-222222222222";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum UserRole {
    Coach,
    Client,
}

impl UserRole {
    fn as_db_value(&self) -> &'static str {
        match self {
            Self::Coach => "COACH",
            Self::Client => "CLIENT",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "COACH" => Some(Self::Coach),
            "CLIENT" => Some(Self::Client),
            _ => None,
        }
    }
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
    database_url: Arc<String>,
}

pub async fn run() {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let db = connect_db(&database_url).await.unwrap();
    init_db(&db).await.unwrap();

    let state = AppState {
        database_url: Arc::new(database_url),
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

async fn connect_db(database_url: &str) -> Result<Client, tokio_postgres::Error> {
    for attempt in 1..=20 {
        match tokio_postgres::connect(database_url, NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(error) = connection.await {
                        eprintln!("Auth DB connection error: {error}");
                    }
                });
                return Ok(client);
            }
            Err(error) if attempt < 20 => {
                eprintln!("Auth DB connect attempt {attempt} failed: {error}");
                sleep(TokioDuration::from_secs(2)).await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!()
}

async fn db_client(state: &AppState) -> Result<Client, tokio_postgres::Error> {
    connect_db(state.database_url.as_str()).await
}

async fn init_db(db: &Client) -> Result<(), tokio_postgres::Error> {
    db.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS auth_users (
            id UUID PRIMARY KEY,
            full_name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL
        );
        ",
    )
    .await?;

    for user in seed_users() {
        db.execute(
            "
            INSERT INTO auth_users (id, full_name, email, password_hash, role, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (email) DO NOTHING
            ",
            &[
                &user.id,
                &user.full_name,
                &user.email,
                &user.password_hash,
                &user.role.as_db_value(),
                &user.created_at,
            ],
        )
        .await?;
    }

    Ok(())
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
    validate_register_input(&payload.full_name, &payload.email, &payload.password)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    let db = db_client(&state).await.map_err(db_error)?;
    let email = payload.email.trim().to_lowercase();
    let existing = db
        .query_opt("SELECT 1 FROM auth_users WHERE email = $1", &[&email])
        .await
        .map_err(db_error)?;

    if existing.is_some() {
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

    db
        .execute(
            "
            INSERT INTO auth_users (id, full_name, email, password_hash, role, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ",
            &[
                &account.id,
                &account.full_name,
                &account.email,
                &account.password_hash,
                &account.role.as_db_value(),
                &account.created_at,
            ],
        )
        .await
        .map_err(db_error)?;

    let token = token_for(&account).map_err(internal_error)?;
    let response = AuthResponse {
        token,
        user: public_user(&account),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<serde_json::Value>)> {
    let db = db_client(&state).await.map_err(db_error)?;
    let email = payload.email.trim().to_lowercase();
    let row = db
        .query_opt(
            "
            SELECT id, full_name, email, password_hash, role, created_at
            FROM auth_users
            WHERE email = $1
            ",
            &[&email],
        )
        .await
        .map_err(db_error)?
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    let account = user_from_row(&row).map_err(internal_state_error)?;

    if account.password_hash != hash_password(&payload.password) {
        return Err(error(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let token = token_for(&account).map_err(internal_error)?;
    Ok(Json(AuthResponse {
        token,
        user: public_user(&account),
    }))
}

async fn me(headers: HeaderMap) -> impl IntoResponse {
    let Some(token) = bearer_token(&headers) else {
        return Err(error(StatusCode::UNAUTHORIZED, "Missing bearer token"));
    };

    let claims =
        decode_claims(token).map_err(|_| error(StatusCode::UNAUTHORIZED, "Invalid token"))?;
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

fn seed_users() -> Vec<UserAccount> {
    vec![
        UserAccount {
            id: Uuid::parse_str(DEMO_COACH_ID).unwrap(),
            full_name: "Mina Coach".into(),
            email: "coach@gymcoach.rs".into(),
            password_hash: hash_password("coach123"),
            role: UserRole::Coach,
            created_at: Utc::now(),
        },
        UserAccount {
            id: Uuid::parse_str(DEMO_CLIENT_ID).unwrap(),
            full_name: "Nikola Client".into(),
            email: "client@gymcoach.rs".into(),
            password_hash: hash_password("client123"),
            role: UserRole::Client,
            created_at: Utc::now(),
        },
    ]
}

fn user_from_row(row: &Row) -> Result<UserAccount, String> {
    let role: String = row.get("role");
    Ok(UserAccount {
        id: row.get("id"),
        full_name: row.get("full_name"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        role: UserRole::from_db_value(&role).ok_or_else(|| format!("Unknown role: {role}"))?,
        created_at: row.get("created_at"),
    })
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

fn db_error(_: tokio_postgres::Error) -> (StatusCode, Json<serde_json::Value>) {
    error(StatusCode::INTERNAL_SERVER_ERROR, "Database operation failed")
}

fn internal_state_error(message: String) -> (StatusCode, Json<serde_json::Value>) {
    error(StatusCode::INTERNAL_SERVER_ERROR, &message)
}

