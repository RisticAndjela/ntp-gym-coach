import { CommonModule } from '@angular/common';
import { Component, computed, inject, signal } from '@angular/core';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import {
  CoachClientLink,
  MediaAsset,
  ProgramDay,
  ProgramExercise,
  ProgramWeek,
  TrainingProgram,
  TrainingSession,
  TrainingStatus,
  UserProfile,
} from '../core/models';

@Component({
  selector: 'app-programs-page',
  imports: [CommonModule],
  templateUrl: './programs-page.component.html',
  styleUrl: './programs-page.component.scss',
})
export class ProgramsPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);

  readonly programs = signal<TrainingProgram[]>([]);
  readonly selectedProgramId = signal<string | null>(null);
  readonly error = signal('');
  readonly message = signal('');
  readonly previewMedia = signal<MediaAsset | null>(null);
  readonly profiles = signal<UserProfile[]>([]);
  readonly clientConnections = signal<CoachClientLink[]>([]);
  readonly coachConnections = signal<CoachClientLink[]>([]);
  readonly plannerState = signal<{
    program: TrainingProgram;
    week: ProgramWeek;
    day: ProgramDay;
  } | null>(null);
  readonly selectedCoachId = signal('');
  readonly selectedClientId = signal('');
  readonly selectedDate = signal(this.todayLabel());
  readonly savingDayKey = signal<string | null>(null);
  readonly isClient = computed(() => this.authStore.role() === 'CLIENT');
  readonly isCoach = computed(() => this.authStore.role() === 'COACH');
  readonly connectedCoachProfiles = computed(() => {
    const connectedCoachIds = new Set(this.clientConnections().map((connection) => connection.coach_id));
    return this.profiles().filter(
      (profile) => profile.role === 'COACH' && connectedCoachIds.has(profile.id),
    );
  });
  readonly connectedClientProfiles = computed(() => {
    const connectedClientIds = new Set(this.coachConnections().map((connection) => connection.client_id));
    return this.profiles().filter(
      (profile) => profile.role === 'CLIENT' && connectedClientIds.has(profile.id),
    );
  });
  readonly selectedProgram = computed(
    () => this.programs().find((item) => item.id === this.selectedProgramId()) ?? null,
  );

  constructor() {
    this.loadPrograms();
    this.loadPlannerContext();
  }

  private loadPrograms(): void {
    this.api.getPrograms().subscribe({
      next: (programs) => {
        this.programs.set(programs);
        this.selectedProgramId.set(programs[0]?.id ?? null);
      },
      error: () => this.error.set('Programs are currently unavailable.'),
    });
  }

  private loadPlannerContext(): void {
    const userId = this.authStore.userId();
    if (!userId) {
      return;
    }

    this.api.getProfiles().subscribe({
      next: (profiles) => this.profiles.set(profiles),
      error: () => this.error.set('Programs loaded, but assignment context could not be checked.'),
    });

    if (this.isClient()) {
      this.api.getClientConnections(userId).subscribe({
        next: (connections) => {
          this.clientConnections.set(connections);
          this.selectedCoachId.set(connections[0]?.coach_id ?? '');
        },
        error: () => this.error.set('Programs loaded, but coach connection could not be checked.'),
      });
    }

    if (this.isCoach()) {
      this.api.getCoachConnections(userId).subscribe({
        next: (connections) => {
          this.coachConnections.set(connections);
          this.selectedClientId.set(connections[0]?.client_id ?? '');
        },
        error: () => this.error.set('Programs loaded, but client connection could not be checked.'),
      });
    }
  }

  selectProgram(id: string): void {
    this.selectedProgramId.set(id);
  }

  openMediaPreview(media: MediaAsset): void {
    this.previewMedia.set(media);
  }

  closeMediaPreview(): void {
    this.previewMedia.set(null);
  }

  openPlanner(program: TrainingProgram, week: ProgramWeek, day: ProgramDay): void {
    this.message.set('');
    this.error.set('');
    this.selectedDate.set(this.todayLabel());
    this.plannerState.set({ program, week, day });
  }

  isSavingDay(programId: string, weekNumber: number, dayNumber: number): boolean {
    return this.savingDayKey() === this.dayKey(programId, weekNumber, dayNumber);
  }

  selectCoach(coachId: string): void {
    this.selectedCoachId.set(coachId);
  }

  selectClient(clientId: string): void {
    this.selectedClientId.set(clientId);
  }

  updateSelectedDate(date: string): void {
    this.selectedDate.set(date);
  }

  closePlanner(): void {
    this.plannerState.set(null);
  }

  submitPlanner(): void {
    const planner = this.plannerState();
    if (!planner) {
      return;
    }

    const dayKey = this.dayKey(planner.program.id, planner.week.week, planner.day.day);
    const clientId = this.isCoach() ? this.selectedClientId() : this.authStore.userId();
    const coachId = this.isCoach() ? this.authStore.userId() : this.selectedCoachId() || null;

    this.message.set('');
    this.error.set('');

    if (!clientId) {
      this.error.set(this.isCoach() ? 'Choose a client before assigning this workout.' : 'You need to be signed in as a client.');
      return;
    }

    if (!this.selectedDate()) {
      this.error.set('Choose a training date before saving.');
      return;
    }

    this.savingDayKey.set(dayKey);

    const requestPayload: Omit<TrainingSession, 'id'> = {
      coach_id: coachId,
      client_id: clientId,
      category: planner.day.title,
      status: 'PLANNED' as TrainingStatus,
      notes: this.isCoach()
        ? `Assigned from program "${planner.program.title}" (week ${planner.week.week}, day ${planner.day.day}).`
        : `Saved from program "${planner.program.title}" (week ${planner.week.week}, day ${planner.day.day}).`,
      exercise_groups: [
        {
          name: planner.day.title,
          exercises: planner.day.exercises.map((exercise) => ({
            name: exercise.name,
            exercise_type: 'program',
            tracking_mode: 'load_reps',
            performed_on: this.selectedDate(),
            sets: this.programExerciseSets(exercise),
            media: exercise.media,
          })),
        },
      ],
    };

    this.api.createTraining(requestPayload).subscribe({
      next: () => {
        this.message.set(
          this.isCoach()
            ? `"${planner.day.title}" was assigned to the selected client.`
            : `"${planner.day.title}" was added to your trainings as planned.`,
        );
        this.savingDayKey.set(null);
        this.closePlanner();
      },
      error: () => {
        this.error.set('Planned training could not be created.');
        this.savingDayKey.set(null);
      },
    });
  }

  async downloadProgramPdf(program: TrainingProgram): Promise<void> {
    const { jsPDF, autoTable } = await this.loadPdfTools();
    const pdf = new jsPDF({
      orientation: 'portrait',
      unit: 'mm',
      format: 'a4',
    });

    const palette = {
      navy: [13, 27, 42] as const,
      blue: [65, 90, 119] as const,
      aqua: [72, 202, 228] as const,
      gold: [255, 183, 3] as const,
      cream: [248, 249, 250] as const,
      ink: [30, 41, 59] as const,
      slate: [100, 116, 139] as const,
      line: [226, 232, 240] as const,
    };

    const metrics = this.programMetrics(program);
    const pageWidth = pdf.internal.pageSize.getWidth();
    const pageHeight = pdf.internal.pageSize.getHeight();
    const margin = 14;
    let cursorY = 18;

    pdf.setFillColor(...palette.navy);
    pdf.rect(0, 0, pageWidth, 52, 'F');
    pdf.setFillColor(...palette.aqua);
    pdf.circle(pageWidth - 22, 12, 16, 'F');
    pdf.setFillColor(...palette.gold);
    pdf.circle(pageWidth - 10, 30, 10, 'F');

    pdf.setTextColor(...palette.cream);
    pdf.setFont('helvetica', 'bold');
    pdf.setFontSize(23);
    pdf.text(program.title, margin, cursorY);
    cursorY += 8;

    pdf.setFontSize(11);
    pdf.setFont('helvetica', 'normal');
    const goalLines = pdf.splitTextToSize(program.goal, 110);
    pdf.text(goalLines, margin, cursorY);

    pdf.setDrawColor(...palette.line);
    pdf.setFillColor(255, 255, 255);
    pdf.roundedRect(margin, 58, pageWidth - margin * 2, 26, 4, 4, 'FD');

    const metricCards = [
      { label: 'Level', value: program.level },
      { label: 'Weeks', value: String(metrics.totalWeeks) },
      { label: 'Training Days', value: String(metrics.totalDays) },
      { label: 'Exercises', value: String(metrics.totalExercises) },
    ];

    metricCards.forEach((metric, index) => {
      const x = margin + index * ((pageWidth - margin * 2) / 4);
      if (index > 0) {
        pdf.setDrawColor(...palette.line);
        pdf.line(x, 61, x, 81);
      }
      pdf.setTextColor(...palette.slate);
      pdf.setFont('helvetica', 'bold');
      pdf.setFontSize(9);
      pdf.text(metric.label.toUpperCase(), x + 4, 66);
      pdf.setTextColor(...palette.ink);
      pdf.setFontSize(15);
      pdf.text(metric.value, x + 4, 75);
    });

    const chartTop = 94;
    pdf.setFillColor(255, 255, 255);
    pdf.setDrawColor(...palette.line);
    pdf.roundedRect(margin, chartTop, pageWidth - margin * 2, 52, 4, 4, 'FD');
    pdf.setTextColor(...palette.ink);
    pdf.setFont('helvetica', 'bold');
    pdf.setFontSize(13);
    pdf.text('Weekly Exercise Distribution', margin + 5, chartTop + 8);
    pdf.setFont('helvetica', 'normal');
    pdf.setTextColor(...palette.slate);
    pdf.setFontSize(9);
    pdf.text('The PDF intentionally excludes demo videos and focuses on printable coaching structure.', margin + 5, chartTop + 14);

    this.drawWeekChart(pdf, program.weeks, {
      x: margin + 5,
      y: chartTop + 18,
      width: pageWidth - margin * 2 - 10,
      height: 24,
      palette,
    });

    pdf.setFont('helvetica', 'bold');
    pdf.setTextColor(...palette.ink);
    pdf.setFontSize(13);
    pdf.text('Program Breakdown', margin, 158);

    autoTable(pdf, {
      startY: 162,
      margin: { left: margin, right: margin },
      head: [['Week', 'Day', 'Focus', 'Exercise', 'Prescription', 'Volume']],
      body: this.programRows(program),
      theme: 'grid',
      headStyles: {
        fillColor: [...palette.blue],
        textColor: 255,
        fontStyle: 'bold',
        halign: 'center',
      },
      styles: {
        font: 'helvetica',
        fontSize: 9,
        cellPadding: 3,
        lineColor: [...palette.line],
        lineWidth: 0.2,
        textColor: [...palette.ink],
      },
      alternateRowStyles: {
        fillColor: [247, 250, 252],
      },
      columnStyles: {
        0: { cellWidth: 16, halign: 'center' },
        1: { cellWidth: 16, halign: 'center' },
        2: { cellWidth: 33 },
        3: { cellWidth: 44 },
        4: { cellWidth: 30, halign: 'center' },
        5: { cellWidth: 24, halign: 'center' },
      },
      didParseCell: (hookData) => {
        if (hookData.section === 'body' && hookData.column.index === 0) {
          hookData.cell.styles.fontStyle = 'bold';
        }
      },
      didDrawPage: () => {
        const currentPage = pdf.getNumberOfPages();
        pdf.setFontSize(8);
        pdf.setTextColor(...palette.slate);
        pdf.text(
          `Generated ${this.generatedAtLabel()}  |  Page ${currentPage}`,
          margin,
          pageHeight - 8,
        );
      },
    });

    pdf.save(`${this.fileSafeName(program.title)}.pdf`);
  }

  private programMetrics(program: TrainingProgram): {
    totalWeeks: number;
    totalDays: number;
    totalExercises: number;
    totalSets: number;
  } {
    const totalWeeks = program.weeks.length;
    const totalDays = program.weeks.reduce((sum, week) => sum + week.days.length, 0);
    const allExercises = program.weeks.flatMap((week) => week.days.flatMap((day) => day.exercises));
    const totalExercises = allExercises.length;
    const totalSets = allExercises.reduce((sum, exercise) => sum + exercise.sets, 0);

    return {
      totalWeeks,
      totalDays,
      totalExercises,
      totalSets,
    };
  }

  private programRows(program: TrainingProgram): string[][] {
    return program.weeks.flatMap((week) =>
      week.days.flatMap((day) =>
        day.exercises.map((exercise) => [
          `W${week.week}`,
          `D${day.day}`,
          day.title,
          exercise.name,
          `${exercise.sets} x ${exercise.reps}`,
          this.exerciseVolumeLabel(exercise),
        ]),
      ),
    );
  }

  private exerciseVolumeLabel(exercise: ProgramExercise): string {
    const reps = exercise.reps.trim();
    return /^\d+$/.test(reps) ? `${exercise.sets * Number(reps)} reps` : `${exercise.sets} sets`;
  }

  private drawWeekChart(
    pdf: {
      setDrawColor: (...args: number[]) => void;
      line: (x1: number, y1: number, x2: number, y2: number) => void;
      setFillColor: (...args: number[]) => void;
      roundedRect: (
        x: number,
        y: number,
        w: number,
        h: number,
        rx: number,
        ry: number,
        style: string,
      ) => void;
      setTextColor: (...args: number[]) => void;
      setFontSize: (size: number) => void;
      text: (
        text: string,
        x: number,
        y: number,
        options?: { align?: 'center' | 'left' | 'right' },
      ) => void;
    },
    weeks: ProgramWeek[],
    config: {
      x: number;
      y: number;
      width: number;
      height: number;
      palette: {
        blue: readonly [number, number, number];
        aqua: readonly [number, number, number];
        gold: readonly [number, number, number];
        slate: readonly [number, number, number];
        line: readonly [number, number, number];
      };
    },
  ): void {
    const weekTotals = weeks.map((week) =>
      week.days.reduce((sum, day) => sum + day.exercises.length, 0),
    );
    const maxValue = Math.max(...weekTotals, 1);
    const slotWidth = config.width / Math.max(weekTotals.length, 1);

    pdf.setDrawColor(...config.palette.line);
    pdf.line(config.x, config.y + config.height, config.x + config.width, config.y + config.height);

    weekTotals.forEach((value, index) => {
      const barHeight = (value / maxValue) * (config.height - 6);
      const barWidth = Math.max(slotWidth - 7, 10);
      const x = config.x + index * slotWidth + 3;
      const y = config.y + config.height - barHeight;
      const isAccent = index % 2 === 0;

      pdf.setFillColor(...(isAccent ? config.palette.aqua : config.palette.gold));
      pdf.roundedRect(x, y, barWidth, barHeight, 1.4, 1.4, 'F');
      pdf.setTextColor(...config.palette.slate);
      pdf.setFontSize(8);
      pdf.text(`W${weeks[index]?.week ?? index + 1}`, x + barWidth / 2, config.y + config.height + 5, {
        align: 'center',
      });
      pdf.setTextColor(...config.palette.blue);
      pdf.text(String(value), x + barWidth / 2, y - 1.5, { align: 'center' });
    });
  }

  private generatedAtLabel(): string {
    return new Intl.DateTimeFormat('sr-RS', {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date());
  }

  private fileSafeName(value: string): string {
    return value
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'training-program';
  }

  private programExerciseSets(exercise: ProgramExercise): Array<{ reps: number; load_kg: number }> {
    const reps = Number(exercise.reps.match(/\d+/)?.[0] ?? 0);
    return Array.from({ length: exercise.sets }, () => ({
      reps,
      load_kg: 0,
    }));
  }

  private todayLabel(): string {
    return new Date().toISOString().slice(0, 10);
  }

  private dayKey(programId: string, weekNumber: number, dayNumber: number): string {
    return `${programId}-${weekNumber}-${dayNumber}`;
  }

  private async loadPdfTools(): Promise<{
    jsPDF: typeof import('jspdf').default;
    autoTable: typeof import('jspdf-autotable').default;
  }> {
    const [{ default: jsPDF }, { default: autoTable }] = await Promise.all([
      import('jspdf'),
      import('jspdf-autotable'),
    ]);

    return { jsPDF, autoTable };
  }
}
