import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable } from 'rxjs';

import {
  AnalyticsReport,
  AnalyticsRequest,
  AuthResponse,
  Claims,
  CoachClientLink,
  CoachMatch,
  RecommendationResponse,
  TrainingCatalog,
  TrainingProgram,
  TrainingSession,
  UserProfile,
  UserRole,
} from './models';

@Injectable({ providedIn: 'root' })
export class ApiService {
  private readonly http = inject(HttpClient);
  private readonly baseUrl = '/api';

  login(payload: { email: string; password: string }): Observable<AuthResponse> {
    return this.http.post<AuthResponse>(`${this.baseUrl}/auth/login`, payload);
  }

  register(payload: {
    full_name: string;
    email: string;
    password: string;
    role: UserRole;
  }): Observable<AuthResponse> {
    return this.http.post<AuthResponse>(`${this.baseUrl}/auth/register`, payload);
  }

  me(): Observable<Claims> {
    return this.http.get<Claims>(`${this.baseUrl}/auth/me`);
  }

  getProfiles(): Observable<UserProfile[]> {
    return this.http.get<UserProfile[]>(`${this.baseUrl}/users/profiles`);
  }

  getCoaches(): Observable<UserProfile[]> {
    return this.http.get<UserProfile[]>(`${this.baseUrl}/users/coaches`);
  }

  getCoachMatches(clientId: string): Observable<CoachMatch[]> {
    return this.http.get<CoachMatch[]>(`${this.baseUrl}/users/clients/${clientId}/matches`);
  }

  updateProfile(
    profileId: string,
    payload: Partial<Pick<UserProfile, 'full_name' | 'goals' | 'offers' | 'bio'>>,
  ): Observable<UserProfile> {
    return this.http.put<UserProfile>(`${this.baseUrl}/users/profiles/${profileId}`, payload);
  }

  createConnection(payload: { coach_id: string; client_id: string }): Observable<CoachClientLink> {
    return this.http.post<CoachClientLink>(`${this.baseUrl}/users/connections`, payload);
  }

  getCoachConnections(coachId: string): Observable<CoachClientLink[]> {
    return this.http.get<CoachClientLink[]>(
      `${this.baseUrl}/users/connections/coach/${coachId}`,
    );
  }

  getClientConnections(clientId: string): Observable<CoachClientLink[]> {
    return this.http.get<CoachClientLink[]>(
      `${this.baseUrl}/users/connections/client/${clientId}`,
    );
  }

  getTrainings(): Observable<TrainingSession[]> {
    return this.http.get<TrainingSession[]>(`${this.baseUrl}/trainings`);
  }

  getClientTrainings(clientId: string): Observable<TrainingSession[]> {
    return this.http.get<TrainingSession[]>(`${this.baseUrl}/trainings/client/${clientId}`);
  }

  createTraining(payload: Omit<TrainingSession, 'id'>): Observable<TrainingSession> {
    return this.http.post<TrainingSession>(`${this.baseUrl}/trainings`, payload);
  }

  updateTraining(
    trainingId: string,
    payload: Omit<TrainingSession, 'id'>,
  ): Observable<TrainingSession> {
    return this.http.put<TrainingSession>(`${this.baseUrl}/trainings/${trainingId}`, payload);
  }

  deleteTraining(trainingId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/trainings/${trainingId}`);
  }

  getTrainingCatalog(): Observable<TrainingCatalog> {
    return this.http.get<TrainingCatalog>(`${this.baseUrl}/trainings/catalog`);
  }

  getPrograms(): Observable<TrainingProgram[]> {
    return this.http.get<TrainingProgram[]>(`${this.baseUrl}/programs`);
  }

  getAnalyticsReport(payload: AnalyticsRequest): Observable<AnalyticsReport> {
    return this.http.post<AnalyticsReport>(`${this.baseUrl}/analytics/report`, payload);
  }

  getRecommendation(payload: AnalyticsRequest): Observable<RecommendationResponse> {
    return this.http.post<RecommendationResponse>(
      `${this.baseUrl}/analytics/recommendation`,
      payload,
    );
  }
}
