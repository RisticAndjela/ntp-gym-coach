import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';

import { ApiService } from '../core/api.service';
import { TrainingProgram } from '../core/models';

@Component({
  selector: 'app-programs-page',
  imports: [CommonModule],
  templateUrl: './programs-page.component.html',
  styleUrl: './programs-page.component.scss',
})
export class ProgramsPageComponent {
  private readonly api = inject(ApiService);

  readonly programs = signal<TrainingProgram[]>([]);
  readonly selectedProgramId = signal<string | null>(null);
  readonly error = signal('');
  readonly selectedProgram = computed(
    () => this.programs().find((item) => item.id === this.selectedProgramId()) ?? null,
  );

  constructor() {
    this.api.getPrograms().subscribe({
      next: (programs) => {
        this.programs.set(programs);
        this.selectedProgramId.set(programs[0]?.id ?? null);
      },
      error: () => this.error.set('Programs are currently unavailable.'),
    });
  }

  selectProgram(id: string): void {
    this.selectedProgramId.set(id);
  }
}
