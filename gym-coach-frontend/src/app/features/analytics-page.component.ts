import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule } from '@angular/forms';
import { forkJoin, of } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import {
  AnalyticsReport,
  AnalyticsRequest,
  CoachClientLink,
  Exercise,
  RecommendationResponse,
  TrainingSession,
  UserProfile,
} from '../core/models';
import { ProgressChartComponent } from '../shared/progress-chart.component';

@Component({
  selector: 'app-analytics-page',
  imports: [CommonModule, ReactiveFormsModule, ProgressChartComponent],
  templateUrl: './analytics-page.component.html',
  styleUrl: './analytics-page.component.scss',
})
export class AnalyticsPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);
  private readonly fb = inject(FormBuilder);

  readonly trainings = signal<TrainingSession[]>([]);
  readonly profiles = signal<UserProfile[]>([]);
  readonly coachConnections = signal<CoachClientLink[]>([]);
  readonly report = signal<AnalyticsReport | null>(null);
  readonly recommendation = signal<RecommendationResponse | null>(null);
  readonly error = signal('');
  readonly isCoach = computed(() => this.authStore.role() === 'COACH');

  readonly form = this.fb.nonNullable.group({
    client_id: [''],
    exercise_name: ['Bench Press'],
    progression_preference: ['progressive_overload' as const],
  });
  readonly selectedClientId = computed(() =>
    this.isCoach() ? this.form.getRawValue().client_id : this.authStore.userId() ?? '',
  );
  readonly connectedClientProfiles = computed(() => {
    const connectedClientIds = new Set(this.coachConnections().map((connection) => connection.client_id));
    return this.profiles()
      .filter((profile) => profile.role === 'CLIENT' && connectedClientIds.has(profile.id))
      .sort((left, right) => left.full_name.localeCompare(right.full_name, undefined, { sensitivity: 'base' }));
  });
  readonly currentProfile = computed(
    () => this.profiles().find((profile) => profile.id === this.selectedClientId()) ?? null,
  );
  readonly visibleTrainings = computed(() => {
    const clientId = this.selectedClientId();
    if (!clientId) {
      return [];
    }

    return this.trainings().filter((training) => training.client_id === clientId);
  });
  readonly emptyStateMessage = computed(() => {
    if (this.isCoach()) {
      if (!this.connectedClientProfiles().length) {
        return 'Analytics is available after you connect at least one client.';
      }

      if (!this.selectedClientId()) {
        return 'Choose a client to see exercise history and recommendations.';
      }
    }

    return 'No training history is available yet for analytics.';
  });

  readonly selectedExercises = computed(() => {
    const selected = this.form.getRawValue().exercise_name;
    return this.visibleTrainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => this.equalsIgnoreCase(exercise.name, selected));
  });

  readonly exerciseNames = computed(() => {
    const names = this.visibleTrainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .map((exercise) => exercise.name);

    return this.uniqueCaseInsensitive(names);
  });

  readonly chartData = computed(() => {
    const selected = this.selectedExercises();
    const recommendation = this.recommendation();
    const report = this.report();

    const history = selected.map((exercise) => ({
      label: exercise.performed_on.slice(5),
      value: this.primaryMetricValue(exercise),
    }));

    return {
      labels: history.map((point) => point.label),
      values: history.map((point) => point.value),
      recommendationValue: recommendation ? recommendation.recommended_primary_metric : null,
      metricTitle: report ? report.primary_metric_label : 'Progress metric',
      metricUnit: report ? report.primary_metric_unit : '',
    };
  });

  constructor() {
    const userId = this.authStore.userId();
    if (!userId) {
      return;
    }

    const loadRequest = this.isCoach()
      ? forkJoin({
          trainings: this.api.getTrainings(),
          profiles: this.api.getProfiles(),
          coachConnections: this.api.getCoachConnections(userId),
        })
      : forkJoin({
          trainings: this.api.getClientTrainings(userId),
          profiles: this.api.getProfiles(),
          coachConnections: of([] as CoachClientLink[]),
        });

    loadRequest.subscribe({
      next: ({ trainings, profiles, coachConnections }) => {
        this.profiles.set(profiles);
        this.coachConnections.set(coachConnections);
        this.trainings.set(this.filterVisibleTrainings(trainings, coachConnections, userId));

        const defaultClientId = this.isCoach()
          ? this.connectedClientProfiles()[0]?.id ?? ''
          : userId;

        this.form.patchValue({
          client_id: defaultClientId,
          exercise_name: this.exerciseNames()[0] ?? '',
        });

        if (this.exerciseNames().length) {
          this.runAnalysis();
        } else {
          this.error.set(this.emptyStateMessage());
        }
      },
      error: () => this.error.set('Analytics history could not be loaded.'),
    });
  }

  runAnalysis(): void {
    const userId = this.selectedClientId();
    const { exercise_name: exerciseName, progression_preference } = this.form.getRawValue();
    if (!userId || !exerciseName) {
      this.error.set(this.emptyStateMessage());
      this.report.set(null);
      this.recommendation.set(null);
      return;
    }

    const history = this.selectedExercises().map((exercise) => ({
      performed_on: exercise.performed_on,
      tracking_mode: exercise.tracking_mode,
      sets: exercise.sets,
    }));

    const trackingMode = history[0]?.tracking_mode;
    if (!trackingMode) {
      this.error.set('No history found for the selected exercise.');
      this.report.set(null);
      this.recommendation.set(null);
      return;
    }

    const payload: AnalyticsRequest = {
      client_id: userId,
      exercise_name: exerciseName,
      client_goals: this.currentProfile()?.goals ?? [],
      progression_preference,
      tracking_mode: trackingMode,
      history,
    };

    this.error.set('');

    this.api.getAnalyticsReport(payload).subscribe({
      next: (report) => this.report.set(report),
      error: () => this.error.set('Analytics report failed.'),
    });

    this.api.getRecommendation(payload).subscribe({
      next: (recommendation) => this.recommendation.set(recommendation),
      error: () => this.error.set('Recommendation could not be generated.'),
    });
  }

  selectClient(clientId: string): void {
    this.form.patchValue({
      client_id: clientId,
      exercise_name: '',
    });
    this.report.set(null);
    this.recommendation.set(null);

    const [firstExercise] = this.exerciseNames();
    if (!firstExercise) {
      this.error.set(this.emptyStateMessage());
      return;
    }

    this.form.patchValue({ exercise_name: firstExercise });
    this.runAnalysis();
  }

  metricValue(value: number | null | undefined, unit: string | null | undefined): string {
    if (value == null) {
      return '-';
    }

    const formatted = Number.isInteger(value) ? value.toString() : value.toFixed(1).replace(/\.0$/, '');
    return unit ? `${formatted} ${unit}` : formatted;
  }

  private primaryMetricValue(exercise: Exercise): number {
    switch (exercise.tracking_mode) {
      case 'load_reps':
        return Math.max(...exercise.sets.map((set) => set.load_kg ?? 0), 0);
      case 'reps_only':
        return Math.max(...exercise.sets.map((set) => set.reps ?? 0), 0);
      case 'duration':
        return Math.max(...exercise.sets.map((set) => set.duration_min ?? 0), 0);
      case 'distance_duration':
        return Math.max(...exercise.sets.map((set) => set.distance_km ?? 0), 0);
    }
  }

  private uniqueCaseInsensitive(values: string[]): string[] {
    const uniqueValues = new Map<string, string>();

    for (const value of values) {
      const trimmed = value.trim();
      if (!trimmed) {
        continue;
      }

      const key = this.normalizeText(trimmed);
      if (!uniqueValues.has(key)) {
        uniqueValues.set(key, trimmed);
      }
    }

    return [...uniqueValues.values()].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }));
  }

  private equalsIgnoreCase(left: string, right: string): boolean {
    return this.normalizeText(left) === this.normalizeText(right);
  }

  private normalizeText(value: string): string {
    return value.trim().toLocaleLowerCase();
  }

  private filterVisibleTrainings(
    trainings: TrainingSession[],
    coachConnections: CoachClientLink[],
    userId: string,
  ): TrainingSession[] {
    if (!this.isCoach()) {
      return trainings;
    }

    const connectedClientIds = new Set(coachConnections.map((connection) => connection.client_id));
    return trainings.filter(
      (training) =>
        training.coach_id === userId &&
        connectedClientIds.has(training.client_id),
    );
  }
}
