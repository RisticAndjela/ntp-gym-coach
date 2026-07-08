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

impl TrackingMode {
    fn metric_spec(self) -> MetricSpec {
        match self {
            Self::LoadReps => MetricSpec {
                primary_label: "load".into(),
                primary_unit: "kg".into(),
                secondary_label: Some("target reps".into()),
                secondary_unit: Some("reps".into()),
                total_output_label: "volume".into(),
                primary_increment: 2.5,
                primary_threshold: 1.0,
                secondary_increment: 1.0,
            },
            Self::RepsOnly => MetricSpec {
                primary_label: "reps".into(),
                primary_unit: "reps".into(),
                secondary_label: None,
                secondary_unit: None,
                total_output_label: "total reps".into(),
                primary_increment: 1.0,
                primary_threshold: 1.0,
                secondary_increment: 0.0,
            },
            Self::Duration => MetricSpec {
                primary_label: "duration".into(),
                primary_unit: "min".into(),
                secondary_label: None,
                secondary_unit: None,
                total_output_label: "total minutes".into(),
                primary_increment: 5.0,
                primary_threshold: 2.0,
                secondary_increment: 0.0,
            },
            Self::DistanceDuration => MetricSpec {
                primary_label: "distance".into(),
                primary_unit: "km".into(),
                secondary_label: Some("target duration".into()),
                secondary_unit: Some("min".into()),
                total_output_label: "total distance".into(),
                primary_increment: 0.5,
                primary_threshold: 0.25,
                secondary_increment: 2.0,
            },
        }
    }
}

impl TrackingMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::LoadReps => "load_reps",
            Self::RepsOnly => "reps_only",
            Self::Duration => "duration",
            Self::DistanceDuration => "distance_duration",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExerciseSnapshot {
    performed_on: NaiveDate,
    #[serde(default)]
    tracking_mode: TrackingMode,
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
    #[serde(default)]
    tracking_mode: TrackingMode,
    history: Vec<ExerciseSnapshot>,
}

#[derive(Debug, Serialize)]
struct AnalyticsReport {
    client_id: String,
    exercise_name: String,
    tracking_mode: String,
    sessions_analyzed: usize,
    primary_metric_label: String,
    primary_metric_unit: String,
    secondary_metric_label: Option<String>,
    secondary_metric_unit: Option<String>,
    avg_primary_metric: f32,
    avg_secondary_metric: Option<f32>,
    best_primary_metric: f32,
    total_output: f32,
    total_output_label: String,
    avg_sets_per_session: f32,
    avg_output_per_session: f32,
    typical_sets: u32,
    typical_primary_metric: f32,
    typical_secondary_metric: Option<f32>,
    goal_focus: String,
    progression_preference: String,
    trend: String,
}

#[derive(Debug, Serialize)]
struct RecommendationResponse {
    exercise_name: String,
    tracking_mode: String,
    recommended_sets: u32,
    primary_metric_label: String,
    primary_metric_unit: String,
    secondary_metric_label: Option<String>,
    secondary_metric_unit: Option<String>,
    recommended_primary_metric: f32,
    recommended_secondary_metric: Option<f32>,
    goal_focus: String,
    progression_preference: String,
    rationale: String,
}

#[derive(Debug, Clone)]
struct SessionSummary {
    set_count: u32,
    avg_primary_metric: f32,
    avg_secondary_metric: Option<f32>,
    top_primary_metric: f32,
    total_output: f32,
}

#[derive(Debug, Clone)]
struct ExerciseAnalysis {
    tracking_mode: TrackingMode,
    metric_spec: MetricSpec,
    goal_focus: GoalFocus,
    sessions: Vec<SessionSummary>,
    avg_primary_metric: f32,
    avg_secondary_metric: Option<f32>,
    best_primary_metric: f32,
    total_output: f32,
    avg_sets_per_session: f32,
    avg_output_per_session: f32,
    typical_sets: u32,
    typical_primary_metric: f32,
    typical_secondary_metric: Option<f32>,
    trend: String,
}

