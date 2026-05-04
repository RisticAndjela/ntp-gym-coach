use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

const USER_PORT: u16 = 8082;
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
    profiles: Arc<Mutex<HashMap<Uuid, UserProfile>>>,
    links: Arc<Mutex<Vec<CoachClientLink>>>,
}

#[tokio::main]
async fn main() {
    let (profiles, links) = seed_data();
    let state = AppState {
        profiles: Arc::new(Mutex::new(profiles)),
        links: Arc::new(Mutex::new(links)),
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

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "UserService".into(),
    })
}

async fn list_profiles(State(state): State<AppState>) -> Json<Vec<UserProfile>> {
    let profiles = state.profiles.lock().await;
    Json(profiles.values().cloned().collect())
}

async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserProfile>, StatusCode> {
    let profiles = state.profiles.lock().await;
    profiles
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserProfile>, StatusCode> {
    let mut profiles = state.profiles.lock().await;
    let profile = profiles.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

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

    Ok(Json(profile.clone()))
}

async fn list_coaches(State(state): State<AppState>) -> Json<Vec<UserProfile>> {
    let profiles = state.profiles.lock().await;
    Json(
        profiles
            .values()
            .filter(|profile| profile.role == UserRole::Coach)
            .cloned()
            .collect(),
    )
}

async fn match_coaches(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<Vec<CoachMatch>>, StatusCode> {
    let profiles = state.profiles.lock().await;
    let client = profiles.get(&client_id).ok_or(StatusCode::NOT_FOUND)?;

    let matches = profiles
        .values()
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
    let profiles = state.profiles.lock().await;
    let Some(coach) = profiles.get(&payload.coach_id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    let Some(client) = profiles.get(&payload.client_id) else {
        return Err(StatusCode::NOT_FOUND);
    };

    if coach.role != UserRole::Coach || client.role != UserRole::Client {
        return Err(StatusCode::BAD_REQUEST);
    }

    drop(profiles);

    let link = CoachClientLink {
        coach_id: payload.coach_id,
        client_id: payload.client_id,
        created_at: Utc::now(),
    };

    let mut links = state.links.lock().await;
    if links
        .iter()
        .any(|existing| existing.coach_id == link.coach_id && existing.client_id == link.client_id)
    {
        return Err(StatusCode::CONFLICT);
    }

    links.push(link.clone());
    Ok((StatusCode::CREATED, Json(link)))
}

async fn get_clients_for_coach(
    State(state): State<AppState>,
    Path(coach_id): Path<Uuid>,
) -> Json<Vec<CoachClientLink>> {
    let links = state.links.lock().await;
    Json(
        links
            .iter()
            .filter(|link| link.coach_id == coach_id)
            .cloned()
            .collect(),
    )
}

async fn get_coach_for_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Json<Vec<CoachClientLink>> {
    let links = state.links.lock().await;
    Json(
        links
            .iter()
            .filter(|link| link.client_id == client_id)
            .cloned()
            .collect(),
    )
}

fn seed_data() -> (HashMap<Uuid, UserProfile>, Vec<CoachClientLink>) {
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

    (
        HashMap::from([(coach.id, coach), (client.id, client)]),
        vec![link],
    )
}
