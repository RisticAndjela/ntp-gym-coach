import { CommonModule } from '@angular/common';
import { HttpErrorResponse } from '@angular/common/http';
import { Component, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule } from '@angular/forms';
import { forkJoin } from 'rxjs';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { CoachClientLink, CoachMatch, UserProfile } from '../core/models';

@Component({
  selector: 'app-users-page',
  imports: [CommonModule, ReactiveFormsModule],
  templateUrl: './users-page.component.html',
  styleUrl: './users-page.component.scss',
})
export class UsersPageComponent {
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);
  private readonly fb = inject(FormBuilder);

  readonly profiles = signal<UserProfile[]>([]);
  readonly coaches = signal<UserProfile[]>([]);
  readonly matches = signal<CoachMatch[]>([]);
  readonly clientConnections = signal<CoachClientLink[]>([]);
  readonly coachConnections = signal<CoachClientLink[]>([]);
  readonly status = signal('');
  readonly error = signal('');

  readonly profileForm = this.fb.nonNullable.group({
    full_name: [''],
    bio: [''],
    goals: [''],
    offers: [''],
  });

  readonly currentProfile = computed(
    () => this.profiles().find((profile) => profile.id === this.authStore.userId()) ?? null,
  );

  constructor() {
    this.load();
  }

  load(): void {
    const userId = this.authStore.userId();
    if (!userId) {
      return;
    }

    forkJoin({
      profiles: this.api.getProfiles(),
      coaches: this.api.getCoaches(),
      matches: this.api.getCoachMatches(userId),
      clientConnections: this.api.getClientConnections(userId),
      coachConnections: this.api.getCoachConnections(userId),
    }).subscribe({
      next: (data) => {
        this.profiles.set(data.profiles);
        this.coaches.set(data.coaches);
        this.matches.set(data.matches.sort((a, b) => b.score - a.score));
        this.clientConnections.set(data.clientConnections);
        this.coachConnections.set(data.coachConnections);

        const profile = data.profiles.find((item) => item.id === userId);
        if (profile) {
          this.profileForm.patchValue({
            full_name: profile.full_name,
            bio: profile.bio,
            goals: profile.goals.join(', '),
            offers: profile.offers.join(', '),
          });
        }
      },
      error: () => this.error.set('Unable to load user workspace.'),
    });
  }

  saveProfile(): void {
    const profile = this.currentProfile();
    if (!profile) {
      return;
    }

    const value = this.profileForm.getRawValue();
    this.api
      .updateProfile(profile.id, {
        full_name: value.full_name,
        bio: value.bio,
        goals: value.goals.split(',').map((item) => item.trim()).filter(Boolean),
        offers: value.offers.split(',').map((item) => item.trim()).filter(Boolean),
      })
      .subscribe({
        next: () => {
          this.status.set('Profile updated successfully.');
          this.load();
        },
        error: () => this.error.set('Profile update failed.'),
      });
  }

  connectWithCoach(coachId: string): void {
    const clientId = this.authStore.userId();
    if (!clientId) {
      return;
    }

    this.status.set('');
    this.error.set('');

    this.api.createConnection({ coach_id: coachId, client_id: clientId }).subscribe({
      next: () => {
        this.status.set('Coach assigned to client.');
        this.error.set('');
        this.load();
      },
      error: (error: HttpErrorResponse) => {
        if (error.status === 409) {
          this.status.set('Coach is already connected to this client.');
          this.error.set('');
          this.load();
          return;
        }

        this.error.set('Connection could not be created.');
      },
    });
  }

  coachNameFor(link: CoachClientLink): string {
    return this.coaches().find((coach) => coach.id === link.coach_id)?.full_name ?? link.coach_id;
  }

  clientNameFor(link: CoachClientLink): string {
    return this.profiles().find((profile) => profile.id === link.client_id)?.full_name ?? link.client_id;
  }
}