#[derive(Debug, Clone)]
struct MetricSpec {
    primary_label: String,
    primary_unit: String,
    secondary_label: Option<String>,
    secondary_unit: Option<String>,
    total_output_label: String,
    primary_increment: f32,
    primary_threshold: f32,
    secondary_increment: f32,
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
    let tracking_mode = payload
        .history
        .first()
        .map(|snapshot| snapshot.tracking_mode)
        .unwrap_or(payload.tracking_mode);
    let metric_spec = tracking_mode.metric_spec();
    let goal_focus = infer_goal_focus(&payload.client_goals);
    let mut total_sets = 0usize;
    let mut total_primary = 0.0f32;
    let mut total_secondary = 0.0f32;
    let mut total_secondary_count = 0usize;
    let mut total_output = 0.0f32;
    let mut best_primary_metric = 0.0f32;
    let mut sessions = payload.history.clone();
    sessions.sort_by_key(|session| session.performed_on);
    let mut session_summaries = Vec::new();

    for session in &sessions {
        let mut session_set_count = 0u32;
        let mut session_total_primary = 0.0f32;
        let mut session_total_secondary = 0.0f32;
        let mut session_secondary_count = 0u32;
        let mut session_total_output = 0.0f32;
        let mut session_top_primary = 0.0f32;

        for set in &session.sets {
            let primary = primary_metric_value(tracking_mode, set);
            let secondary = secondary_metric_value(tracking_mode, set);
            let output = total_output_value(tracking_mode, set);

            total_sets += 1;
            total_primary += primary;
            total_output += output;
            best_primary_metric = best_primary_metric.max(primary);

            session_set_count += 1;
            session_total_primary += primary;
            session_total_output += output;
            session_top_primary = session_top_primary.max(primary);

            if let Some(secondary_value) = secondary {
                total_secondary += secondary_value;
                total_secondary_count += 1;
                session_total_secondary += secondary_value;
                session_secondary_count += 1;
            }
        }

        if session_set_count > 0 {
            session_summaries.push(SessionSummary {
                set_count: session_set_count,
                avg_primary_metric: session_total_primary / session_set_count as f32,
                avg_secondary_metric: if session_secondary_count > 0 {
                    Some(session_total_secondary / session_secondary_count as f32)
                } else {
                    None
                },
                top_primary_metric: session_top_primary,
                total_output: session_total_output,
            });
        }
    }

    let avg_primary_metric = if total_sets > 0 {
        total_primary / total_sets as f32
    } else {
        0.0
    };
    let avg_secondary_metric = if total_secondary_count > 0 {
        Some(total_secondary / total_secondary_count as f32)
    } else {
        None
    };

    let trend = detect_trend(&session_summaries, tracking_mode);

