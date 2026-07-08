use axum::{
    routing::{get, post},
    Json, Router,
};
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

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum ProgressionPreference {
    Stagnation,
    #[default]
    ProgressiveOverload,
}

impl ProgressionPreference {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stagnation => "stagnation",
            Self::ProgressiveOverload => "progressive_overload",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum GoalFocus {
    Strength,
    Hypertrophy,
    Endurance,
    General,
}

impl GoalFocus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Strength => "strength",
            Self::Hypertrophy => "hypertrophy",
            Self::Endurance => "endurance",
            Self::General => "general",
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnalyticsRequest {
    client_id: String,
    exercise_name: String,
    #[serde(default)]
    client_goals: Vec<String>,
    #[serde(default)]
    progression_preference: ProgressionPreference,
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
    avg_sets_per_session: f32,
    avg_volume_per_session: f32,
    typical_sets: u32,
    typical_reps: u32,
    typical_load_kg: f32,
    goal_focus: String,
    progression_preference: String,
    trend: String,
}

#[derive(Debug, Serialize)]
struct RecommendationResponse {
    exercise_name: String,
    recommended_sets: u32,
    recommended_load_kg: f32,
    recommended_reps: u32,
    goal_focus: String,
    progression_preference: String,
    rationale: String,
}

#[derive(Debug, Clone)]
struct SessionSummary {
    set_count: u32,
    avg_reps: f32,
    equivalent_load: f32,
    top_load: f32,
    total_volume: f32,
}

#[derive(Debug, Clone)]
struct ExerciseAnalysis {
    goal_focus: GoalFocus,
    sessions: Vec<SessionSummary>,
    avg_load: f32,
    avg_reps: f32,
    best_load: f32,
    total_volume: f32,
    avg_sets_per_session: f32,
    avg_volume_per_session: f32,
    typical_sets: u32,
    typical_reps: u32,
    typical_load: f32,
    trend: String,
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
    let analysis = analyze_history(&payload);
    Json(build_report(&payload, &analysis))
}

async fn recommendation(Json(payload): Json<AnalyticsRequest>) -> Json<RecommendationResponse> {
    let analysis = analyze_history(&payload);
    let analytics = build_report(&payload, &analysis);
    Json(build_recommendation(&payload, &analysis, &analytics))
}

fn analyze_history(payload: &AnalyticsRequest) -> ExerciseAnalysis {
    let goal_focus = infer_goal_focus(&payload.client_goals);
    let mut total_sets = 0usize;
    let mut total_load = 0.0f32;
    let mut total_reps = 0u32;
    let mut total_volume = 0.0f32;
    let mut best_load = 0.0f32;
    let mut sessions = payload.history.clone();
    sessions.sort_by_key(|session| session.performed_on);
    let mut session_summaries = Vec::new();

    for session in &sessions {
        let mut session_set_count = 0u32;
        let mut session_total_reps = 0u32;
        let mut session_total_volume = 0.0f32;
        let mut session_top_load = 0.0f32;

        for set in &session.sets {
            total_sets += 1;
            total_load += set.load_kg;
            total_reps += set.reps;
            total_volume += set.load_kg * set.reps as f32;
            best_load = best_load.max(set.load_kg);

            session_set_count += 1;
            session_total_reps += set.reps;
            session_total_volume += set.load_kg * set.reps as f32;
            session_top_load = session_top_load.max(set.load_kg);
        }

        if session_set_count > 0 && session_total_reps > 0 {
            session_summaries.push(SessionSummary {
                set_count: session_set_count,
                avg_reps: session_total_reps as f32 / session_set_count as f32,
                equivalent_load: session_total_volume / session_total_reps as f32,
                top_load: session_top_load,
                total_volume: session_total_volume,
            });
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

    let trend = if session_summaries.len() >= 2
        && session_summaries.last().unwrap().equivalent_load
            > session_summaries[0].equivalent_load + 1.0
    {
        "upward"
    } else if session_summaries.len() >= 2
        && session_summaries.last().unwrap().equivalent_load + 1.0
            < session_summaries[0].equivalent_load
    {
        "downward"
    } else {
        "stable"
    };

    let weighted_sets = weighted_average(
        &session_summaries
            .iter()
            .enumerate()
            .map(|(index, session)| ((index + 1) as f32, session.set_count as f32))
            .collect::<Vec<_>>(),
    );
    let weighted_reps = weighted_average(
        &session_summaries
            .iter()
            .enumerate()
            .map(|(index, session)| ((index + 1) as f32, session.avg_reps))
            .collect::<Vec<_>>(),
    );
    let weighted_load = weighted_average(
        &session_summaries
            .iter()
            .enumerate()
            .map(|(index, session)| ((index + 1) as f32, session.equivalent_load))
            .collect::<Vec<_>>(),
    );
    let avg_sets_per_session = if session_summaries.is_empty() {
        0.0
    } else {
        session_summaries
            .iter()
            .map(|session| session.set_count as f32)
            .sum::<f32>()
            / session_summaries.len() as f32
    };
    let avg_volume_per_session = if session_summaries.is_empty() {
        0.0
    } else {
        session_summaries
            .iter()
            .map(|session| session.total_volume)
            .sum::<f32>()
            / session_summaries.len() as f32
    };

    ExerciseAnalysis {
        goal_focus,
        sessions: session_summaries,
        avg_load,
        avg_reps,
        best_load,
        total_volume,
        avg_sets_per_session,
        avg_volume_per_session,
        typical_sets: weighted_sets.round().max(1.0) as u32,
        typical_reps: weighted_reps.round().max(1.0) as u32,
        typical_load: weighted_load,
        trend: trend.into(),
    }
}

fn build_report(payload: &AnalyticsRequest, analysis: &ExerciseAnalysis) -> AnalyticsReport {
    AnalyticsReport {
        client_id: payload.client_id.clone(),
        exercise_name: payload.exercise_name.clone(),
        sessions_analyzed: payload.history.len(),
        avg_load_kg: round_to_half(analysis.avg_load),
        avg_reps: (analysis.avg_reps * 10.0).round() / 10.0,
        best_load_kg: round_to_half(analysis.best_load),
        total_volume: (analysis.total_volume * 10.0).round() / 10.0,
        avg_sets_per_session: (analysis.avg_sets_per_session * 10.0).round() / 10.0,
        avg_volume_per_session: (analysis.avg_volume_per_session * 10.0).round() / 10.0,
        typical_sets: analysis.typical_sets,
        typical_reps: analysis.typical_reps,
        typical_load_kg: round_to_half(analysis.typical_load),
        goal_focus: analysis.goal_focus.as_str().into(),
        progression_preference: payload.progression_preference.as_str().into(),
        trend: analysis.trend.clone(),
    }
}

fn round_to_half(value: f32) -> f32 {
    (value * 2.0).round() / 2.0
}

fn build_recommendation(
    payload: &AnalyticsRequest,
    analysis: &ExerciseAnalysis,
    analytics: &AnalyticsReport,
) -> RecommendationResponse {
    let settings = goal_settings(analysis.goal_focus);
    let last_session = analysis.sessions.last();
    let baseline_load = if let Some(last) = last_session {
        round_to_half((analysis.typical_load * 0.65) + (last.equivalent_load * 0.35))
    } else {
        round_to_half(analysis.typical_load)
    };
    let latest_top_load = last_session
        .map(|session| session.top_load)
        .unwrap_or(baseline_load);
    let mut recommended_sets = analysis.typical_sets.max(1);
    let mut recommended_reps = analysis.typical_reps.max(1);
    let mut recommended_load = baseline_load.max(0.0);

    if has_consistent_pattern(&analysis.sessions) {
        recommended_reps = recommended_reps
            .max(settings.min_reps)
            .min(settings.max_reps);
    } else {
        recommended_reps = clamp_u32(recommended_reps, settings.min_reps, settings.max_reps);
    }

    match payload.progression_preference {
        ProgressionPreference::Stagnation => {
            if let Some(last) = last_session {
                recommended_load = round_to_half((baseline_load + last.equivalent_load) / 2.0);
                if last.avg_reps + 0.5 < settings.min_reps as f32 {
                    recommended_load =
                        round_to_half((recommended_load - settings.load_increment).max(0.0));
                    recommended_reps = settings.min_reps;
                }
            }
        }
        ProgressionPreference::ProgressiveOverload => {
            if let Some(last) = last_session {
                if analytics.trend == "upward" && last.avg_reps + 0.5 >= recommended_reps as f32 {
                    recommended_load = round_to_half(
                        recommended_load.max(latest_top_load) + settings.load_increment,
                    );
                    recommended_reps = recommended_reps.saturating_sub(1).max(settings.min_reps);
                } else if analytics.trend == "stable"
                    && last.avg_reps + 0.5 >= recommended_reps as f32
                {
                    recommended_load = round_to_half(recommended_load + settings.load_increment);
                } else if last.avg_reps + 0.5 < settings.min_reps as f32 {
                    recommended_load =
                        round_to_half((recommended_load - settings.load_increment).max(0.0));
                    recommended_reps = settings.min_reps;
                } else if recommended_reps < settings.max_reps {
                    recommended_reps += 1;
                }
            }
        }
    }

    if matches!(analysis.goal_focus, GoalFocus::Hypertrophy) && analytics.trend != "downward" {
        recommended_sets = recommended_sets.max(3);
    }

    RecommendationResponse {
        exercise_name: payload.exercise_name.clone(),
        recommended_sets,
        recommended_load_kg: round_to_half(recommended_load),
        recommended_reps,
        goal_focus: analysis.goal_focus.as_str().into(),
        progression_preference: payload.progression_preference.as_str().into(),
        rationale: format!(
            "Based on {} sessions, your usual pattern is around {} sets of {} reps at {:.1}kg. Goal focus is {} and strategy is {}, so the next session is balanced around volume {:.0}kg-reps with a {} trend.",
            analytics.sessions_analyzed,
            analytics.typical_sets,
            analytics.typical_reps,
            analytics.typical_load_kg,
            analytics.goal_focus,
            analytics.progression_preference,
            analytics.avg_volume_per_session,
            analytics.trend
        ),
    }
}

struct GoalSettings {
    min_reps: u32,
    max_reps: u32,
    load_increment: f32,
}

fn goal_settings(goal_focus: GoalFocus) -> GoalSettings {
    match goal_focus {
        GoalFocus::Strength => GoalSettings {
            min_reps: 5,
            max_reps: 10,
            load_increment: 2.5,
        },
        GoalFocus::Hypertrophy => GoalSettings {
            min_reps: 8,
            max_reps: 12,
            load_increment: 2.5,
        },
        GoalFocus::Endurance => GoalSettings {
            min_reps: 10,
            max_reps: 15,
            load_increment: 1.25,
        },
        GoalFocus::General => GoalSettings {
            min_reps: 6,
            max_reps: 12,
            load_increment: 2.5,
        },
    }
}

fn infer_goal_focus(goals: &[String]) -> GoalFocus {
    for goal in goals {
        let normalized = goal.trim().to_ascii_lowercase();
        if normalized.contains("strength") || normalized.contains("snaga") {
            return GoalFocus::Strength;
        }
        if normalized.contains("hypertrophy")
            || normalized.contains("muscle")
            || normalized.contains("mass")
            || normalized.contains("masa")
        {
            return GoalFocus::Hypertrophy;
        }
        if normalized.contains("endurance")
            || normalized.contains("fat loss")
            || normalized.contains("conditioning")
            || normalized.contains("izdrz")
            || normalized.contains("mrsav")
        {
            return GoalFocus::Endurance;
        }
    }

    GoalFocus::General
}

fn weighted_average(values: &[(f32, f32)]) -> f32 {
    let total_weight: f32 = values.iter().map(|(weight, _)| *weight).sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    values
        .iter()
        .map(|(weight, value)| weight * value)
        .sum::<f32>()
        / total_weight
}

fn has_consistent_pattern(sessions: &[SessionSummary]) -> bool {
    if sessions.len() < 2 {
        return false;
    }

    let min_load = sessions
        .iter()
        .map(|session| session.equivalent_load)
        .fold(f32::INFINITY, f32::min);
    let max_load = sessions
        .iter()
        .map(|session| session.equivalent_load)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_reps = sessions
        .iter()
        .map(|session| session.avg_reps)
        .fold(f32::INFINITY, f32::min);
    let max_reps = sessions
        .iter()
        .map(|session| session.avg_reps)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_sets = sessions
        .iter()
        .map(|session| session.set_count)
        .min()
        .unwrap_or(0);
    let max_sets = sessions
        .iter()
        .map(|session| session.set_count)
        .max()
        .unwrap_or(0);

    (max_load - min_load) <= 1.0 && (max_reps - min_reps) <= 1.0 && (max_sets - min_sets) <= 1
}

fn clamp_u32(value: u32, min: u32, max: u32) -> u32 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(date: &str, sets: &[(u32, f32)]) -> ExerciseSnapshot {
        ExerciseSnapshot {
            performed_on: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            sets: sets
                .iter()
                .map(|(reps, load_kg)| ExerciseSet {
                    reps: *reps,
                    load_kg: *load_kg,
                })
                .collect(),
        }
    }

    #[test]
    fn stagnation_keeps_consistent_pattern() {
        let payload = AnalyticsRequest {
            client_id: "1".into(),
            exercise_name: "Bench Press".into(),
            client_goals: vec!["strength".into()],
            progression_preference: ProgressionPreference::Stagnation,
            history: vec![
                snapshot("2026-06-01", &[(10, 65.0), (10, 65.0), (10, 65.0)]),
                snapshot("2026-06-08", &[(10, 65.0), (10, 65.0), (10, 65.0)]),
            ],
        };

        let analysis = analyze_history(&payload);
        let report = build_report(&payload, &analysis);
        let recommendation = build_recommendation(&payload, &analysis, &report);

        assert_eq!(recommendation.recommended_sets, 3);
        assert_eq!(recommendation.recommended_reps, 10);
        assert_eq!(recommendation.recommended_load_kg, 65.0);
    }

    #[test]
    fn varied_sessions_find_balanced_middle() {
        let payload = AnalyticsRequest {
            client_id: "1".into(),
            exercise_name: "Bench Press".into(),
            client_goals: vec!["strength".into()],
            progression_preference: ProgressionPreference::Stagnation,
            history: vec![
                snapshot("2026-06-01", &[(10, 62.0), (8, 65.0), (8, 70.0)]),
                snapshot("2026-06-08", &[(10, 62.0), (10, 65.0), (8, 67.5)]),
            ],
        };

        let analysis = analyze_history(&payload);
        let report = build_report(&payload, &analysis);
        let recommendation = build_recommendation(&payload, &analysis, &report);

        assert_eq!(recommendation.recommended_sets, 3);
        assert!(recommendation.recommended_load_kg >= 64.5);
        assert!(recommendation.recommended_load_kg <= 67.5);
        assert!(recommendation.recommended_reps >= 8);
        assert!(recommendation.recommended_reps <= 10);
    }
}
