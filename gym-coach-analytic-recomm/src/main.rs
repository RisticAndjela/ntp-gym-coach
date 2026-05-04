use axum::{routing::{get, post}, Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};

const ANALYTICS_PORT: u16 = 8085;

#[derive(Debug, Serialize)]
struct ServiceStatus {
    status: String,
    service: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExerciseSet {
    reps: u32,
    load_kg: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExerciseSnapshot {
    performed_on: NaiveDate,
    sets: Vec<ExerciseSet>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsRequest {
    client_id: String,
    exercise_name: String,
    history: Vec<ExerciseSnapshot>,
}

#[derive(Debug, Serialize)]
struct AnalyticsReport {
    client_id: String,
    exercise_name: String,
    sessions_analyzed: usize,
    avg_load_kg: f32,
    avg_reps: f32,
    best_load_kg: f32,
    total_volume: f32,
    trend: String,
}

#[derive(Debug, Serialize)]
struct RecommendationResponse {
    exercise_name: String,
    recommended_load_kg: f32,
    recommended_reps: u32,
    rationale: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/analytics/report", post(report))
        .route("/analytics/recommendation", post(recommendation));

    let host = env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let addr: SocketAddr = format!("{host}:{ANALYTICS_PORT}").parse().unwrap();
    println!("Analytics service listening on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "UP".into(),
        service: "AnalyticsRecommendationService".into(),
    })
}

async fn report(Json(payload): Json<AnalyticsRequest>) -> Json<AnalyticsReport> {
    Json(build_report(payload))
}

async fn recommendation(Json(payload): Json<AnalyticsRequest>) -> Json<RecommendationResponse> {
    let analytics = build_report(payload);
    let suggested_load = if analytics.trend == "upward" {
        analytics.best_load_kg + 2.5
    } else {
        analytics.best_load_kg
    };
    let suggested_reps = if analytics.avg_reps >= 8.0 { 8 } else { 6 };

    Json(RecommendationResponse {
        exercise_name: analytics.exercise_name,
        recommended_load_kg: round_to_half(suggested_load),
        recommended_reps: suggested_reps,
        rationale: format!(
            "Trend is {}, average load {:.1}kg across {} sessions.",
            analytics.trend, analytics.avg_load_kg, analytics.sessions_analyzed
        ),
    })
}

fn build_report(payload: AnalyticsRequest) -> AnalyticsReport {
    let mut total_sets = 0usize;
    let mut total_load = 0.0f32;
    let mut total_reps = 0u32;
    let mut total_volume = 0.0f32;
    let mut best_load = 0.0f32;
    let mut session_avg_loads = Vec::new();

    for session in &payload.history {
        let mut session_load_sum = 0.0f32;
        let mut session_set_count = 0usize;

        for set in &session.sets {
            total_sets += 1;
            total_load += set.load_kg;
            total_reps += set.reps;
            total_volume += set.load_kg * set.reps as f32;
            best_load = best_load.max(set.load_kg);

            session_load_sum += set.load_kg;
            session_set_count += 1;
        }

        if session_set_count > 0 {
            session_avg_loads.push(session_load_sum / session_set_count as f32);
        }
    }

    let avg_load = if total_sets > 0 {
        total_load / total_sets as f32
    } else {
        0.0
    };
    let avg_reps = if total_sets > 0 {
        total_reps as f32 / total_sets as f32
    } else {
        0.0
    };

    let trend = if session_avg_loads.len() >= 2
        && session_avg_loads.last().unwrap() > &(session_avg_loads[0] + 1.0)
    {
        "upward"
    } else if session_avg_loads.len() >= 2
        && session_avg_loads.last().unwrap() + 1.0 < session_avg_loads[0]
    {
        "downward"
    } else {
        "stable"
    };

    AnalyticsReport {
        client_id: payload.client_id,
        exercise_name: payload.exercise_name,
        sessions_analyzed: payload.history.len(),
        avg_load_kg: round_to_half(avg_load),
        avg_reps: (avg_reps * 10.0).round() / 10.0,
        best_load_kg: round_to_half(best_load),
        total_volume: (total_volume * 10.0).round() / 10.0,
        trend: trend.into(),
    }
}

fn round_to_half(value: f32) -> f32 {
    (value * 2.0).round() / 2.0
}
