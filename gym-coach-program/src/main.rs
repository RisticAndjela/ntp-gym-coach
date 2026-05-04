use axum::{
    extract::Path,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use uuid::Uuid;

const PROGRAM_PORT: u16 = 8084;

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

#[tokio::main]
async fn main() {
    let programs = Arc::new(seed_programs());

    let app = Router::new()
        .route("/health", get(health))
        .route("/programs", get(list_programs))
        .route("/programs/:id", get(get_program))
        .with_state(programs);

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{PROGRAM_PORT}").parse().unwrap();
    println!("Program service listening on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "ProgramService".into(),
    })
}

async fn list_programs(
    axum::extract::State(programs): axum::extract::State<Arc<Vec<TrainingProgram>>>,
) -> Json<Vec<TrainingProgram>> {
    Json(programs.as_ref().clone())
}

async fn get_program(
    axum::extract::State(programs): axum::extract::State<Arc<Vec<TrainingProgram>>>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrainingProgram>, StatusCode> {
    programs
        .iter()
        .find(|program| program.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn seed_programs() -> Vec<TrainingProgram> {
    vec![TrainingProgram {
        id: Uuid::new_v4(),
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
                                    url: "https://example.com/media/goblet-squat".into(),
                                }],
                            },
                            ProgramExercise {
                                name: "Push Up".into(),
                                sets: 3,
                                reps: "8-12".into(),
                                media: vec![],
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
                                title: "RDL Setup".into(),
                                media_type: "image".into(),
                                url: "https://example.com/media/rdl-setup".into(),
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
                        media: vec![],
                    }],
                }],
            },
        ],
    }]
}
