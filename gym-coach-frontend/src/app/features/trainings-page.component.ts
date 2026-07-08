import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import {
  FormArray,
  FormBuilder,
  FormGroup,
  ReactiveFormsModule,
  Validators,
} from '@angular/forms';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { TrainingCatalog, TrainingSession, TrainingStatus, UserProfile } from '../core/models';

type ExerciseFormGroup = FormGroup;
type FlattenedExercise = {
  category: string;
  name: string;
  exercise_type: string;
};
type ExerciseFormValue = {
  name: string;
  exercise_type: string;
  sets: string;
  video_title: string;
  video_url: string;
};

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
  readonly editingTrainingId = signal<string | null>(null);
  readonly previewVideo = signal<{ title: string; url: string } | null>(null);

  readonly trainingForm = this.fb.nonNullable.group({
    category: ['Upper Body Strength', Validators.required],
    status: ['COMPLETED' as TrainingStatus, Validators.required],
    performed_on: ['2026-05-05', Validators.required],
    notes: [''],
    client_id: [''],
    exercise_groups: this.fb.array([this.createExerciseForm()]),
  });

  readonly coachProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'COACH'),
  );
  readonly clientProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'CLIENT'),
  );
  readonly isCoach = computed(() => this.authStore.role() === 'COACH');
  readonly isClient = computed(() => this.authStore.role() === 'CLIENT');

  get exercises(): FormArray<ExerciseFormGroup> {
    return this.trainingForm.controls.exercise_groups;
  }

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

  categoryOptions(): string[] {
    const selectedCategories = this.selectedExerciseFilters().map((filter) => filter.category);

    return this.uniqueSorted([
      ...this.catalog().categories,
      this.trainingForm.controls.category.value,
      ...selectedCategories,
      ...this.trainings().map((training) => training.category),
    ]);
  }

  exerciseTypeOptions(index: number): string[] {
    const category = this.trainingForm.controls.category.value.trim();
    const selectedExercise = this.exerciseControlValue(index, 'name');
    const currentType = this.exerciseControlValue(index, 'exercise_type');

    const matches = this.flattenExercises().filter((exercise) => {
      const categoryMatch = category && this.equalsIgnoreCase(exercise.category, category);
      const exerciseMatch = selectedExercise && this.equalsIgnoreCase(exercise.name, selectedExercise);
      return categoryMatch || exerciseMatch;
    });

    return this.uniqueSorted([
      ...this.catalog().exercise_types,
      currentType,
      ...matches.map((exercise) => exercise.exercise_type),
    ]);
  }

  exerciseNameOptions(index: number): string[] {
    const category = this.trainingForm.controls.category.value.trim();
    const selectedType = this.exerciseControlValue(index, 'exercise_type');
    const currentName = this.exerciseControlValue(index, 'name');

    const matches = this.flattenExercises().filter((exercise) => {
      const categoryMatch = category && this.equalsIgnoreCase(exercise.category, category);
      const typeMatch = selectedType && this.equalsIgnoreCase(exercise.exercise_type, selectedType);
      return categoryMatch || typeMatch;
    });

    return this.uniqueSorted([
      currentName,
      ...matches.map((exercise) => exercise.name),
    ]);
  }

  addExercise(): void {
    this.exercises.push(this.createExerciseForm());
  }

  removeExercise(index: number): void {
    if (this.exercises.length > 1) {
      this.exercises.removeAt(index);
    }
  }

  submitTraining(): void {
    const coachId = this.isCoach() ? this.authStore.userId() : this.coachProfiles()[0]?.id;
    const clientId = this.isClient()
      ? this.authStore.userId()
      : this.trainingForm.controls.client_id.value;
    const payload = this.trainingForm.getRawValue();

    if (!coachId) {
      this.error.set('No coach is available for this training entry.');
      return;
    }

    if (!clientId) {
      this.error.set('No client is available for this training entry.');
      return;
    }

    const requestPayload = {
      coach_id: coachId,
      client_id: clientId,
      category: payload.category,
      status: payload.status,
      notes: payload.notes,
      exercise_groups: [
        {
          name: 'Primary Work',
          exercises: payload.exercise_groups.map((exercise) => ({
            name: exercise['name'],
            exercise_type: exercise['exercise_type'],
            performed_on: payload.performed_on,
            sets: exercise['sets'].split(',').map((chunk: string) => {
              const [reps, load] = chunk.trim().split('x');
              return {
                reps: Number(reps),
                load_kg: Number(load),
              };
            }),
            media: exercise['video_url']
              ? [
                  {
                    title: exercise['video_title'] || `${exercise['name']} Demo`,
                    media_type: 'video',
                    url: exercise['video_url'],
                  },
                ]
              : [],
          })),
        },
      ],
    };

    const trainingId = this.editingTrainingId();
    const request$ = trainingId
      ? this.api.updateTraining(trainingId, requestPayload)
      : this.api.createTraining(requestPayload);

    request$.subscribe({
      next: () => {
        this.message.set(trainingId ? 'Training session updated.' : 'Training session saved.');
        this.error.set('');
        this.resetForm();
        this.load();
      },
      error: () =>
        this.error.set(trainingId ? 'Training update failed.' : 'Training creation failed.'),
    });
  }

  startEdit(training: TrainingSession): void {
    const exercises = training.exercise_groups.flatMap((group) => group.exercises);
    const firstExercise = exercises[0];

    this.editingTrainingId.set(training.id);
    this.message.set('');
    this.error.set('');

    while (this.exercises.length > 0) {
      this.exercises.removeAt(this.exercises.length - 1);
    }

    for (const exercise of exercises) {
      this.exercises.push(
        this.createExerciseForm({
          name: exercise.name,
          exercise_type: exercise.exercise_type,
          sets: exercise.sets.map((set) => `${set.reps}x${set.load_kg}`).join(', '),
          video_title: exercise.media[0]?.title ?? '',
          video_url: exercise.media[0]?.url ?? '',
        }),
      );
    }

    this.trainingForm.patchValue({
      category: training.category,
      status: training.status,
      performed_on: firstExercise?.performed_on ?? '2026-05-05',
      notes: training.notes,
      client_id: training.client_id,
    });
  }

  cancelEdit(): void {
    this.resetForm();
    this.message.set('');
    this.error.set('');
  }

  deleteTraining(training: TrainingSession): void {
    if (!globalThis.confirm(`Delete training "${training.category}"?`)) {
      return;
    }

    this.api.deleteTraining(training.id).subscribe({
      next: () => {
        if (this.editingTrainingId() === training.id) {
          this.resetForm();
        }
        this.message.set('Training session deleted.');
        this.error.set('');
        this.load();
      },
      error: () => this.error.set('Training deletion failed.'),
    });
  }

  isEditing(trainingId: string): boolean {
    return this.editingTrainingId() === trainingId;
  }

  openVideoPreview(title: string, url: string): void {
    this.previewVideo.set({ title, url });
  }

  closeVideoPreview(): void {
    this.previewVideo.set(null);
  }

  private createExerciseForm(initial?: Partial<ExerciseFormValue>): ExerciseFormGroup {
    const form = this.fb.nonNullable.group({
      name: [initial?.name ?? 'Bench Press', Validators.required],
      exercise_type: [initial?.exercise_type ?? 'compound', Validators.required],
      sets: [initial?.sets ?? '8x60, 8x62.5, 6x65', Validators.required],
      video_title: [initial?.video_title ?? (this.isCoach() ? 'Bench Press Demo' : '')],
      video_url: [
        initial?.video_url ?? (this.isCoach() ? 'https://www.w3schools.com/html/mov_bbb.mp4' : ''),
      ],
    });

    if (this.isClient()) {
      form.controls.video_title.disable();
      form.controls.video_url.disable();
    }

    return form;
  }

  trainingDate(training: TrainingSession): string {
    return (
      training.exercise_groups
        .flatMap((group) => group.exercises)
        .map((exercise) => exercise.performed_on)
        .find(Boolean) ?? ''
    );
  }

  private resetForm(): void {
    this.editingTrainingId.set(null);
    this.trainingForm.patchValue({
      category: 'Upper Body Strength',
      status: 'COMPLETED',
      performed_on: '2026-05-05',
      notes: '',
      client_id: this.isClient()
        ? this.authStore.userId() ?? ''
        : this.trainingForm.controls.client_id.value ||
          this.clientProfiles()[0]?.id ||
          this.authStore.userId() ||
          '',
    });

    while (this.exercises.length > 1) {
      this.exercises.removeAt(this.exercises.length - 1);
    }

    if (this.exercises.length === 0) {
      this.exercises.push(this.createExerciseForm());
    }

    this.exercises.at(0).patchValue({
      name: 'Bench Press',
      exercise_type: 'compound',
      sets: '8x60, 8x62.5, 6x65',
      video_title: 'Bench Press Demo',
      video_url: 'https://www.w3schools.com/html/mov_bbb.mp4',
    });
  }

  private exerciseControlValue(index: number, controlName: string): string {
    const value = this.exercises.at(index).get(controlName)?.value;
    return typeof value === 'string' ? value.trim() : '';
  }

  private flattenExercises(): FlattenedExercise[] {
    return this.trainings().flatMap((training) =>
      training.exercise_groups.flatMap((group) =>
        group.exercises.map((exercise) => ({
          category: training.category,
          name: exercise.name,
          exercise_type: exercise.exercise_type,
        })),
      ),
    );
  }

  private selectedExerciseFilters(): FlattenedExercise[] {
    const flattened = this.flattenExercises();
    const selectedNames = this.exercises.controls
      .map((_, index) => this.exerciseControlValue(index, 'name'))
      .filter(Boolean);
    const selectedTypes = this.exercises.controls
      .map((_, index) => this.exerciseControlValue(index, 'exercise_type'))
      .filter(Boolean);

    return flattened.filter(
      (exercise) =>
        selectedNames.some((name) => this.equalsIgnoreCase(name, exercise.name)) ||
        selectedTypes.some((type) => this.equalsIgnoreCase(type, exercise.exercise_type)),
    );
  }

  private uniqueSorted(values: string[]): string[] {
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
