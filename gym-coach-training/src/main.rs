use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

const TRAINING_PORT: u16 = 8083;
const DEMO_COACH_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEMO_CLIENT_ID: &str = "22222222-2222-2222-2222-222222222222";

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: String,
    service: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum TrainingStatus {
    Planned,
    Completed,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TrainingSet {
    reps: u32,
    load_kg: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Exercise {
    name: String,
    exercise_type: String,
    performed_on: NaiveDate,
    sets: Vec<TrainingSet>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExerciseGroup {
    name: String,
    exercises: Vec<Exercise>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TrainingSession {
    id: Uuid,
    coach_id: Uuid,
    client_id: Uuid,
    category: String,
    status: TrainingStatus,
    notes: String,
    exercise_groups: Vec<ExerciseGroup>,
}

#[derive(Debug, Deserialize)]
struct CreateTrainingRequest {
    coach_id: Uuid,
    client_id: Uuid,
    category: String,
    status: TrainingStatus,
    notes: String,
    exercise_groups: Vec<ExerciseGroup>,
}

#[derive(Debug, Serialize)]
struct TrainingCatalog {
    categories: Vec<String>,
    exercise_types: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    trainings: Arc<Mutex<Vec<TrainingSession>>>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        trainings: Arc::new(Mutex::new(seed_trainings())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/trainings", get(list_trainings).post(create_training))
        .route("/trainings/:id", get(get_training))
        .route("/trainings/client/:client_id", get(list_trainings_for_client))
        .route("/trainings/catalog", get(get_catalog))
        .with_state(state);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{TRAINING_PORT}").parse().unwrap();
    println!("Training service listening on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "TrainingService".into(),
    })
}

async fn list_trainings(State(state): State<AppState>) -> Json<Vec<TrainingSession>> {
    let trainings = state.trainings.lock().await;
    Json(trainings.clone())
}

async fn get_training(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingSession>, StatusCode> {
    let trainings = state.trainings.lock().await;
    trainings
        .iter()
        .find(|session| session.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn list_trainings_for_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Json<Vec<TrainingSession>> {
    let trainings = state.trainings.lock().await;
    Json(
        trainings
            .iter()
            .filter(|session| session.client_id == client_id)
            .cloned()
            .collect(),
    )
}

async fn create_training(
    State(state): State<AppState>,
    Json(payload): Json<CreateTrainingRequest>,
) -> (StatusCode, Json<TrainingSession>) {
    let training = TrainingSession {
        id: Uuid::new_v4(),
        coach_id: payload.coach_id,
        client_id: payload.client_id,
        category: payload.category,
        status: payload.status,
        notes: payload.notes,
        exercise_groups: payload.exercise_groups,
    };

    let mut trainings = state.trainings.lock().await;
    trainings.push(training.clone());

    (StatusCode::CREATED, Json(training))
}

async fn get_catalog(State(state): State<AppState>) -> Json<TrainingCatalog> {
    let trainings = state.trainings.lock().await;
    let mut categories: Vec<String> = trainings.iter().map(|session| session.category.clone()).collect();
    categories.sort();
    categories.dedup();

    let mut exercise_types: Vec<String> = trainings
        .iter()
        .flat_map(|session| session.exercise_groups.iter())
        .flat_map(|group| group.exercises.iter())
        .map(|exercise| exercise.exercise_type.clone())
        .collect();
    exercise_types.sort();
    exercise_types.dedup();

    Json(TrainingCatalog {
        categories,
        exercise_types,
    })
}

fn seed_trainings() -> Vec<TrainingSession> {
    let coach_id = Uuid::parse_str(DEMO_COACH_ID).unwrap();
    let client_id = Uuid::parse_str(DEMO_CLIENT_ID).unwrap();

    vec![
        TrainingSession {
            id: Uuid::new_v4(),
            coach_id,
            client_id,
            category: "Upper Body Strength".into(),
            status: TrainingStatus::Completed,
            notes: "Strong bench session with stable tempo.".into(),
            exercise_groups: vec![ExerciseGroup {
                name: "Push".into(),
                exercises: vec![Exercise {
                    name: "Bench Press".into(),
                    exercise_type: "compound".into(),
                    performed_on: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
                    sets: vec![
                        TrainingSet { reps: 8, load_kg: 60.0 },
                        TrainingSet { reps: 8, load_kg: 62.5 },
                        TrainingSet { reps: 6, load_kg: 65.0 },
                    ],
                }],
            }],
        },
        TrainingSession {
            id: Uuid::new_v4(),
            coach_id,
            client_id,
            category: "Upper Body Strength".into(),
            status: TrainingStatus::Completed,
            notes: "Slight increase in load, reps still controlled.".into(),
            exercise_groups: vec![ExerciseGroup {
                name: "Push".into(),
                exercises: vec![Exercise {
                    name: "Bench Press".into(),
                    exercise_type: "compound".into(),
                    performed_on: NaiveDate::from_ymd_opt(2026, 4, 24).unwrap(),
                    sets: vec![
                        TrainingSet { reps: 8, load_kg: 62.5 },
                        TrainingSet { reps: 8, load_kg: 65.0 },
                        TrainingSet { reps: 6, load_kg: 67.5 },
                    ],
                }],
            }],
        },
    ]
}
