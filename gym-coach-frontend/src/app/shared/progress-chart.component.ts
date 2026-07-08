import { CommonModule } from '@angular/common';
import { Component, computed, input } from '@angular/core';

interface HistoryPoint {
  x: number;
  y: number;
  value: number;
  label: string;
  isCurrent: boolean;
}

interface AxisTick {
  y: number;
  value: string;
}

@Component({
  selector: 'app-progress-chart',
  imports: [CommonModule],
  templateUrl: './progress-chart.component.html',
  styleUrl: './progress-chart.component.scss',
})
export class ProgressChartComponent {
  readonly values = input<number[]>([]);
  readonly labels = input<string[]>([]);
  readonly recommendationValue = input<number | null>(null);
  readonly metricTitle = input('Progress metric');
  readonly metricUnit = input('');

  readonly model = computed(() => {
    const values = this.values();
    const labels = this.labels();
    const recommendation = this.recommendationValue();
    const unit = this.metricUnit();

    if (!values.length && recommendation == null) {
      return null;
    }

    const left = 12;
    const right = 92;
    const top = 12;
    const bottom = 82;
    const chartWidth = right - left;
    const chartHeight = bottom - top;
    const currentValue = values.at(-1) ?? 0;

    const domainMax = Math.max(0, ...values, recommendation ?? 0);
    const paddedMax = domainMax === 0 ? 10 : domainMax * 1.12;
    const toY = (value: number) => bottom - (value / paddedMax) * chartHeight;

    const historyPoints: HistoryPoint[] = values.map((value, index) => {
      const denominator = Math.max(values.length - 1, 1);
      const x = left + (index / denominator) * chartWidth;

      return {
        x,
        y: toY(value),
        value,
        label: labels[index] || `Session ${index + 1}`,
        isCurrent: index === values.length - 1,
      };
    });

    const recommendationPoint =
      recommendation != null
        ? {
            x: values.length > 1 ? right : left + chartWidth * 0.82,
            y: toY(recommendation),
            value: recommendation,
            label: 'Recommendation',
          }
        : null;

    const historyPath = historyPoints.length
      ? historyPoints.map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x} ${point.y}`).join(' ')
      : '';

    const projectionPath =
      recommendationPoint && historyPoints.length
        ? `M ${historyPoints[historyPoints.length - 1].x} ${historyPoints[historyPoints.length - 1].y} L ${recommendationPoint.x} ${recommendationPoint.y}`
        : '';

    const yTicks: AxisTick[] = [0, paddedMax * 0.5, paddedMax].map((value) => ({
      y: toY(value),
      value: this.formatValue(value, unit),
    }));

    const fullXLabels = historyPoints.map((point) => ({
      x: point.x,
      label: point.label,
      current: point.isCurrent,
    }));

    if (recommendationPoint) {
      fullXLabels.push({
        x: recommendationPoint.x,
        label: 'Target',
        current: false,
      });
    }

    const xLabels = fullXLabels.filter((label, index) => {
      if (index === 0 || index === fullXLabels.length - 1 || label.current) {
        return true;
      }

      if (fullXLabels.length <= 5) {
        return true;
      }

      const step = Math.ceil(fullXLabels.length / 4);
      return index % step === 0;
    });

    return {
      historyPoints,
      historyPath,
      projectionPath,
      recommendationPoint,
      yTicks,
      xLabels,
      currentValue: this.formatValue(currentValue, unit),
      recommendationValue: recommendation != null ? this.formatValue(recommendation, unit) : null,
      sessionsCount: values.length,
      peakValue: this.formatValue(Math.max(...values, recommendation ?? 0, 0), unit),
      baselineY: toY(0),
      left,
      right,
      top,
      bottom,
    };
  });

  private formatValue(value: number, unit: string): string {
    const formatted = value.toFixed(1);
    return unit ? `${formatted} ${unit}` : formatted;
  }
}
