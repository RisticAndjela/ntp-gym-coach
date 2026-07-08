use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::time::{sleep, Duration as TokioDuration};
use tokio_postgres::{types::Json as PgJson, Client, NoTls, Row};
use uuid::Uuid;

const USER_PORT: u16 = 8082;
const DEFAULT_DATABASE_URL: &str = "postgres://gymcoach:gymcoach@127.0.0.1:5432/gymcoach";
const DEMO_COACH_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEMO_CLIENT_ID: &str = "22222222-2222-2222-2222-222222222222";

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: String,
    service: String,
}

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
struct UserProfile {
    id: Uuid,
    full_name: String,
    email: String,
    role: UserRole,
    goals: Vec<String>,
    offers: Vec<String>,
    bio: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CoachClientLink {
    coach_id: Uuid,
    client_id: Uuid,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateConnectionRequest {
    coach_id: Uuid,
    client_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileRequest {
    full_name: Option<String>,
    goals: Option<Vec<String>>,
    offers: Option<Vec<String>>,
    bio: Option<String>,
}

#[derive(Debug, Serialize)]
struct CoachMatch {
    coach_id: Uuid,
    coach_name: String,
    matching_goals: Vec<String>,
    score: usize,
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
        .route("/users/profiles", get(list_profiles))
        .route("/users/profiles/:id", get(get_profile).put(update_profile))
        .route("/users/coaches", get(list_coaches))
        .route("/users/clients/:client_id/matches", get(match_coaches))
        .route("/users/connections", post(create_connection))
        .route("/users/connections/coach/:coach_id", get(get_clients_for_coach))
        .route("/users/connections/client/:client_id", get(get_coach_for_client))
        .with_state(state);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{USER_PORT}").parse().unwrap();
    println!("User service listening on http://{addr}");

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
                        eprintln!("User DB connection error: {error}");
                    }
                });
                return Ok(client);
            }
            Err(error) if attempt < 20 => {
                eprintln!("User DB connect attempt {attempt} failed: {error}");
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
        CREATE TABLE IF NOT EXISTS user_profiles (
            id UUID PRIMARY KEY,
            full_name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL,
            goals JSONB NOT NULL,
            offers JSONB NOT NULL,
            bio TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL
        );

        CREATE TABLE IF NOT EXISTS coach_client_links (
            coach_id UUID NOT NULL,
            client_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (coach_id, client_id)
        );
        ",
    )
    .await?;

    let (profiles, links) = seed_data();
    for profile in profiles {
        db.execute(
            "
            INSERT INTO user_profiles (id, full_name, email, role, goals, offers, bio, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO NOTHING
            ",
            &[
                &profile.id,
                &profile.full_name,
                &profile.email,
                &profile.role.as_db_value(),
                &PgJson(&profile.goals),
                &PgJson(&profile.offers),
                &profile.bio,
                &profile.created_at,
            ],
        )
        .await?;
    }

    for link in links {
        db.execute(
            "
            INSERT INTO coach_client_links (coach_id, client_id, created_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (coach_id, client_id) DO NOTHING
            ",
            &[&link.coach_id, &link.client_id, &link.created_at],
        )
        .await?;
    }

    Ok(())
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "UserService".into(),
    })
}

async fn list_profiles(State(state): State<AppState>) -> Result<Json<Vec<UserProfile>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            ORDER BY created_at, full_name
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let profiles = rows
        .iter()
        .map(profile_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(profiles))
}

