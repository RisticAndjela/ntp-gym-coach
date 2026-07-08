import { CommonModule } from '@angular/common';
import { Component, computed, input } from '@angular/core';

interface HistoryPoint {
  key: string;
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

interface SanitizedValue {
  value: number;
  label: string;
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
    const labels = this.labels();
    const unit = this.metricUnit();
    const sanitizedValues: SanitizedValue[] = this.values()
      .map((value, index) => ({
        value: this.toFiniteNumber(value),
        label: labels[index] || `Session ${index + 1}`,
      }))
      .filter((entry): entry is SanitizedValue => entry.value !== null);
    const recommendation = this.toFiniteNumber(this.recommendationValue());

    if (!sanitizedValues.length && recommendation == null) {
      return null;
    }

    const left = 15;
    const right = 90;
    const top = 12;
    const bottom = 78;
    const chartWidth = right - left;
    const chartHeight = bottom - top;
    const numericValues = sanitizedValues.map((entry) => entry.value);
    const currentValue = numericValues.at(-1) ?? 0;
    const allValues = recommendation != null ? [...numericValues, recommendation] : [...numericValues];
    const rawMin = Math.min(...allValues);
    const rawMax = Math.max(...allValues);
    const range = Math.max(rawMax - rawMin, rawMax || 10, 10);
    const domainMin = rawMin - range * 0.1;
    const domainMax = rawMax + range * 0.2;
    const domainSpan = Math.max(domainMax - domainMin, 1);
    const toY = (value: number) => bottom - ((value - domainMin) / domainSpan) * chartHeight;

    const historyPoints: HistoryPoint[] = sanitizedValues.map((entry, index) => {
      const denominator = Math.max(sanitizedValues.length - 1, 1);
      const x = left + (index / denominator) * chartWidth;

      return {
        key: `${entry.label}-${index}`,
        x,
        y: toY(entry.value),
        value: entry.value,
        label: entry.label,
        isCurrent: index === sanitizedValues.length - 1,
      };
    });

    const recommendationPoint =
      recommendation != null
        ? {
            x: historyPoints.length > 1 ? right : left + chartWidth * 0.82,
            y: toY(recommendation),
            value: recommendation,
            label: 'Recommendation',
          }
        : null;

    const historyPath = historyPoints.length
      ? historyPoints.map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x} ${point.y}`).join(' ')
      : '';
    const historyAreaPath = historyPoints.length
      ? `${historyPath} L ${historyPoints[historyPoints.length - 1].x} ${bottom} L ${historyPoints[0].x} ${bottom} Z`
      : '';

    const projectionPath =
      recommendationPoint && historyPoints.length
        ? `M ${historyPoints[historyPoints.length - 1].x} ${historyPoints[historyPoints.length - 1].y} L ${recommendationPoint.x} ${recommendationPoint.y}`
        : '';

    const yTicks = this.buildTicks(domainMin, domainMax).map((value) => ({
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
      historyAreaPath,
      projectionPath,
      recommendationPoint,
      yTicks,
      xLabels,
      plotWidth: chartWidth,
      plotHeight: chartHeight,
      currentValue: this.formatValue(currentValue, unit),
      recommendationValue: recommendation != null ? this.formatValue(recommendation, unit) : null,
      sessionsCount: historyPoints.length,
      peakValue: this.formatValue(Math.max(...numericValues, recommendation ?? 0, 0), unit),
      currentPoint: historyPoints.at(-1) ?? null,
      baselineY: bottom,
      left,
      right,
      top,
      bottom,
    };
  });

  private formatValue(value: number, unit: string): string {
    if (!Number.isFinite(value)) {
      return unit ? `0.0 ${unit}` : '0.0';
    }

    const formatted = value.toFixed(1);
    return unit ? `${formatted} ${unit}` : formatted;
  }

  private buildTicks(minValue: number, maxValue: number): number[] {
    if (!Number.isFinite(minValue) || !Number.isFinite(maxValue) || minValue === maxValue) {
      return [0, 5, 10];
    }

    const span = maxValue - minValue;
    const rawTicks = [minValue, minValue + span / 3, minValue + (span * 2) / 3, maxValue];
    const roundedTicks = rawTicks.map((value) => Number(value.toFixed(1)));
    return [...new Set(roundedTicks)];
  }

  private toFiniteNumber(value: number | null | undefined): number | null {
    return typeof value === 'number' && Number.isFinite(value) ? value : null;
  }
}
