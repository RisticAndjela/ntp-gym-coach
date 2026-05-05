import { CommonModule } from '@angular/common';
import { Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router } from '@angular/router';

import { ApiService } from '../core/api.service';
import { AuthStore } from '../core/auth.store';
import { UserRole } from '../core/models';

@Component({
  selector: 'app-auth-page',
  imports: [CommonModule, ReactiveFormsModule],
  templateUrl: './auth-page.component.html',
  styleUrl: './auth-page.component.scss',
})
export class AuthPageComponent {
  private readonly fb = inject(FormBuilder);
  private readonly api = inject(ApiService);
  private readonly authStore = inject(AuthStore);
  private readonly router = inject(Router);

  readonly mode = signal<'login' | 'register'>('login');
  readonly loading = signal(false);
  readonly error = signal('');

  readonly loginForm = this.fb.nonNullable.group({
    email: ['client@gymcoach.rs', [Validators.required, Validators.email]],
    password: ['client123', [Validators.required]],
  });

  readonly registerForm = this.fb.nonNullable.group({
    full_name: [''],
    email: [''],
    password: [''],
    role: ['CLIENT' as UserRole],
  });

  setMode(mode: 'login' | 'register'): void {
    this.error.set('');
    this.mode.set(mode);
  }

  submit(): void {
    this.error.set('');
    this.loading.set(true);

    const request =
      this.mode() === 'login'
        ? this.api.login(this.loginForm.getRawValue())
        : this.api.register(this.registerForm.getRawValue());

    request.subscribe({
      next: (session) => {
        this.authStore.setToken(session.token);
        void this.router.navigateByUrl('/app/dashboard');
      },
      error: (err) => {
        this.error.set(err?.error?.error ?? 'Request failed. Check if the gateway is running.');
        this.loading.set(false);
      },
      complete: () => this.loading.set(false),
    });
  }
}
