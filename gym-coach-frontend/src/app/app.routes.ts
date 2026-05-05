import { Routes } from '@angular/router';

import { authGuard } from './core/auth.guard';

export const routes: Routes = [
  {
    path: '',
    pathMatch: 'full',
    redirectTo: 'app/dashboard',
  },
  {
    path: 'auth',
    loadComponent: () =>
      import('./features/auth-page.component').then((m) => m.AuthPageComponent),
  },
  {
    path: 'app',
    canActivate: [authGuard],
    loadComponent: () => import('./features/shell.component').then((m) => m.ShellComponent),
    children: [
      {
        path: '',
        pathMatch: 'full',
        redirectTo: 'dashboard',
      },
      {
        path: 'dashboard',
        loadComponent: () =>
          import('./features/dashboard-page.component').then((m) => m.DashboardPageComponent),
      },
      {
        path: 'users',
        loadComponent: () =>
          import('./features/users-page.component').then((m) => m.UsersPageComponent),
      },
      {
        path: 'trainings',
        loadComponent: () =>
          import('./features/trainings-page.component').then((m) => m.TrainingsPageComponent),
      },
      {
        path: 'programs',
        loadComponent: () =>
          import('./features/programs-page.component').then((m) => m.ProgramsPageComponent),
      },
      {
        path: 'analytics',
        loadComponent: () =>
          import('./features/analytics-page.component').then((m) => m.AnalyticsPageComponent),
      },
    ],
  },
  {
    path: '**',
    redirectTo: 'app/dashboard',
  },
];
