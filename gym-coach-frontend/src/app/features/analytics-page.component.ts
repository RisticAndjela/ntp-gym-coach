import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule } from '@angular/forms';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import {
  AnalyticsReport,
  AnalyticsRequest,
  RecommendationResponse,
  TrainingSession,
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
  readonly report = signal<AnalyticsReport | null>(null);
  readonly recommendation = signal<RecommendationResponse | null>(null);
  readonly error = signal('');

  readonly form = this.fb.nonNullable.group({
    exercise_name: ['Bench Press'],
  });

  readonly exerciseNames = computed(() => {
    const names = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .map((exercise) => exercise.name);

    return [...new Set(names)];
  });

  readonly chartData = computed(() => {
    const selected = this.form.getRawValue().exercise_name;
    const points = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => exercise.name === selected)
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

    this.api.getClientTrainings(userId).subscribe({
      next: (trainings) => {
        this.trainings.set(trainings);
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
    const exerciseName = this.form.getRawValue().exercise_name;
    if (!userId || !exerciseName) {
      return;
    }

    const history = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => exercise.name === exerciseName)
      .map((exercise) => ({
        performed_on: exercise.performed_on,
        sets: exercise.sets,
      }));

    const payload: AnalyticsRequest = {
      client_id: userId,
      exercise_name: exerciseName,
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
}
