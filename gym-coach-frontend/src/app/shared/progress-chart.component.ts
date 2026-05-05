import { CommonModule } from '@angular/common';
import { Component, computed, input } from '@angular/core';

@Component({
  selector: 'app-progress-chart',
  imports: [CommonModule],
  templateUrl: './progress-chart.component.html',
  styleUrl: './progress-chart.component.scss',
})
export class ProgressChartComponent {
  readonly Math = Math;
  readonly values = input<number[]>([]);
  readonly labels = input<string[]>([]);

  readonly points = computed(() => {
    const values = this.values();
    if (!values.length) {
      return '';
    }

    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max - min || 1;

    return values
      .map((value, index) => {
        const x = (index / Math.max(values.length - 1, 1)) * 100;
        const y = 100 - ((value - min) / range) * 100;
        return `${x},${y}`;
      })
      .join(' ');
  });

  readonly stats = computed(() => {
    const values = this.values();
    if (!values.length) {
      return { min: 0, max: 0, latest: 0 };
    }

    return {
      min: Math.min(...values),
      max: Math.max(...values),
      latest: values[values.length - 1],
    };
  });
}
