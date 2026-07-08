import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule } from '@angular/forms';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import {
  AnalyticsReport,
  AnalyticsRequest,
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
  readonly report = signal<AnalyticsReport | null>(null);
  readonly recommendation = signal<RecommendationResponse | null>(null);
  readonly error = signal('');

  readonly form = this.fb.nonNullable.group({
    exercise_name: ['Bench Press'],
    progression_preference: ['progressive_overload' as const],
  });
  readonly currentProfile = computed(
    () => this.profiles().find((profile) => profile.id === this.authStore.userId()) ?? null,
  );

  readonly exerciseNames = computed(() => {
    const names = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .map((exercise) => exercise.name);

    return this.uniqueCaseInsensitive(names);
  });

  readonly chartData = computed(() => {
    const selected = this.form.getRawValue().exercise_name;
    const points = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => this.equalsIgnoreCase(exercise.name, selected))
      .map((exercise) => ({
        label: exercise.performed_on.slice(5),
        value:
          exercise.sets.reduce((sum, current) => sum + current.load_kg * current.reps, 0) /
          Math.max(exercise.sets.length, 1),
      }));

    return {
      labels: points.map((point) => point.label),
      values: points.map((point) => point.value),
    };
  });

  constructor() {
    const userId = this.authStore.userId();
    if (!userId) {
      return;
    }

    forkJoin({
      trainings: this.api.getClientTrainings(userId),
      profiles: this.api.getProfiles(),
    }).subscribe({
      next: ({ trainings, profiles }) => {
        this.trainings.set(trainings);
        this.profiles.set(profiles);
        if (this.exerciseNames().length) {
          this.form.patchValue({ exercise_name: this.exerciseNames()[0] });
        }
        this.runAnalysis();
      },
      error: () => this.error.set('Analytics history could not be loaded.'),
    });
  }

  runAnalysis(): void {
    const userId = this.authStore.userId();
    const { exercise_name: exerciseName, progression_preference } = this.form.getRawValue();
    if (!userId || !exerciseName) {
      return;
    }

    const history = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => this.equalsIgnoreCase(exercise.name, exerciseName))
      .map((exercise) => ({
        performed_on: exercise.performed_on,
        sets: exercise.sets,
      }));

    const payload: AnalyticsRequest = {
      client_id: userId,
      exercise_name: exerciseName,
      client_goals: this.currentProfile()?.goals ?? [],
      progression_preference,
      history,
    };

    this.api.getAnalyticsReport(payload).subscribe({
      next: (report) => this.report.set(report),
      error: () => this.error.set('Analytics report failed.'),
    });

    this.api.getRecommendation(payload).subscribe({
      next: (recommendation) => this.recommendation.set(recommendation),
      error: () => this.error.set('Recommendation could not be generated.'),
    });
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
}