async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserProfile>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = db
        .query_opt(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            WHERE id = $1
            ",
            &[&id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    profile_from_row(&row)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn update_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserProfile>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let current = db
        .query_opt(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            WHERE id = $1
            ",
            &[&id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut profile =
        profile_from_row(&current).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(full_name) = payload.full_name {
        profile.full_name = full_name;
    }
    if let Some(goals) = payload.goals {
        profile.goals = goals;
    }
    if let Some(offers) = payload.offers {
        profile.offers = offers;
    }
    if let Some(bio) = payload.bio {
        profile.bio = bio;
    }

    db
        .execute(
            "
            UPDATE user_profiles
            SET full_name = $2, goals = $3, offers = $4, bio = $5
            WHERE id = $1
            ",
            &[
                &id,
                &profile.full_name,
                &PgJson(&profile.goals),
                &PgJson(&profile.offers),
                &profile.bio,
            ],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(profile))
}

async fn list_coaches(State(state): State<AppState>) -> Result<Json<Vec<UserProfile>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            WHERE role = 'COACH'
            ORDER BY full_name
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let coaches = rows
        .iter()
        .map(profile_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(coaches))
}

async fn match_coaches(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<Vec<CoachMatch>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            ORDER BY full_name
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let profiles = rows
        .iter()
        .map(profile_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let client = profiles
        .iter()
        .find(|profile| profile.id == client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let matches = profiles
        .iter()
        .filter(|profile| profile.role == UserRole::Coach)
        .map(|coach| {
            let matching_goals: Vec<String> = client
                .goals
                .iter()
                .filter(|goal| coach.offers.iter().any(|offer| offer.eq_ignore_ascii_case(goal)))
                .cloned()
                .collect();

            CoachMatch {
                coach_id: coach.id,
                coach_name: coach.full_name.clone(),
                score: matching_goals.len(),
                matching_goals,
            }
        })
        .collect();

    Ok(Json(matches))
}

async fn create_connection(
    State(state): State<AppState>,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<(StatusCode, Json<CoachClientLink>), StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let coach_row = db
        .query_opt(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            WHERE id = $1
            ",
            &[&payload.coach_id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let client_row = db
        .query_opt(
            "
            SELECT id, full_name, email, role, goals, offers, bio, created_at
            FROM user_profiles
            WHERE id = $1
            ",
            &[&payload.client_id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let coach = profile_from_row(&coach_row).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let client = profile_from_row(&client_row).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if coach.role != UserRole::Coach || client.role != UserRole::Client {
        return Err(StatusCode::BAD_REQUEST);
    }

    let link = CoachClientLink {
        coach_id: payload.coach_id,
        client_id: payload.client_id,
        created_at: Utc::now(),
    };

    let inserted = db
        .execute(
            "
            INSERT INTO coach_client_links (coach_id, client_id, created_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (coach_id, client_id) DO NOTHING
            ",
            &[&link.coach_id, &link.client_id, &link.created_at],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if inserted == 0 {
        return Err(StatusCode::CONFLICT);
    }

    Ok((StatusCode::CREATED, Json(link)))
}

async fn get_clients_for_coach(
    State(state): State<AppState>,
    Path(coach_id): Path<Uuid>,
) -> Result<Json<Vec<CoachClientLink>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT coach_id, client_id, created_at
            FROM coach_client_links
            WHERE coach_id = $1
            ORDER BY created_at
            ",
            &[&coach_id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows.iter().map(link_from_row).collect()))
}

async fn get_coach_for_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<Vec<CoachClientLink>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT coach_id, client_id, created_at
            FROM coach_client_links
            WHERE client_id = $1
            ORDER BY created_at
            ",
            &[&client_id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows.iter().map(link_from_row).collect()))
}

fn seed_data() -> (Vec<UserProfile>, Vec<CoachClientLink>) {
    let coach_id = Uuid::parse_str(DEMO_COACH_ID).unwrap();
    let client_id = Uuid::parse_str(DEMO_CLIENT_ID).unwrap();

    let coach = UserProfile {
        id: coach_id,
        full_name: "Mina Coach".into(),
        email: "coach@gymcoach.rs".into(),
        role: UserRole::Coach,
        goals: vec![],
        offers: vec!["strength".into(), "fat loss".into(), "mobility".into()],
        bio: "Coach focused on structured strength progression.".into(),
        created_at: Utc::now(),
    };
    let client = UserProfile {
        id: client_id,
        full_name: "Nikola Client".into(),
        email: "client@gymcoach.rs".into(),
        role: UserRole::Client,
        goals: vec!["strength".into(), "fat loss".into()],
        offers: vec![],
        bio: "Client working on consistency and better squat form.".into(),
        created_at: Utc::now(),
    };
    let link = CoachClientLink {
        coach_id,
        client_id,
        created_at: Utc::now(),
    };

    (vec![coach, client], vec![link])
}

fn profile_from_row(row: &Row) -> Result<UserProfile, String> {
    let role: String = row.get("role");
    let goals: Value = row.get("goals");
    let offers: Value = row.get("offers");

    Ok(UserProfile {
        id: row.get("id"),
        full_name: row.get("full_name"),
        email: row.get("email"),
        role: UserRole::from_db_value(&role).ok_or_else(|| format!("Unknown role: {role}"))?,
        goals: serde_json::from_value(goals).map_err(|error| error.to_string())?,
        offers: serde_json::from_value(offers).map_err(|error| error.to_string())?,
        bio: row.get("bio"),
        created_at: row.get("created_at"),
    })
}

fn link_from_row(row: &Row) -> CoachClientLink {
    CoachClientLink {
        coach_id: row.get("coach_id"),
        client_id: row.get("client_id"),
        created_at: row.get("created_at"),
    }
}

