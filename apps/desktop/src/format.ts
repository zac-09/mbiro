const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** `1.8 GB`: one decimal from KB up, binary multiples. */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${UNITS[unit]}`;
}

/** Speed in MB/s with one decimal, as the design's readout shows it. */
export function formatSpeed(bytesPerSecond: number): string {
  return `${(Math.max(0, bytesPerSecond) / (1024 * 1024)).toFixed(1)} MB/s`;
}

/** Whole percent, 0 when the size is not known yet. */
export function percent(completed: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.floor((completed / total) * 100));
}
