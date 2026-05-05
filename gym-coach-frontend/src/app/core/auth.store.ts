import { Injectable, computed, signal } from '@angular/core';

import { Claims } from './models';

const TOKEN_KEY = 'gym-coach-token';

@Injectable({ providedIn: 'root' })
export class AuthStore {
  readonly token = signal<string | null>(localStorage.getItem(TOKEN_KEY));
  readonly claims = signal<Claims | null>(this.decodeToken(this.token()));

  readonly isAuthenticated = computed(() => !!this.token());
  readonly role = computed(() => this.claims()?.role ?? null);
  readonly userId = computed(() => this.claims()?.sub ?? null);

  setToken(token: string): void {
    localStorage.setItem(TOKEN_KEY, token);
    this.token.set(token);
    this.claims.set(this.decodeToken(token));
  }

  clear(): void {
    localStorage.removeItem(TOKEN_KEY);
    this.token.set(null);
    this.claims.set(null);
  }

  private decodeToken(token: string | null): Claims | null {
    if (!token) {
      return null;
    }

    try {
      const payload = token.split('.')[1];
      const normalized = payload.replace(/-/g, '+').replace(/_/g, '/');
      return JSON.parse(atob(normalized)) as Claims;
    } catch {
      return null;
    }
  }
}
