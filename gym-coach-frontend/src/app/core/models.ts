export type UserRole = 'COACH' | 'CLIENT';
export type TrainingStatus = 'PLANNED' | 'COMPLETED' | 'SKIPPED';

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
  reps: number;
  load_kg: number;
}

export interface Exercise {
  name: string;
  exercise_type: string;
  performed_on: string;
  sets: TrainingSet[];
}

export interface ExerciseGroup {
  name: string;
  exercises: Exercise[];
}

export interface TrainingSession {
  id: string;
  coach_id: string;
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
  history: Array<{
    performed_on: string;
    sets: TrainingSet[];
  }>;
}

export interface AnalyticsReport {
  client_id: string;
  exercise_name: string;
  sessions_analyzed: number;
  avg_load_kg: number;
  avg_reps: number;
  best_load_kg: number;
  total_volume: number;
  trend: string;
}

export interface RecommendationResponse {
  exercise_name: string;
  recommended_load_kg: number;
  recommended_reps: number;
  rationale: string;
}
