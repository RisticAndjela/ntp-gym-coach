import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { TrainingSession, UserProfile } from '../core/models';

interface CalendarTrainingEntry {
  training: TrainingSession;
  date: Date;
  dayKey: string;
}

interface CalendarDay {
  date: Date;
  dayKey: string;
  inCurrentMonth: boolean;
  isToday: boolean;
  trainings: CalendarTrainingEntry[];
}

@Component({
  selector: 'app-dashboard-page',
  imports: [CommonModule],
  templateUrl: './dashboard-page.component.html',
  styleUrl: './dashboard-page.component.scss',
})
export class DashboardPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);
  private readonly monthFormatter = new Intl.DateTimeFormat('en-US', {
    month: 'long',
    year: 'numeric',
  });
  private readonly dayFormatter = new Intl.DateTimeFormat('en-US', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  });

  readonly loading = signal(true);
  readonly profiles = signal<UserProfile[]>([]);
  readonly trainings = signal<TrainingSession[]>([]);
  readonly programCount = signal(0);
  readonly programTitle = signal('');
  readonly visibleMonthOffset = signal(0);
  readonly selectedDayKey = signal(this.toDayKey(new Date()));
  readonly error = signal('');

  readonly currentProfile = computed(() =>
    this.profiles().find((profile) => profile.id === this.authStore.userId()) ?? null,
  );
  readonly displayName = computed(
    () => this.currentProfile()?.full_name ?? this.authStore.claims()?.email ?? 'Athlete',
  );
  readonly completedSessions = computed(
    () => this.trainings().filter((training) => training.status === 'COMPLETED').length,
  );
  readonly plannedSessions = computed(
    () => this.trainings().filter((training) => training.status === 'PLANNED').length,
  );
  readonly skippedSessions = computed(
    () => this.trainings().filter((training) => training.status === 'SKIPPED').length,
  );
  readonly recentTrainings = computed(() => [...this.trainings()].slice(-4).reverse());
  readonly nextStep = computed(() => {
    if (this.plannedSessions() > 0) {
      return 'You have sessions ready for your next workout.';
    }

    if (this.completedSessions() > 0) {
      return 'You are building good momentum. Keep tracking your results and form.';
    }

    return 'Start with your program and first session whenever you are ready.';
  });
  readonly focusMessage = computed(() => {
    const goals = this.currentProfile()?.goals ?? [];
    if (goals.length) {
      return goals[0];
    }

    return this.programTitle()
      ? 'Follow your assigned program and log each session.'
      : 'Set a goal and build your routine step by step.';
  });
  readonly statusSummary = computed(() => {
    if (this.completedSessions() > 0 && this.plannedSessions() > 0) {
      return 'You are on track';
    }

    if (this.completedSessions() > 0) {
      return 'Progress is underway';
    }

    if (this.programCount() > 0) {
      return 'Program ready';
    }

    return 'Getting started';
  });
  readonly datedTrainings = computed<CalendarTrainingEntry[]>(() =>
    this.trainings()
      .map((training) => {
        const date = this.extractTrainingDate(training);
        if (!date) {
          return null;
        }

        return {
          training,
          date,
          dayKey: this.toDayKey(date),
        };
      })
      .filter((entry): entry is CalendarTrainingEntry => entry !== null)
      .sort((left, right) => left.date.getTime() - right.date.getTime()),
  );
  readonly calendarMonth = computed(() => {
    const latestDatedTraining = this.datedTrainings()[this.datedTrainings().length - 1];
    const anchor = latestDatedTraining?.date ?? new Date();
    return new Date(anchor.getFullYear(), anchor.getMonth() + this.visibleMonthOffset(), 1);
  });
  readonly calendarMonthLabel = computed(() => this.monthFormatter.format(this.calendarMonth()));
  readonly weekSummary = computed(() => {
    const today = new Date();
    const dayIndex = today.getDay();
    const mondayShift = dayIndex === 0 ? 6 : dayIndex - 1;
    const startOfWeek = new Date(today.getFullYear(), today.getMonth(), today.getDate() - mondayShift);
    const endOfWeek = new Date(today.getFullYear(), today.getMonth(), today.getDate() + (6 - mondayShift));
    const weeklyTrainings = this.datedTrainings().filter(
      (entry) => entry.date >= startOfWeek && entry.date <= endOfWeek,
    );

    return {
      completed: weeklyTrainings.filter((entry) => entry.training.status === 'COMPLETED').length,
      planned: weeklyTrainings.filter((entry) => entry.training.status === 'PLANNED').length,
    };
  });
  readonly calendarDays = computed<CalendarDay[]>(() => {
    const month = this.calendarMonth();
    const monthStart = new Date(month.getFullYear(), month.getMonth(), 1);
    const monthEnd = new Date(month.getFullYear(), month.getMonth() + 1, 0);
    const startOffset = (monthStart.getDay() + 6) % 7;
    const gridStart = new Date(monthStart);
    gridStart.setDate(monthStart.getDate() - startOffset);

    const trainingsByDay = new Map<string, CalendarTrainingEntry[]>();
    for (const entry of this.datedTrainings()) {
      const existing = trainingsByDay.get(entry.dayKey) ?? [];
      existing.push(entry);
      trainingsByDay.set(entry.dayKey, existing);
    }

    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(gridStart);
      date.setDate(gridStart.getDate() + index);
      const dayKey = this.toDayKey(date);

      return {
        date,
        dayKey,
        inCurrentMonth: date >= monthStart && date <= monthEnd,
        isToday: dayKey === this.toDayKey(new Date()),
        trainings: trainingsByDay.get(dayKey) ?? [],
      };
    });
  });
  readonly selectedDay = computed(() => {
    const selected = this.calendarDays().find((day) => day.dayKey === this.selectedDayKey());
    return selected ?? this.calendarDays().find((day) => day.isToday) ?? this.calendarDays()[0] ?? null;
  });
  readonly selectedDayLabel = computed(() =>
    this.selectedDay() ? this.dayFormatter.format(this.selectedDay()!.date) : '',
  );
  readonly selectedDaySessions = computed(() => this.selectedDay()?.trainings ?? []);

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
        this.programTitle.set(programs[0]?.title ?? '');
      },
      error: () =>
        this.error.set('We could not load your dashboard right now. Please try again shortly.'),
      complete: () => this.loading.set(false),
    });
  }

  selectDay(dayKey: string): void {
    this.selectedDayKey.set(dayKey);
  }

  changeMonth(direction: -1 | 1): void {
    this.visibleMonthOffset.update((value) => value + direction);
  }

  trainingStatusLabel(status: TrainingSession['status']): string {
    switch (status) {
      case 'COMPLETED':
        return 'Completed';
      case 'SKIPPED':
        return 'Skipped';
      default:
        return 'Planned';
    }
  }

  calendarStatus(day: CalendarDay): 'completed' | 'planned' | 'skipped' | 'mixed' | 'empty' {
    if (!day.trainings.length) {
      return 'empty';
    }

    const statuses = new Set(day.trainings.map((entry) => entry.training.status));
    if (statuses.size > 1) {
      return 'mixed';
    }

    const [status] = [...statuses];
    if (status === 'COMPLETED') {
      return 'completed';
    }
    if (status === 'SKIPPED') {
      return 'skipped';
    }
    return 'planned';
  }

  private extractTrainingDate(training: TrainingSession): Date | null {
    const performedOn = training.exercise_groups
      .flatMap((group) => group.exercises)
      .map((exercise) => exercise.performed_on)
      .find((date) => Boolean(date));

    return performedOn ? this.parseDateOnly(performedOn) : null;
  }

  private parseDateOnly(input: string): Date | null {
    const [year, month, day] = input.split('-').map(Number);
    if (!year || !month || !day) {
      return null;
    }

    return new Date(year, month - 1, day);
  }

  private toDayKey(date: Date): string {
    const year = date.getFullYear();
    const month = `${date.getMonth() + 1}`.padStart(2, '0');
    const day = `${date.getDate()}`.padStart(2, '0');
    return `${year}-${month}-${day}`;
  }
}
