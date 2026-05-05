import { CommonModule } from '@angular/common';
import { Component, computed, inject } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { AuthStore } from '../core/auth.store';

@Component({
  selector: 'app-shell',
  imports: [CommonModule, RouterLink, RouterLinkActive, RouterOutlet],
  templateUrl: './shell.component.html',
  styleUrl: './shell.component.scss',
})
export class ShellComponent {
  private readonly authStore = inject(AuthStore);
  private readonly router = inject(Router);

  readonly claims = this.authStore.claims;
  readonly role = this.authStore.role;
  readonly navigation = [
    { label: 'Dashboard', path: '/app/dashboard' },
    { label: 'Users', path: '/app/users' },
    { label: 'Trainings', path: '/app/trainings' },
    { label: 'Programs', path: '/app/programs' },
    { label: 'Analytics', path: '/app/analytics' },
  ];

  readonly initials = computed(() => {
    const email = this.claims()?.email ?? 'gym';
    return email.slice(0, 2).toUpperCase();
  });

  logout(): void {
    this.authStore.clear();
    void this.router.navigateByUrl('/auth');
  }
}
