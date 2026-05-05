import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { TrainingCatalog, TrainingSession, TrainingStatus, UserProfile } from '../core/models';

@Component({
  selector: 'app-trainings-page',
  imports: [CommonModule, ReactiveFormsModule],
  templateUrl: './trainings-page.component.html',
  styleUrl: './trainings-page.component.scss',
})
export class TrainingsPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);
  private readonly fb = inject(FormBuilder);

  readonly trainings = signal<TrainingSession[]>([]);
  readonly profiles = signal<UserProfile[]>([]);
  readonly catalog = signal<TrainingCatalog>({ categories: [], exercise_types: [] });
  readonly message = signal('');
  readonly error = signal('');

  readonly trainingForm = this.fb.nonNullable.group({
    category: ['Upper Body Strength', Validators.required],
    status: ['COMPLETED' as TrainingStatus, Validators.required],
    notes: [''],
    client_id: [''],
    exercise_name: ['Bench Press', Validators.required],
    exercise_type: ['compound', Validators.required],
    performed_on: ['2026-05-05', Validators.required],
    sets: ['8x60, 8x62.5, 6x65', Validators.required],
  });

  readonly coachProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'COACH'),
  );
  readonly clientProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'CLIENT'),
  );

  constructor() {
    this.load();
  }

  load(): void {
    forkJoin({
      trainings: this.api.getTrainings(),
      profiles: this.api.getProfiles(),
      catalog: this.api.getTrainingCatalog(),
    }).subscribe({
      next: ({ trainings, profiles, catalog }) => {
        this.trainings.set(trainings);
        this.profiles.set(profiles);
        this.catalog.set(catalog);

        const defaultClient =
          profiles.find((profile) => profile.role === 'CLIENT')?.id ??
          this.authStore.userId() ??
          '';
        this.trainingForm.patchValue({ client_id: defaultClient });
      },
      error: () => this.error.set('Training data could not be loaded.'),
    });
  }

  submitTraining(): void {
    const coachId =
      this.authStore.role() === 'COACH'
        ? this.authStore.userId()
        : this.coachProfiles()[0]?.id;
    const payload = this.trainingForm.getRawValue();

    if (!coachId) {
      this.error.set('No coach is available for this training entry.');
      return;
    }

    this.api
      .createTraining({
        coach_id: coachId,
        client_id: payload.client_id,
        category: payload.category,
        status: payload.status,
        notes: payload.notes,
        exercise_groups: [
          {
            name: 'Primary Work',
            exercises: [
              {
                name: payload.exercise_name,
                exercise_type: payload.exercise_type,
                performed_on: payload.performed_on,
                sets: payload.sets.split(',').map((chunk) => {
                  const [reps, load] = chunk.trim().split('x');
                  return {
                    reps: Number(reps),
                    load_kg: Number(load),
                  };
                }),
              },
            ],
          },
        ],
      })
      .subscribe({
        next: () => {
          this.message.set('Training session saved.');
          this.load();
        },
        error: () => this.error.set('Training creation failed.'),
      });
  }
}
