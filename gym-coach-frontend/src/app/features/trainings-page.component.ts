import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';
import {
  FormArray,
  FormBuilder,
  FormGroup,
  ReactiveFormsModule,
  Validators,
} from '@angular/forms';
import { forkJoin, of } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import {
  CoachClientLink,
  Exercise,
  TrackingMode,
  TrainingCatalog,
  TrainingSession,
  TrainingSet,
  TrainingStatus,
  UserProfile,
} from '../core/models';

type ExerciseFormGroup = FormGroup;
type FlattenedExercise = {
  category: string;
  name: string;
  exercise_type: string;
};
type ExerciseFormValue = {
  name: string;
  exercise_type: string;
  tracking_mode: TrackingMode;
  sets: string;
  video_title: string;
  video_url: string;
};

type TrackingModeOption = {
  value: TrackingMode;
  label: string;
  placeholder: string;
  helper: string;
  defaultValue: string;
};

const TRACKING_MODE_OPTIONS: TrackingModeOption[] = [
  {
    value: 'load_reps',
    label: 'Load + reps',
    placeholder: '8x60, 8x62.5, 6x65',
    helper: 'Format: reps x kg. Example: 8x60, 8x62.5',
    defaultValue: '8x60, 8x62.5, 6x65',
  },
  {
    value: 'reps_only',
    label: 'Reps only',
    placeholder: '15, 14, 12',
    helper: 'Format: one reps value per set. Example: 15, 14, 12',
    defaultValue: '15, 14, 12',
  },
  {
    value: 'duration',
    label: 'Duration',
    placeholder: '30min, 25min, 20min',
    helper: 'Format: one duration per set or block. Example: 30min, 25min',
    defaultValue: '30min, 25min, 20min',
  },
  {
    value: 'distance_duration',
    label: 'Running / cardio',
    placeholder: '5km/28min, 6km/33min',
    helper: 'Format: distance/time. Example: 5km/28min, 6km/33min',
    defaultValue: '5km/28min, 6km/33min',
  },
];

