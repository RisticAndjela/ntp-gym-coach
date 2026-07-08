export type UserRole = 'COACH' | 'CLIENT';
export type TrainingStatus = 'PLANNED' | 'COMPLETED' | 'SKIPPED';
export type TrackingMode = 'load_reps' | 'reps_only' | 'duration' | 'distance_duration';

export interface Claims {
  sub: string;
  email: string;
  role: UserRole;
  exp: number;
  iat: number;
}

export interface PublicUser {
  id: string;
  full_name: string;
  email: string;
  role: UserRole;
}

export interface AuthResponse {
  token: string;
  user: PublicUser;
}

export interface UserProfile {
  id: string;
  full_name: string;
  email: string;
  role: UserRole;
  goals: string[];
  offers: string[];
  bio: string;
  created_at: string;
}

export interface CoachMatch {
  coach_id: string;
  coach_name: string;
  matching_goals: string[];
  score: number;
}

export interface CoachClientLink {
  coach_id: string;
  client_id: string;
  created_at: string;
}

export interface TrainingSet {
  reps?: number;
  load_kg?: number;
  duration_min?: number;
  distance_km?: number;
}

export interface Exercise {
  name: string;
  exercise_type: string;
  tracking_mode: TrackingMode;
  performed_on: string;
  sets: TrainingSet[];
  media: MediaAsset[];
}

export interface ExerciseGroup {
  name: string;
  exercises: Exercise[];
}

export interface TrainingSession {
  id: string;
  coach_id: string | null;
  client_id: string;
  category: string;
  status: TrainingStatus;
  notes: string;
  exercise_groups: ExerciseGroup[];
}

export interface TrainingCatalog {
  categories: string[];
  exercise_types: string[];
}

export interface MediaAsset {
  title: string;
  media_type: string;
  url: string;
}

export interface ProgramExercise {
  name: string;
  sets: number;
  reps: string;
  media: MediaAsset[];
}

export interface ProgramDay {
  day: number;
  title: string;
  exercises: ProgramExercise[];
}

export interface ProgramWeek {
  week: number;
  days: ProgramDay[];
}

export interface TrainingProgram {
  id: string;
  title: string;
  level: string;
  goal: string;
  weeks: ProgramWeek[];
}

export interface AnalyticsRequest {
  client_id: string;
  exercise_name: string;
  client_goals?: string[];
  progression_preference?: 'stagnation' | 'progressive_overload';
  tracking_mode: TrackingMode;
  history: Array<{
    performed_on: string;
    tracking_mode: TrackingMode;
    sets: TrainingSet[];
  }>;
}

export interface AnalyticsReport {
  client_id: string;
  exercise_name: string;
  tracking_mode: TrackingMode;
  sessions_analyzed: number;
  primary_metric_label: string;
  primary_metric_unit: string;
  secondary_metric_label: string | null;
  secondary_metric_unit: string | null;
  avg_primary_metric: number;
  avg_secondary_metric: number | null;
  best_primary_metric: number;
  total_output: number;
  total_output_label: string;
  avg_sets_per_session: number;
  avg_output_per_session: number;
  typical_sets: number;
  typical_primary_metric: number;
  typical_secondary_metric: number | null;
  goal_focus: string;
  progression_preference: string;
  trend: string;
}

export interface RecommendationResponse {
  exercise_name: string;
  tracking_mode: TrackingMode;
  recommended_sets: number;
  primary_metric_label: string;
  primary_metric_unit: string;
  secondary_metric_label: string | null;
  secondary_metric_unit: string | null;
  recommended_primary_metric: number;
  recommended_secondary_metric: number | null;
  goal_focus: string;
  progression_preference: string;
  rationale: string;
}