    let weighted_sets = weighted_average(
        &session_summaries
            .iter()
            .enumerate()
            .map(|(index, session)| ((index + 1) as f32, session.set_count as f32))
            .collect::<Vec<_>>(),
    );
    let weighted_primary = weighted_average(
        &session_summaries
            .iter()
            .enumerate()
            .map(|(index, session)| ((index + 1) as f32, session.avg_primary_metric))
            .collect::<Vec<_>>(),
    );
    let weighted_secondary = weighted_optional_average(
        &session_summaries
            .iter()
            .enumerate()
            .filter_map(|(index, session)| {
                session
                    .avg_secondary_metric
                    .map(|value| ((index + 1) as f32, value))
            })
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
    let avg_output_per_session = if session_summaries.is_empty() {
        0.0
    } else {
        session_summaries
            .iter()
            .map(|session| session.total_output)
            .sum::<f32>()
            / session_summaries.len() as f32
    };

    ExerciseAnalysis {
        tracking_mode,
        metric_spec,
        goal_focus,
        sessions: session_summaries,
        avg_primary_metric,
        avg_secondary_metric,
        best_primary_metric,
        total_output,
        avg_sets_per_session,
        avg_output_per_session,
        typical_sets: weighted_sets.round().max(1.0) as u32,
        typical_primary_metric: weighted_primary.max(0.0),
        typical_secondary_metric: weighted_secondary,
        trend,
    }
}

fn build_report(payload: &AnalyticsRequest, analysis: &ExerciseAnalysis) -> AnalyticsReport {
    AnalyticsReport {
        client_id: payload.client_id.clone(),
        exercise_name: payload.exercise_name.clone(),
        tracking_mode: analysis.tracking_mode.as_str().into(),
        sessions_analyzed: payload.history.len(),
        primary_metric_label: analysis.metric_spec.primary_label.clone(),
        primary_metric_unit: analysis.metric_spec.primary_unit.clone(),
        secondary_metric_label: analysis.metric_spec.secondary_label.clone(),
        secondary_metric_unit: analysis.metric_spec.secondary_unit.clone(),
        avg_primary_metric: round_metric(analysis.avg_primary_metric),
        avg_secondary_metric: analysis.avg_secondary_metric.map(round_metric),
        best_primary_metric: round_metric(analysis.best_primary_metric),
        total_output: round_metric(analysis.total_output),
        total_output_label: analysis.metric_spec.total_output_label.clone(),
        avg_sets_per_session: round_metric(analysis.avg_sets_per_session),
        avg_output_per_session: round_metric(analysis.avg_output_per_session),
        typical_sets: analysis.typical_sets,
        typical_primary_metric: round_metric(analysis.typical_primary_metric),
        typical_secondary_metric: analysis.typical_secondary_metric.map(round_metric),
        goal_focus: analysis.goal_focus.as_str().into(),
        progression_preference: payload.progression_preference.as_str().into(),
        trend: analysis.trend.clone(),
    }
}

fn build_recommendation(
    payload: &AnalyticsRequest,
    analysis: &ExerciseAnalysis,
    analytics: &AnalyticsReport,
) -> RecommendationResponse {
    let settings = goal_settings(analysis.goal_focus);
    let last_session = analysis.sessions.last();
    let mut recommended_sets = analysis.typical_sets.max(1);
    let mut recommended_primary = if let Some(last) = last_session {
        round_metric((analysis.typical_primary_metric * 0.65) + (last.avg_primary_metric * 0.35))
    } else {
        round_metric(analysis.typical_primary_metric)
    };
    let mut recommended_secondary = if let Some(last) = last_session {
        analysis
            .typical_secondary_metric
            .map(|typical| round_metric((typical * 0.65) + (last.avg_secondary_metric.unwrap_or(typical) * 0.35)))
    } else {
        analysis.typical_secondary_metric.map(round_metric)
    };

    match analysis.tracking_mode {
        TrackingMode::LoadReps => {
            let latest_top_primary = last_session
                .map(|session| session.top_primary_metric)
                .unwrap_or(recommended_primary);
            let mut reps_target = recommended_secondary.unwrap_or(settings.default_secondary as f32);

            if has_consistent_pattern(&analysis.sessions, analysis.tracking_mode) {
                reps_target = reps_target
                    .max(settings.min_secondary as f32)
                    .min(settings.max_secondary as f32);
            } else {
                reps_target = clamp_f32(
                    reps_target,
                    settings.min_secondary as f32,
                    settings.max_secondary as f32,
                );
            }

            match payload.progression_preference {
                ProgressionPreference::Stagnation => {
                    if let Some(last) = last_session {
                        recommended_primary =
                            round_metric((recommended_primary + last.avg_primary_metric) / 2.0);
                        if last.avg_secondary_metric.unwrap_or(reps_target)
                            + 0.5
                            < settings.min_secondary as f32
                        {
                            recommended_primary = round_metric(
                                (recommended_primary - analysis.metric_spec.primary_increment)
                                    .max(0.0),
                            );
                            reps_target = settings.min_secondary as f32;
                        }
                    }
                }
                ProgressionPreference::ProgressiveOverload => {
                    if let Some(last) = last_session {
                        if analytics.trend == "upward"
                            && last.avg_secondary_metric.unwrap_or(reps_target) + 0.5 >= reps_target
                        {
                            recommended_primary = round_metric(
                                recommended_primary.max(latest_top_primary)
                                    + analysis.metric_spec.primary_increment,
                            );
                            reps_target = (reps_target - 1.0).max(settings.min_secondary as f32);
                        } else if analytics.trend == "stable"
                            && last.avg_secondary_metric.unwrap_or(reps_target) + 0.5 >= reps_target
                        {
                            recommended_primary =
                                round_metric(recommended_primary + analysis.metric_spec.primary_increment);
                        } else if last.avg_secondary_metric.unwrap_or(reps_target) + 0.5
                            < settings.min_secondary as f32
                        {
                            recommended_primary = round_metric(
                                (recommended_primary - analysis.metric_spec.primary_increment)
                                    .max(0.0),
                            );
                            reps_target = settings.min_secondary as f32;
                        } else if reps_target < settings.max_secondary as f32 {
                            reps_target += 1.0;
                        }
                    }
                }
            }

            recommended_secondary = Some(round_metric(reps_target));
        }
        TrackingMode::RepsOnly | TrackingMode::Duration => match payload.progression_preference {
            ProgressionPreference::Stagnation => {
                if let Some(last) = last_session {
                    recommended_primary =
                        round_metric((recommended_primary + last.avg_primary_metric) / 2.0);
                }
            }
            ProgressionPreference::ProgressiveOverload => {
                if analytics.trend != "downward" {
                    recommended_primary =
                        round_metric(recommended_primary + analysis.metric_spec.primary_increment);
                }
            }
        },
        TrackingMode::DistanceDuration => {
            let latest_secondary = last_session.and_then(|session| session.avg_secondary_metric);
            match payload.progression_preference {
                ProgressionPreference::Stagnation => {
                    if let Some(last) = last_session {
                        recommended_primary =
                            round_metric((recommended_primary + last.avg_primary_metric) / 2.0);
                        if let Some(duration) = latest_secondary {
                            recommended_secondary = Some(round_metric(
                                (recommended_secondary.unwrap_or(duration) + duration) / 2.0,
                            ));
                        }
                    }
                }
                ProgressionPreference::ProgressiveOverload => {
                    if analytics.trend != "downward" {
                        recommended_primary =
                            round_metric(recommended_primary + analysis.metric_spec.primary_increment);
                        if let (Some(distance), Some(duration)) = (
                            last_session.map(|session| session.avg_primary_metric),
                            latest_secondary,
                        ) {
                            let pace = if distance > 0.0 { duration / distance } else { 0.0 };
                            recommended_secondary =
                                Some(round_metric((recommended_primary * pace).max(duration)));
                        } else if let Some(duration) = recommended_secondary {
                            recommended_secondary =
                                Some(round_metric(duration + analysis.metric_spec.secondary_increment));
                        }
                    }
                }
            }
        }
    }

    if matches!(analysis.goal_focus, GoalFocus::Hypertrophy) && analytics.trend != "downward" {
        recommended_sets = recommended_sets.max(3);
    }

    RecommendationResponse {
        exercise_name: payload.exercise_name.clone(),
        tracking_mode: analysis.tracking_mode.as_str().into(),
        recommended_sets,
        primary_metric_label: analysis.metric_spec.primary_label.clone(),
        primary_metric_unit: analysis.metric_spec.primary_unit.clone(),
        secondary_metric_label: analysis.metric_spec.secondary_label.clone(),
        secondary_metric_unit: analysis.metric_spec.secondary_unit.clone(),
        recommended_primary_metric: round_metric(recommended_primary),
        recommended_secondary_metric: recommended_secondary.map(round_metric),
        goal_focus: analysis.goal_focus.as_str().into(),
        progression_preference: payload.progression_preference.as_str().into(),
        rationale: recommendation_rationale(analytics, analysis, recommended_sets, recommended_primary, recommended_secondary),
    }
}

fn recommendation_rationale(
    analytics: &AnalyticsReport,
    analysis: &ExerciseAnalysis,
    recommended_sets: u32,
    recommended_primary: f32,
    recommended_secondary: Option<f32>,
) -> String {
    let usual_pattern = if let Some(secondary) = analytics.typical_secondary_metric {
        format!(
            "{} sets around {} {} and {} {}",
            analytics.typical_sets,
            round_metric(analytics.typical_primary_metric),
            analytics.primary_metric_unit,
            round_metric(secondary),
            analytics.secondary_metric_unit.clone().unwrap_or_default()
        )
    } else {
        format!(
            "{} sets around {} {}",
            analytics.typical_sets,
            round_metric(analytics.typical_primary_metric),
            analytics.primary_metric_unit
        )
    };

    let next_pattern = if let Some(secondary) = recommended_secondary {
        format!(
            "{} sets with {} {} and {} {}",
            recommended_sets,
            round_metric(recommended_primary),
            analytics.primary_metric_unit,
            round_metric(secondary),
            analytics.secondary_metric_unit.clone().unwrap_or_default()
        )
    } else {
        format!(
            "{} sets with {} {}",
            recommended_sets,
            round_metric(recommended_primary),
            analytics.primary_metric_unit
        )
    };

    format!(
        "Based on {} sessions, your usual pattern is {}. Goal focus is {} and strategy is {}, so the next session targets {} with a {} trend in {}.",
        analytics.sessions_analyzed,
        usual_pattern,
        analytics.goal_focus,
        analytics.progression_preference,
        next_pattern,
        analytics.trend,
        analysis.metric_spec.total_output_label
    )
}

struct GoalSettings {
    min_secondary: u32,
    max_secondary: u32,
    default_secondary: u32,
}

fn goal_settings(goal_focus: GoalFocus) -> GoalSettings {
    match goal_focus {
        GoalFocus::Strength => GoalSettings {
            min_secondary: 5,
            max_secondary: 10,
            default_secondary: 8,
        },
        GoalFocus::Hypertrophy => GoalSettings {
            min_secondary: 8,
            max_secondary: 12,
            default_secondary: 10,
        },
        GoalFocus::Endurance => GoalSettings {
            min_secondary: 10,
            max_secondary: 15,
            default_secondary: 12,
        },
        GoalFocus::General => GoalSettings {
            min_secondary: 6,
            max_secondary: 12,
            default_secondary: 10,
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

fn primary_metric_value(tracking_mode: TrackingMode, set: &ExerciseSet) -> f32 {
    match tracking_mode {
        TrackingMode::LoadReps => set.load_kg.unwrap_or(0.0),
        TrackingMode::RepsOnly => set.reps.unwrap_or(0) as f32,
        TrackingMode::Duration => set.duration_min.unwrap_or(0.0),
        TrackingMode::DistanceDuration => set.distance_km.unwrap_or(0.0),
    }
}

fn secondary_metric_value(tracking_mode: TrackingMode, set: &ExerciseSet) -> Option<f32> {
    match tracking_mode {
        TrackingMode::LoadReps => Some(set.reps.unwrap_or(0) as f32),
        TrackingMode::DistanceDuration => Some(set.duration_min.unwrap_or(0.0)),
        TrackingMode::RepsOnly | TrackingMode::Duration => None,
    }
}

fn total_output_value(tracking_mode: TrackingMode, set: &ExerciseSet) -> f32 {
    match tracking_mode {
        TrackingMode::LoadReps => set.load_kg.unwrap_or(0.0) * set.reps.unwrap_or(0) as f32,
        TrackingMode::RepsOnly => set.reps.unwrap_or(0) as f32,
        TrackingMode::Duration => set.duration_min.unwrap_or(0.0),
        TrackingMode::DistanceDuration => set.distance_km.unwrap_or(0.0),
    }
}

fn detect_trend(sessions: &[SessionSummary], tracking_mode: TrackingMode) -> String {
    if sessions.len() < 2 {
        return "stable".into();
    }

    let threshold = tracking_mode.metric_spec().primary_threshold;
    let first = sessions.first().unwrap().avg_primary_metric;
    let last = sessions.last().unwrap().avg_primary_metric;

    if last > first + threshold {
        "upward".into()
    } else if last + threshold < first {
        "downward".into()
    } else {
        "stable".into()
    }
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

fn weighted_optional_average(values: &[(f32, f32)]) -> Option<f32> {
    if values.is_empty() {
        None
    } else {
        Some(weighted_average(values))
    }
}

fn has_consistent_pattern(sessions: &[SessionSummary], tracking_mode: TrackingMode) -> bool {
    if sessions.len() < 2 {
        return false;
    }

    let min_primary = sessions
        .iter()
        .map(|session| session.avg_primary_metric)
        .fold(f32::INFINITY, f32::min);
    let max_primary = sessions
        .iter()
        .map(|session| session.avg_primary_metric)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_secondary = sessions
        .iter()
        .filter_map(|session| session.avg_secondary_metric)
        .fold(f32::INFINITY, f32::min);
    let max_secondary = sessions
        .iter()
        .filter_map(|session| session.avg_secondary_metric)
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

    let primary_consistent = (max_primary - min_primary) <= tracking_mode.metric_spec().primary_threshold;
    let secondary_consistent = if min_secondary.is_infinite() || max_secondary.is_infinite() {
        true
    } else {
        (max_secondary - min_secondary) <= 1.0
    };

    primary_consistent && secondary_consistent && (max_sets - min_sets) <= 1
}

fn round_metric(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(date: &str, tracking_mode: TrackingMode, sets: Vec<ExerciseSet>) -> ExerciseSnapshot {
        ExerciseSnapshot {
            performed_on: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            tracking_mode,
            sets,
        }
    }

    #[test]
    fn load_and_reps_recommendation_still_works() {
        let payload = AnalyticsRequest {
            client_id: "1".into(),
            exercise_name: "Bench Press".into(),
            client_goals: vec!["strength".into()],
            progression_preference: ProgressionPreference::Stagnation,
            tracking_mode: TrackingMode::LoadReps,
            history: vec![
                snapshot(
                    "2026-06-01",
                    TrackingMode::LoadReps,
                    vec![
                        ExerciseSet { reps: Some(10), load_kg: Some(65.0), duration_min: None, distance_km: None },
                        ExerciseSet { reps: Some(10), load_kg: Some(65.0), duration_min: None, distance_km: None },
                    ],
                ),
                snapshot(
                    "2026-06-08",
                    TrackingMode::LoadReps,
                    vec![
                        ExerciseSet { reps: Some(10), load_kg: Some(65.0), duration_min: None, distance_km: None },
                        ExerciseSet { reps: Some(10), load_kg: Some(65.0), duration_min: None, distance_km: None },
                    ],
                ),
            ],
        };

        let analysis = analyze_history(&payload);
        let report = build_report(&payload, &analysis);
        let recommendation = build_recommendation(&payload, &analysis, &report);

        assert_eq!(recommendation.recommended_sets, 2);
        assert_eq!(recommendation.recommended_primary_metric, 65.0);
        assert_eq!(recommendation.recommended_secondary_metric, Some(10.0));
    }

    #[test]
    fn reps_only_mode_tracks_bodyweight_exercises() {
        let payload = AnalyticsRequest {
            client_id: "1".into(),
            exercise_name: "Push Ups".into(),
            client_goals: vec!["endurance".into()],
            progression_preference: ProgressionPreference::ProgressiveOverload,
            tracking_mode: TrackingMode::RepsOnly,
            history: vec![
                snapshot(
                    "2026-06-01",
                    TrackingMode::RepsOnly,
                    vec![
                        ExerciseSet { reps: Some(15), load_kg: None, duration_min: None, distance_km: None },
                        ExerciseSet { reps: Some(12), load_kg: None, duration_min: None, distance_km: None },
                    ],
                ),
                snapshot(
                    "2026-06-08",
                    TrackingMode::RepsOnly,
                    vec![
                        ExerciseSet { reps: Some(16), load_kg: None, duration_min: None, distance_km: None },
                        ExerciseSet { reps: Some(13), load_kg: None, duration_min: None, distance_km: None },
                    ],
                ),
            ],
        };

        let analysis = analyze_history(&payload);
        let report = build_report(&payload, &analysis);
        let recommendation = build_recommendation(&payload, &analysis, &report);

        assert_eq!(report.primary_metric_label, "reps");
        assert!(recommendation.recommended_primary_metric >= 15.0);
    }

    #[test]
    fn running_mode_returns_distance_and_duration() {
        let payload = AnalyticsRequest {
            client_id: "1".into(),
            exercise_name: "Running".into(),
            client_goals: vec!["conditioning".into()],
            progression_preference: ProgressionPreference::ProgressiveOverload,
            tracking_mode: TrackingMode::DistanceDuration,
            history: vec![
                snapshot(
                    "2026-06-01",
                    TrackingMode::DistanceDuration,
                    vec![ExerciseSet { reps: None, load_kg: None, duration_min: Some(28.0), distance_km: Some(5.0) }],
                ),
                snapshot(
                    "2026-06-08",
                    TrackingMode::DistanceDuration,
                    vec![ExerciseSet { reps: None, load_kg: None, duration_min: Some(33.0), distance_km: Some(6.0) }],
                ),
            ],
        };

        let analysis = analyze_history(&payload);
        let report = build_report(&payload, &analysis);
        let recommendation = build_recommendation(&payload, &analysis, &report);

        assert_eq!(report.primary_metric_unit, "km");
        assert_eq!(report.secondary_metric_unit.as_deref(), Some("min"));
        assert!(recommendation.recommended_primary_metric >= 6.0);
        assert!(recommendation.recommended_secondary_metric.is_some());
    }
}