const INDIVIDUAL_TRAINING_FILTER = 'INDIVIDUAL';

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
  readonly clientConnections = signal<CoachClientLink[]>([]);
  readonly coachConnections = signal<CoachClientLink[]>([]);
  readonly catalog = signal<TrainingCatalog>({ categories: [], exercise_types: [] });
  readonly message = signal('');
  readonly error = signal('');
  readonly editingTrainingId = signal<string | null>(null);
  readonly previewVideo = signal<{ title: string; url: string } | null>(null);
  readonly sessionSearch = signal('');
  readonly selectedSessionStatus = signal<'ALL' | TrainingStatus>('ALL');
  readonly selectedSessionCategory = signal('ALL');
  readonly selectedSessionClientId = signal('ALL');
  readonly selectedSessionCoachId = signal('ALL');
  readonly categoryEntryMode = signal<'select' | 'input'>('select');
  readonly exerciseEntryModes = signal<Record<number, { name: 'select' | 'input'; exercise_type: 'select' | 'input' }>>({});
  readonly collapsedExercises = signal<Record<number, boolean>>({});
  readonly isCoach = computed(() => this.authStore.role() === 'COACH');
  readonly isClient = computed(() => this.authStore.role() === 'CLIENT');

  readonly trainingForm = this.fb.nonNullable.group({
    category: ['Upper Body Strength', Validators.required],
    status: ['COMPLETED' as TrainingStatus, Validators.required],
    performed_on: ['2026-05-05', Validators.required],
    notes: [''],
    coach_id: [''],
    client_id: [''],
    exercise_groups: this.fb.array([this.createExerciseForm()]),
  });

  readonly coachProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'COACH'),
  );
  readonly clientProfiles = computed(() =>
    this.profiles().filter((profile) => profile.role === 'CLIENT'),
  );
  readonly connectedCoachProfiles = computed(() => {
    const connectedCoachIds = new Set(this.clientConnections().map((connection) => connection.coach_id));
    return this.coachProfiles().filter((profile) => connectedCoachIds.has(profile.id));
  });
  readonly connectedClientProfiles = computed(() => {
    const connectedClientIds = new Set(this.coachConnections().map((connection) => connection.client_id));
    return this.clientProfiles().filter((profile) => connectedClientIds.has(profile.id));
  });
  readonly sessionCategoryOptions = computed(() =>
    this.uniqueSorted(this.trainings().map((training) => training.category)),
  );
  readonly sessionClientOptions = computed(() =>
    this.profileOptionsFromIds(this.trainings().map((training) => training.client_id)),
  );
  readonly sessionCoachOptions = computed(() =>
    this.profileOptionsFromIds(
      this.trainings()
        .map((training) => training.coach_id)
        .filter((coachId): coachId is string => Boolean(coachId)),
    ),
  );
  readonly filteredTrainings = computed(() => {
    const search = this.normalizeText(this.sessionSearch());
    const status = this.selectedSessionStatus();
    const category = this.selectedSessionCategory();
    const clientId = this.selectedSessionClientId();
    const coachId = this.selectedSessionCoachId();

    return this.trainings()
      .filter((training) => {
        if (status !== 'ALL' && training.status !== status) {
          return false;
        }

        if (category !== 'ALL' && !this.equalsIgnoreCase(training.category, category)) {
          return false;
        }

        if (clientId !== 'ALL' && training.client_id !== clientId) {
          return false;
        }

        if (coachId === INDIVIDUAL_TRAINING_FILTER) {
          if (training.coach_id !== null) {
            return false;
          }
        } else if (coachId !== 'ALL' && (training.coach_id ?? '') !== coachId) {
          return false;
        }

        if (!search) {
          return true;
        }

        const exerciseNames = training.exercise_groups
          .flatMap((group) => group.exercises)
          .map((exercise) => exercise.name)
          .join(' ');
        const haystack = [
          training.category,
          training.status,
          training.notes,
          this.trainingDate(training),
          this.profileName(training.client_id),
          training.coach_id ? this.profileName(training.coach_id) : 'Individual training',
          exerciseNames,
        ]
          .join(' ')
          .toLocaleLowerCase();

        return haystack.includes(search);
      })
      .sort((left, right) => this.compareTrainingDates(right, left));
  });

  get exercises(): FormArray<ExerciseFormGroup> {
    return this.trainingForm.controls.exercise_groups;
  }

  constructor() {
    this.load();
  }

  load(): void {
    const userId = this.authStore.userId();
    const loadRequest = this.isClient() && userId
      ? forkJoin({
          trainings: this.api.getClientTrainings(userId),
          profiles: this.api.getProfiles(),
          catalog: this.api.getTrainingCatalog(),
          clientConnections: this.api.getClientConnections(userId),
          coachConnections: of([] as CoachClientLink[]),
        })
      : this.isCoach() && userId
      ? forkJoin({
          trainings: this.api.getTrainings(),
          profiles: this.api.getProfiles(),
          catalog: this.api.getTrainingCatalog(),
          clientConnections: of([] as CoachClientLink[]),
          coachConnections: this.api.getCoachConnections(userId),
        })
      : forkJoin({
          trainings: this.api.getTrainings(),
          profiles: this.api.getProfiles(),
          catalog: this.api.getTrainingCatalog(),
          clientConnections: of([] as CoachClientLink[]),
          coachConnections: of([] as CoachClientLink[]),
        });

    loadRequest.subscribe({
      next: ({ trainings, profiles, catalog, clientConnections, coachConnections }) => {
        this.profiles.set(profiles);
        this.clientConnections.set(clientConnections);
        this.coachConnections.set(coachConnections);
        this.catalog.set(catalog);
        this.trainings.set(
          this.isCoach() && userId
            ? trainings.filter((training) => training.coach_id === userId)
            : trainings,
        );

        const defaultClient = this.isCoach()
          ? this.connectedClientProfiles()[0]?.id ??
            profiles.find((profile) => profile.role === 'CLIENT')?.id ??
            ''
          : this.authStore.userId() ??
            profiles.find((profile) => profile.role === 'CLIENT')?.id ??
            '';
        const defaultCoach = this.isClient()
          ? this.trainingForm.controls.coach_id.value || this.connectedCoachProfiles()[0]?.id || ''
          : this.authStore.userId() ?? '';

        this.trainingForm.patchValue({
          client_id: defaultClient,
          coach_id: defaultCoach,
        });
      },
      error: () => this.error.set('Training data could not be loaded.'),
    });
  }

  categoryMode(): 'select' | 'input' {
    return this.categoryEntryMode();
  }

  setCategoryMode(mode: 'select' | 'input'): void {
    this.categoryEntryMode.set(mode);

    if (mode === 'select') {
      const currentCategory = this.trainingForm.controls.category.value.trim();
      const options = this.categoryOptions();
      if (!options.some((option) => this.equalsIgnoreCase(option, currentCategory))) {
        this.trainingForm.controls.category.setValue(options[0] ?? '');
      }
    }
  }

  updateCategoryValue(value: string): void {
    this.trainingForm.controls.category.setValue(value);
  }

  categorySelectValue(): string {
    const currentCategory = this.trainingForm.controls.category.value.trim();
    const match = this.categoryOptions().find((option) => this.equalsIgnoreCase(option, currentCategory));
    return match ?? '';
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

    return this.uniqueSorted([currentName, ...matches.map((exercise) => exercise.name)]);
  }

  exerciseFieldMode(index: number, field: 'name' | 'exercise_type'): 'select' | 'input' {
    return this.exerciseEntryModes()[index]?.[field] ?? 'select';
  }

  setExerciseFieldMode(index: number, field: 'name' | 'exercise_type', mode: 'select' | 'input'): void {
    const modes = { ...this.exerciseEntryModes() };
    const current = modes[index] ?? { name: 'select' as const, exercise_type: 'select' as const };
    modes[index] = { ...current, [field]: mode };
    this.exerciseEntryModes.set(modes);

    if (mode === 'select') {
      const options = field === 'name' ? this.exerciseNameOptions(index) : this.exerciseTypeOptions(index);
      const currentValue = this.exerciseControlValue(index, field);
      if (!options.some((option) => this.equalsIgnoreCase(option, currentValue))) {
        this.exercises.at(index).get(field)?.setValue(options[0] ?? '');
      }
    }
  }

  updateExerciseField(index: number, field: 'name' | 'exercise_type', value: string): void {
    this.exercises.at(index).get(field)?.setValue(value);
  }

  exerciseSelectValue(index: number, field: 'name' | 'exercise_type'): string {
    const currentValue = this.exerciseControlValue(index, field);
    const options = field === 'name' ? this.exerciseNameOptions(index) : this.exerciseTypeOptions(index);
    const match = options.find((option) => this.equalsIgnoreCase(option, currentValue));
    return match ?? '';
  }

  trackingModeOptions(): TrackingModeOption[] {
    return TRACKING_MODE_OPTIONS;
  }

  exerciseTrackingMode(index: number): TrackingMode {
    return (this.exercises.at(index).get('tracking_mode')?.value as TrackingMode | undefined) ?? 'load_reps';
  }

  setExerciseTrackingMode(index: number, mode: TrackingMode): void {
    const exercise = this.exercises.at(index);
    exercise.get('tracking_mode')?.setValue(mode);
    exercise.get('sets')?.setValue(this.trackingModeConfig(mode).defaultValue);
  }

  exerciseSetsPlaceholder(index: number): string {
    return this.trackingModeConfig(this.exerciseTrackingMode(index)).placeholder;
  }

  exerciseSetsHelper(index: number): string {
    return this.trackingModeConfig(this.exerciseTrackingMode(index)).helper;
  }

  addExercise(): void {
    const newIndex = this.exercises.length;
    this.exercises.push(this.createExerciseForm());
    this.setExerciseModesForIndex(newIndex, 'select', 'select');
    this.setExerciseCollapsed(newIndex, false);
  }

  removeExercise(index: number): void {
    if (this.exercises.length > 1) {
      this.exercises.removeAt(index);
      this.reindexExerciseModes(index);
      this.reindexCollapsedExercises(index);
    }
  }

  isExerciseCollapsed(index: number): boolean {
    return this.collapsedExercises()[index] ?? false;
  }

  toggleExerciseCollapsed(index: number): void {
    this.collapsedExercises.update((collapsed) => ({
      ...collapsed,
      [index]: !(collapsed[index] ?? false),
    }));
  }

  submitTraining(): void {
    const selectedCoachId = this.trainingForm.controls.coach_id.value;
    const coachId = this.isCoach()
      ? this.authStore.userId()
      : selectedCoachId || null;
    const clientId = this.isClient()
      ? this.authStore.userId()
      : this.trainingForm.controls.client_id.value;
    const payload = this.trainingForm.getRawValue();

    if (!clientId) {
      this.error.set('No client is available for this training entry.');
      return;
    }

    try {
      const requestPayload: Omit<TrainingSession, 'id'> = {
        coach_id: coachId,
        client_id: clientId,
        category: payload.category,
        status: payload.status,
        notes: payload.notes,
        exercise_groups: [
          {
            name: 'Primary Work',
            exercises: payload.exercise_groups.map((exercise, index) => {
              const exerciseName = this.exerciseLabel(exercise['name'], index);
              return {
                name: exercise['name'],
                exercise_type: exercise['exercise_type'],
                tracking_mode: exercise['tracking_mode'],
                performed_on: payload.performed_on,
                sets: this.parseSets(exercise['sets'], exercise['tracking_mode'], exerciseName),
                media: exercise['video_url']
                  ? [
                      {
                        title: exercise['video_title'] || `${exercise['name']} Demo`,
                        media_type: 'video',
                        url: exercise['video_url'],
                      },
                    ]
                  : [],
              };
            }),
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
    } catch (error) {
      this.error.set(error instanceof Error ? error.message : 'Training format is not valid.');
    }
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

    this.exerciseEntryModes.set({});
    this.collapsedExercises.set({});

    for (const exercise of exercises) {
      const nextIndex = this.exercises.length;
      this.exercises.push(
        this.createExerciseForm({
          name: exercise.name,
          exercise_type: exercise.exercise_type,
          tracking_mode: exercise.tracking_mode,
          sets: this.formatExerciseSets(exercise),
          video_title: exercise.media[0]?.title ?? '',
          video_url: exercise.media[0]?.url ?? '',
        }),
      );
      this.setExerciseModesForIndex(nextIndex, 'select', 'select');
      this.setExerciseCollapsed(nextIndex, false);
    }

    this.trainingForm.patchValue({
      category: training.category,
      status: training.status,
      performed_on: firstExercise?.performed_on ?? '2026-05-05',
      notes: training.notes,
      coach_id: training.coach_id ?? '',
      client_id: training.client_id,
    });
    this.categoryEntryMode.set('select');
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

  updateSessionSearch(value: string): void {
    this.sessionSearch.set(value);
  }

  updateSessionStatus(value: string): void {
    this.selectedSessionStatus.set(value === 'ALL' ? 'ALL' : (value as TrainingStatus));
  }

  updateSessionCategory(value: string): void {
    this.selectedSessionCategory.set(value || 'ALL');
  }

  updateSessionClient(value: string): void {
    this.selectedSessionClientId.set(value || 'ALL');
  }

  updateSessionCoach(value: string): void {
    this.selectedSessionCoachId.set(value || 'ALL');
  }

  individualTrainingFilterValue(): string {
    return INDIVIDUAL_TRAINING_FILTER;
  }

  resetSessionFilters(): void {
    this.sessionSearch.set('');
    this.selectedSessionStatus.set('ALL');
    this.selectedSessionCategory.set('ALL');
    this.selectedSessionClientId.set('ALL');
    this.selectedSessionCoachId.set('ALL');
  }

  profileName(profileId: string): string {
    return this.profiles().find((profile) => profile.id === profileId)?.full_name ?? profileId;
  }

  trainingClientLabel(training: TrainingSession): string {
    return this.profileName(training.client_id);
  }

  trainingCoachLabel(training: TrainingSession): string {
    return training.coach_id ? this.profileName(training.coach_id) : 'Me';
  }

  exerciseSummary(exercise: Exercise): string {
    const modeLabel = this.trackingModeConfig(exercise.tracking_mode).label;
    const sets = this.formatExerciseSets(exercise);
    return `${modeLabel}: ${sets}`;
  }

  trainingDate(training: TrainingSession): string {
    return (
      training.exercise_groups
        .flatMap((group) => group.exercises)
        .map((exercise) => exercise.performed_on)
        .find(Boolean) ?? ''
    );
  }

  private createExerciseForm(initial?: Partial<ExerciseFormValue>): ExerciseFormGroup {
    const trackingMode = initial?.tracking_mode ?? 'load_reps';
    const form = this.fb.nonNullable.group({
      name: [initial?.name ?? 'Bench Press', Validators.required],
      exercise_type: [initial?.exercise_type ?? 'compound', Validators.required],
      tracking_mode: [trackingMode, Validators.required],
      sets: [initial?.sets ?? this.trackingModeConfig(trackingMode).defaultValue, Validators.required],
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

  private resetForm(): void {
    this.editingTrainingId.set(null);
    this.categoryEntryMode.set('select');
    this.exerciseEntryModes.set({});
    this.collapsedExercises.set({});
    this.trainingForm.patchValue({
      category: 'Upper Body Strength',
      status: 'COMPLETED',
      performed_on: '2026-05-05',
      notes: '',
      coach_id: this.isClient()
        ? this.trainingForm.controls.coach_id.value ||
          this.connectedCoachProfiles()[0]?.id ||
          ''
        : this.authStore.userId() ?? '',
      client_id: this.isClient()
        ? this.authStore.userId() ?? ''
        : this.trainingForm.controls.client_id.value ||
          this.connectedClientProfiles()[0]?.id ||
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
      tracking_mode: 'load_reps',
      sets: this.trackingModeConfig('load_reps').defaultValue,
      video_title: 'Bench Press Demo',
      video_url: 'https://www.w3schools.com/html/mov_bbb.mp4',
    });
    this.setExerciseModesForIndex(0, 'select', 'select');
    this.setExerciseCollapsed(0, false);
  }

  private parseSets(value: string, trackingMode: TrackingMode, exerciseName: string): TrainingSet[] {
    const normalizedValue = typeof value === 'string' ? value.trim() : '';
    const chunks = normalizedValue
      .split(',')
      .map((chunk) => chunk.trim())
      .filter(Boolean);

    if (!chunks.length) {
      throw new Error(
        `"${exerciseName}" has no valid set entries. Enter values like "8x60, 8x62.5" or "15, 14, 12".`,
      );
    }

    return chunks.map((chunk) => this.parseSetChunk(chunk, trackingMode, exerciseName));
  }

  private parseSetChunk(chunk: string, trackingMode: TrackingMode, exerciseName: string): TrainingSet {
    switch (trackingMode) {
      case 'load_reps': {
        const match = chunk.match(/^(\d+(?:[.,]\d+)?)\s*x\s*(\d+(?:[.,]\d+)?)$/i);
        if (!match) {
          throw new Error(
            `"${exerciseName}" has invalid set "${chunk}". Load + reps format should look like 8x60, 8x62.5.`,
          );
        }

        return {
          reps: Math.round(this.parseNumericValue(match[1])),
          load_kg: this.parseNumericValue(match[2]),
        };
      }
      case 'reps_only':
        return {
          reps: Math.round(this.parseNumericValue(chunk)),
        };
      case 'duration':
        return {
          duration_min: this.parseNumericValue(chunk.replace(/min/gi, '')),
        };
      case 'distance_duration': {
        const match = chunk.match(/^(\d+(?:[.,]\d+)?)\s*km\s*\/\s*(\d+(?:[.,]\d+)?)\s*min$/i);
        if (!match) {
          throw new Error(
            `"${exerciseName}" has invalid set "${chunk}". Running format should look like 5km/28min, 6km/33min.`,
          );
        }

        return {
          distance_km: this.parseNumericValue(match[1]),
          duration_min: this.parseNumericValue(match[2]),
        };
      }
    }
  }

  private formatExerciseSets(exercise: Exercise): string {
    return exercise.sets.map((set) => this.formatSet(set, exercise.tracking_mode)).join(', ');
  }

  private formatSet(set: TrainingSet, trackingMode: TrackingMode): string {
    switch (trackingMode) {
      case 'load_reps':
        return `${set.reps ?? 0}x${this.formatDecimal(set.load_kg ?? 0)}`;
      case 'reps_only':
        return `${set.reps ?? 0}`;
      case 'duration':
        return `${this.formatDecimal(set.duration_min ?? 0)}min`;
      case 'distance_duration':
        return `${this.formatDecimal(set.distance_km ?? 0)}km/${this.formatDecimal(set.duration_min ?? 0)}min`;
    }
  }

  private parseNumericValue(value: string): number {
    const normalized = value.trim().replace(',', '.');
    const parsed = Number(normalized);
    if (!Number.isFinite(parsed)) {
      throw new Error('One of the entered values is not a valid number.');
    }

    return parsed;
  }

  private formatDecimal(value: number): string {
    return Number.isInteger(value) ? value.toString() : value.toFixed(1).replace(/\.0$/, '');
  }

  private exerciseLabel(name: string | undefined, index: number): string {
    const trimmedName = typeof name === 'string' ? name.trim() : '';
    return trimmedName ? `Exercise ${index + 1} (${trimmedName})` : `Exercise ${index + 1}`;
  }

  private trackingModeConfig(mode: TrackingMode): TrackingModeOption {
    return TRACKING_MODE_OPTIONS.find((option) => option.value === mode) ?? TRACKING_MODE_OPTIONS[0];
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

  private profileOptionsFromIds(profileIds: string[]): UserProfile[] {
    const uniqueIds = [...new Set(profileIds)];
    return uniqueIds
      .map((profileId) => this.profiles().find((profile) => profile.id === profileId) ?? null)
      .filter((profile): profile is UserProfile => profile !== null)
      .sort((left, right) => left.full_name.localeCompare(right.full_name, undefined, { sensitivity: 'base' }));
  }

  private equalsIgnoreCase(left: string, right: string): boolean {
    return this.normalizeText(left) === this.normalizeText(right);
  }

  private normalizeText(value: string): string {
    return value.trim().toLocaleLowerCase();
  }

  private compareTrainingDates(left: TrainingSession, right: TrainingSession): number {
    const leftDate = this.trainingDate(left);
    const rightDate = this.trainingDate(right);

    return leftDate.localeCompare(rightDate);
  }

  private setExerciseModesForIndex(
    index: number,
    nameMode: 'select' | 'input',
    typeMode: 'select' | 'input',
  ): void {
    this.exerciseEntryModes.update((modes) => ({
      ...modes,
      [index]: {
        name: nameMode,
        exercise_type: typeMode,
      },
    }));
  }

  private reindexExerciseModes(removedIndex: number): void {
    const nextModes: Record<number, { name: 'select' | 'input'; exercise_type: 'select' | 'input' }> = {};

    for (const [rawIndex, mode] of Object.entries(this.exerciseEntryModes())) {
      const index = Number(rawIndex);
      if (index < removedIndex) {
        nextModes[index] = mode;
      } else if (index > removedIndex) {
        nextModes[index - 1] = mode;
      }
    }

    this.exerciseEntryModes.set(nextModes);
  }

  private setExerciseCollapsed(index: number, collapsed: boolean): void {
    this.collapsedExercises.update((entries) => ({
      ...entries,
      [index]: collapsed,
    }));
  }

  private reindexCollapsedExercises(removedIndex: number): void {
    const nextCollapsed: Record<number, boolean> = {};

    for (const [rawIndex, collapsed] of Object.entries(this.collapsedExercises())) {
      const index = Number(rawIndex);
      if (index < removedIndex) {
        nextCollapsed[index] = collapsed;
      } else if (index > removedIndex) {
        nextCollapsed[index - 1] = collapsed;
      }
    }

    this.collapsedExercises.set(nextCollapsed);
  }
}
