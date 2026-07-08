use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::time::{sleep, Duration as TokioDuration};
use tokio_postgres::{types::Json as PgJson, Client, NoTls, Row};
use uuid::Uuid;

const TRAINING_PORT: u16 = 8083;
const DEFAULT_DATABASE_URL: &str = "postgres://gymcoach:gymcoach@127.0.0.1:5432/gymcoach";
const DEMO_COACH_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEMO_CLIENT_ID: &str = "22222222-2222-2222-2222-222222222222";
const DEMO_TRAINING_ONE_ID: &str = "33333333-3333-3333-3333-333333333331";
const DEMO_TRAINING_TWO_ID: &str = "33333333-3333-3333-3333-333333333332";

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

impl TrainingStatus {
    fn as_db_value(&self) -> &'static str {
        match self {
            Self::Planned => "PLANNED",
            Self::Completed => "COMPLETED",
            Self::Skipped => "SKIPPED",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "PLANNED" => Some(Self::Planned),
            "COMPLETED" => Some(Self::Completed),
            "SKIPPED" => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TrainingSet {
    #[serde(default)]
    reps: Option<u32>,
    #[serde(default)]
    load_kg: Option<f32>,
    #[serde(default)]
    duration_min: Option<f32>,
    #[serde(default)]
    distance_km: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum TrackingMode {
    #[default]
    LoadReps,
    RepsOnly,
    Duration,
    DistanceDuration,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MediaAsset {
    title: String,
    media_type: String,
    url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Exercise {
    name: String,
    exercise_type: String,
    #[serde(default)]
    tracking_mode: TrackingMode,
    performed_on: NaiveDate,
    sets: Vec<TrainingSet>,
    #[serde(default)]
    media: Vec<MediaAsset>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExerciseGroup {
    name: String,
    exercises: Vec<Exercise>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TrainingSession {
    id: Uuid,
    coach_id: Option<Uuid>,
    client_id: Uuid,
    category: String,
    status: TrainingStatus,
    notes: String,
    exercise_groups: Vec<ExerciseGroup>,
}

#[derive(Debug, Deserialize)]
struct CreateTrainingRequest {
    coach_id: Option<Uuid>,
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
    database_url: Arc<String>,
}

#[tokio::main]
async fn main() {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let db = connect_db(&database_url).await.unwrap();
    init_db(&db).await.unwrap();
    let state = AppState {
        database_url: Arc::new(database_url),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/trainings", get(list_trainings).post(create_training))
        .route(
            "/trainings/:id",
            get(get_training).put(update_training).delete(delete_training),
        )
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

async fn connect_db(database_url: &str) -> Result<Client, tokio_postgres::Error> {
    for attempt in 1..=20 {
        match tokio_postgres::connect(database_url, NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(error) = connection.await {
                        eprintln!("Training DB connection error: {error}");
                    }
                });
                return Ok(client);
            }
            Err(error) if attempt < 20 => {
                eprintln!("Training DB connect attempt {attempt} failed: {error}");
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
        CREATE TABLE IF NOT EXISTS training_sessions (
            id UUID PRIMARY KEY,
            coach_id UUID,
            client_id UUID NOT NULL,
            category TEXT NOT NULL,
            status TEXT NOT NULL,
            notes TEXT NOT NULL,
            exercise_groups JSONB NOT NULL
        );
        ",
    )
    .await?;

    db.batch_execute(
        "
        ALTER TABLE training_sessions
        ALTER COLUMN coach_id DROP NOT NULL;
        ",
    )
    .await?;

    db.execute(
        "
        DELETE FROM training_sessions
        WHERE category = 'Upper Body Strength'
          AND notes IN (
            'Strong bench session with stable tempo.',
            'Slight increase in load, reps still controlled.'
          )
          AND id NOT IN ($1, $2)
        ",
        &[
            &Uuid::parse_str(DEMO_TRAINING_ONE_ID).unwrap(),
            &Uuid::parse_str(DEMO_TRAINING_TWO_ID).unwrap(),
        ],
    )
    .await?;

    for training in seed_trainings() {
        db.execute(
            "
            INSERT INTO training_sessions (id, coach_id, client_id, category, status, notes, exercise_groups)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE
            SET coach_id = EXCLUDED.coach_id,
                client_id = EXCLUDED.client_id,
                category = EXCLUDED.category,
                status = EXCLUDED.status,
                notes = EXCLUDED.notes,
                exercise_groups = EXCLUDED.exercise_groups
            ",
            &[
                &training.id,
                &training.coach_id,
                &training.client_id,
                &training.category,
                &training.status.as_db_value(),
                &training.notes,
                &PgJson(&training.exercise_groups),
            ],
        )
        .await?;
    }

    Ok(())
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "TrainingService".into(),
    })
}

async fn list_trainings(State(state): State<AppState>) -> Result<Json<Vec<TrainingSession>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, coach_id, client_id, category, status, notes, exercise_groups
            FROM training_sessions
            ORDER BY category, id
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let trainings = rows
        .iter()
        .map(training_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(trainings))
}

async fn get_training(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingSession>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = db
        .query_opt(
            "
            SELECT id, coach_id, client_id, category, status, notes, exercise_groups
            FROM training_sessions
            WHERE id = $1
            ",
            &[&id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    training_from_row(&row)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn list_trainings_for_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<Vec<TrainingSession>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, coach_id, client_id, category, status, notes, exercise_groups
            FROM training_sessions
            WHERE client_id = $1
            ORDER BY id
            ",
            &[&client_id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let trainings = rows
        .iter()
        .map(training_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(trainings))
}

async fn create_training(
    State(state): State<AppState>,
    Json(payload): Json<CreateTrainingRequest>,
) -> Result<(StatusCode, Json<TrainingSession>), StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let training = normalize_training_payload(&db, payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    db
        .execute(
            "
            INSERT INTO training_sessions (id, coach_id, client_id, category, status, notes, exercise_groups)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ",
            &[
                &training.id,
                &training.coach_id,
                &training.client_id,
                &training.category,
                &training.status.as_db_value(),
                &training.notes,
                &PgJson(&training.exercise_groups),
            ],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(training)))
}

async fn update_training(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateTrainingRequest>,
) -> Result<Json<TrainingSession>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let updated_training = normalize_training_payload(&db, payload)
        .await
        .map(|training| TrainingSession { id, ..training })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let affected = db
        .execute(
            "
            UPDATE training_sessions
            SET coach_id = $2,
                client_id = $3,
                category = $4,
                status = $5,
                notes = $6,
                exercise_groups = $7
            WHERE id = $1
            ",
            &[
                &updated_training.id,
                &updated_training.coach_id,
                &updated_training.client_id,
                &updated_training.category,
                &updated_training.status.as_db_value(),
                &updated_training.notes,
                &PgJson(&updated_training.exercise_groups),
            ],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(updated_training))
}

async fn normalize_training_payload(
    db: &Client,
    payload: CreateTrainingRequest,
) -> Result<TrainingSession, tokio_postgres::Error> {
    let catalog = load_canonical_catalog(db).await?;
    let category = canonicalize_text(&payload.category, &catalog.categories);
    let notes = payload.notes.trim().to_string();
    let exercise_groups = payload
        .exercise_groups
        .into_iter()
        .map(|group| ExerciseGroup {
            name: group.name.trim().to_string(),
            exercises: group
                .exercises
                .into_iter()
                .map(|exercise| Exercise {
                    name: canonicalize_text(&exercise.name, &catalog.exercise_names),
                    exercise_type: canonicalize_text(
                        &exercise.exercise_type,
                        &catalog.exercise_types,
                    ),
                    tracking_mode: exercise.tracking_mode,
                    performed_on: exercise.performed_on,
                    sets: exercise.sets,
                    media: exercise.media,
                })
                .collect(),
        })
        .collect();

    Ok(TrainingSession {
        id: Uuid::new_v4(),
        coach_id: payload.coach_id,
        client_id: payload.client_id,
        category,
        status: payload.status,
        notes,
        exercise_groups,
    })
}

async fn delete_training(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let affected = db
        .execute("DELETE FROM training_sessions WHERE id = $1", &[&id])
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_catalog(State(state): State<AppState>) -> Result<Json<TrainingCatalog>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, coach_id, client_id, category, status, notes, exercise_groups
            FROM training_sessions
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let trainings = rows
        .iter()
        .map(training_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let categories = unique_case_insensitive(
        trainings
            .iter()
            .map(|session| session.category.clone())
            .collect(),
    );

    let exercise_types = unique_case_insensitive(
        trainings
        .iter()
        .flat_map(|session| session.exercise_groups.iter())
        .flat_map(|group| group.exercises.iter())
        .map(|exercise| exercise.exercise_type.clone())
        .collect(),
    );

    Ok(Json(TrainingCatalog {
        categories,
        exercise_types,
    }))
}

fn seed_trainings() -> Vec<TrainingSession> {
    let coach_id = Uuid::parse_str(DEMO_COACH_ID).unwrap();
    let client_id = Uuid::parse_str(DEMO_CLIENT_ID).unwrap();

    vec![
        TrainingSession {
            id: Uuid::parse_str(DEMO_TRAINING_ONE_ID).unwrap(),
            coach_id: Some(coach_id),
            client_id,
            category: "Upper Body Strength".into(),
            status: TrainingStatus::Completed,
            notes: "Strong bench session with stable tempo.".into(),
            exercise_groups: vec![ExerciseGroup {
                name: "Push".into(),
                exercises: vec![Exercise {
                    name: "Bench Press".into(),
                    exercise_type: "compound".into(),
                    tracking_mode: TrackingMode::LoadReps,
                    performed_on: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
                    sets: vec![
                        TrainingSet { reps: Some(8), load_kg: Some(60.0), duration_min: None, distance_km: None },
                        TrainingSet { reps: Some(8), load_kg: Some(62.5), duration_min: None, distance_km: None },
                        TrainingSet { reps: Some(6), load_kg: Some(65.0), duration_min: None, distance_km: None },
                    ],
                    media: vec![MediaAsset {
                        title: "Bench Press Demo".into(),
                        media_type: "video".into(),
                        url: "https://www.w3schools.com/html/mov_bbb.mp4".into(),
                    }],
                }],
            }],
        },
        TrainingSession {
            id: Uuid::parse_str(DEMO_TRAINING_TWO_ID).unwrap(),
            coach_id: Some(coach_id),
            client_id,
            category: "Upper Body Strength".into(),
            status: TrainingStatus::Completed,
            notes: "Slight increase in load, reps still controlled.".into(),
            exercise_groups: vec![ExerciseGroup {
                name: "Push".into(),
                exercises: vec![Exercise {
                    name: "Bench Press".into(),
                    exercise_type: "compound".into(),
                    tracking_mode: TrackingMode::LoadReps,
                    performed_on: NaiveDate::from_ymd_opt(2026, 4, 24).unwrap(),
                    sets: vec![
                        TrainingSet { reps: Some(8), load_kg: Some(62.5), duration_min: None, distance_km: None },
                        TrainingSet { reps: Some(8), load_kg: Some(65.0), duration_min: None, distance_km: None },
                        TrainingSet { reps: Some(6), load_kg: Some(67.5), duration_min: None, distance_km: None },
                    ],
                    media: vec![MediaAsset {
                        title: "Bench Press Form Check".into(),
                        media_type: "video".into(),
                        url: "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4".into(),
                    }],
                }],
            }],
        },
    ]
}

fn training_from_row(row: &Row) -> Result<TrainingSession, String> {
    let status: String = row.get("status");
    let exercise_groups: Value = row.get("exercise_groups");

    Ok(TrainingSession {
        id: row.get("id"),
        coach_id: row.get("coach_id"),
        client_id: row.get("client_id"),
        category: row.get("category"),
        status: TrainingStatus::from_db_value(&status)
            .ok_or_else(|| format!("Unknown training status: {status}"))?,
        notes: row.get("notes"),
        exercise_groups: serde_json::from_value(exercise_groups)
            .map_err(|error| error.to_string())?,
    })
}

struct CanonicalCatalog {
    categories: HashMap<String, String>,
    exercise_types: HashMap<String, String>,
    exercise_names: HashMap<String, String>,
}

async fn load_canonical_catalog(db: &Client) -> Result<CanonicalCatalog, tokio_postgres::Error> {
    let rows = db
        .query(
            "
            SELECT category, exercise_groups
            FROM training_sessions
            ",
            &[],
        )
        .await?;

    let mut categories = HashMap::new();
    let mut exercise_types = HashMap::new();
    let mut exercise_names = HashMap::new();

    for row in rows {
        let category: String = row.get("category");
        register_canonical_value(&mut categories, &category);

        let exercise_groups: Value = row.get("exercise_groups");
        let groups: Vec<ExerciseGroup> = serde_json::from_value(exercise_groups).unwrap_or_default();

        for group in groups {
            for exercise in group.exercises {
                register_canonical_value(&mut exercise_types, &exercise.exercise_type);
                register_canonical_value(&mut exercise_names, &exercise.name);
            }
        }
    }

    Ok(CanonicalCatalog {
        categories,
        exercise_types,
        exercise_names,
    })
}

fn register_canonical_value(values: &mut HashMap<String, String>, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    values
        .entry(normalize_key(trimmed))
        .or_insert_with(|| trimmed.to_string());
}

fn canonicalize_text(raw: &str, existing: &HashMap<String, String>) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    existing
        .get(&normalize_key(trimmed))
        .cloned()
        .unwrap_or_else(|| trimmed.to_string())
}

fn unique_case_insensitive(values: Vec<String>) -> Vec<String> {
    let mut canonical = HashMap::new();

    for value in values {
        register_canonical_value(&mut canonical, &value);
    }

    let mut deduped: Vec<String> = canonical.into_values().collect();
    deduped.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    deduped
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}
