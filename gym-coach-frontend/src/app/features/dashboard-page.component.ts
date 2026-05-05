import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { TrainingSession, UserProfile } from '../core/models';
import { ProgressChartComponent } from '../shared/progress-chart.component';

@Component({
  selector: 'app-dashboard-page',
  imports: [CommonModule, ProgressChartComponent],
  templateUrl: './dashboard-page.component.html',
  styleUrl: './dashboard-page.component.scss',
})
export class DashboardPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);

  readonly loading = signal(true);
  readonly profiles = signal<UserProfile[]>([]);
  readonly trainings = signal<TrainingSession[]>([]);
  readonly programCount = signal(0);
  readonly error = signal('');

  readonly currentProfile = computed(() =>
    this.profiles().find((profile) => profile.id === this.authStore.userId()) ?? null,
  );
  readonly displayName = computed(
    () => this.currentProfile()?.full_name ?? this.authStore.claims()?.email ?? 'Athlete',
  );

  readonly benchTrend = computed(() => {
    const points = this.trainings()
      .flatMap((training) => training.exercise_groups)
      .flatMap((group) => group.exercises)
      .filter((exercise) => exercise.name === 'Bench Press')
      .map((exercise) => ({
        label: exercise.performed_on.slice(5),
        value:
          exercise.sets.reduce((sum, current) => sum + current.load_kg, 0) /
          Math.max(exercise.sets.length, 1),
      }));

    return {
      labels: points.map((point) => point.label),
      values: points.map((point) => point.value),
    };
  });

  constructor() {
    forkJoin({
      profiles: this.api.getProfiles(),
      trainings: this.api.getTrainings(),
      programs: this.api.getPrograms(),
    }).subscribe({
      next: ({ profiles, trainings, programs }) => {
        this.profiles.set(profiles);
        this.trainings.set(trainings);
        this.programCount.set(programs.length);
      },
      error: () => this.error.set('Dashboard could not load data from the gateway.'),
      complete: () => this.loading.set(false),
    });
  }
}
