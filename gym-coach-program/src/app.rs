use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::time::{sleep, Duration as TokioDuration};
use tokio_postgres::{types::Json as PgJson, Client, NoTls, Row};
use uuid::Uuid;

const PROGRAM_PORT: u16 = 8084;
const DEFAULT_DATABASE_URL: &str = "postgres://gymcoach:gymcoach@127.0.0.1:5432/gymcoach";
const DEMO_PROGRAM_ID: &str = "44444444-4444-4444-4444-444444444444";

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: String,
    service: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MediaAsset {
    title: String,
    media_type: String,
    url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProgramExercise {
    name: String,
    sets: u32,
    reps: String,
    #[serde(default)]
    media: Vec<MediaAsset>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProgramDay {
    day: u32,
    title: String,
    exercises: Vec<ProgramExercise>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProgramWeek {
    week: u32,
    days: Vec<ProgramDay>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TrainingProgram {
    id: Uuid,
    title: String,
    level: String,
    goal: String,
    weeks: Vec<ProgramWeek>,
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
        .route("/programs", get(list_programs))
        .route("/programs/:id", get(get_program))
        .with_state(state);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{PROGRAM_PORT}").parse().unwrap();
    println!("Program service listening on http://{addr}");

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
                        eprintln!("Program DB connection error: {error}");
                    }
                });
                return Ok(client);
            }
            Err(error) if attempt < 20 => {
                eprintln!("Program DB connect attempt {attempt} failed: {error}");
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
        CREATE TABLE IF NOT EXISTS training_programs (
            id UUID PRIMARY KEY,
            title TEXT NOT NULL,
            level TEXT NOT NULL,
            goal TEXT NOT NULL,
            weeks JSONB NOT NULL
        );
        ",
    )
    .await?;

    db.execute(
        "
        DELETE FROM training_programs
        WHERE title = '4-Week Beginner Strength'
          AND id <> $1
        ",
        &[&Uuid::parse_str(DEMO_PROGRAM_ID).unwrap()],
    )
    .await?;

    for program in seed_programs() {
        db.execute(
            "
            INSERT INTO training_programs (id, title, level, goal, weeks)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE
            SET title = EXCLUDED.title,
                level = EXCLUDED.level,
                goal = EXCLUDED.goal,
                weeks = EXCLUDED.weeks
            ",
            &[
                &program.id,
                &program.title,
                &program.level,
                &program.goal,
                &PgJson(&program.weeks),
            ],
        )
        .await?;
    }

    Ok(())
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "ProgramService".into(),
    })
}

async fn list_programs(State(state): State<AppState>) -> Result<Json<Vec<TrainingProgram>>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rows = db
        .query(
            "
            SELECT id, title, level, goal, weeks
            FROM training_programs
            ORDER BY title
            ",
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let programs = rows
        .iter()
        .map(program_from_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(programs))
}

async fn get_program(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingProgram>, StatusCode> {
    let db = db_client(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = db
        .query_opt(
            "
            SELECT id, title, level, goal, weeks
            FROM training_programs
            WHERE id = $1
            ",
            &[&id],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    program_from_row(&row)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn seed_programs() -> Vec<TrainingProgram> {
    vec![TrainingProgram {
        id: Uuid::parse_str(DEMO_PROGRAM_ID).unwrap(),
        title: "4-Week Beginner Strength".into(),
        level: "Beginner".into(),
        goal: "Build basic strength and exercise consistency".into(),
        weeks: vec![
            ProgramWeek {
                week: 1,
                days: vec![
                    ProgramDay {
                        day: 1,
                        title: "Full Body A".into(),
                        exercises: vec![
                            ProgramExercise {
                                name: "Goblet Squat".into(),
                                sets: 3,
                                reps: "10".into(),
                                media: vec![MediaAsset {
                                    title: "Goblet Squat Demo".into(),
                                    media_type: "video".into(),
                                    url: "https://www.w3schools.com/html/mov_bbb.mp4".into(),
                                }],
                            },
                            ProgramExercise {
                                name: "Push Up".into(),
                                sets: 3,
                                reps: "8-12".into(),
                                media: vec![MediaAsset {
                                    title: "Push Up Demo".into(),
                                    media_type: "video".into(),
                                    url: "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4".into(),
                                }],
                            },
                        ],
                    },
                    ProgramDay {
                        day: 2,
                        title: "Full Body B".into(),
                        exercises: vec![ProgramExercise {
                            name: "Romanian Deadlift".into(),
                            sets: 3,
                            reps: "10".into(),
                            media: vec![MediaAsset {
                                title: "RDL Demo".into(),
                                media_type: "video".into(),
                                url: "https://www.w3schools.com/html/movie.mp4".into(),
                            }],
                        }],
                    },
                ],
            },
            ProgramWeek {
                week: 2,
                days: vec![ProgramDay {
                    day: 1,
                    title: "Progression Day".into(),
                    exercises: vec![ProgramExercise {
                        name: "Bench Press".into(),
                        sets: 4,
                        reps: "6-8".into(),
                        media: vec![MediaAsset {
                            title: "Bench Press Demo".into(),
                            media_type: "video".into(),
                            url: "https://www.w3schools.com/html/mov_bbb.mp4".into(),
                        }],
                    }],
                }],
            },
        ],
    }]
}

fn program_from_row(row: &Row) -> Result<TrainingProgram, String> {
    let weeks: Value = row.get("weeks");

    Ok(TrainingProgram {
        id: row.get("id"),
        title: row.get("title"),
        level: row.get("level"),
        goal: row.get("goal"),
        weeks: serde_json::from_value(weeks).map_err(|error| error.to_string())?,
    })
}

