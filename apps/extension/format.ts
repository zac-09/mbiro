const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

/** `1.8 GB`, `120 MB`, `4.2 MB`, `640 KB`: the same forms the app uses. */
export function formatSize(bytes: number): string {
  const value = Math.max(0, bytes);
  if (value >= 1000 * MB) return `${(value / GB).toFixed(1)} GB`;
  if (value >= 10 * MB) return `${Math.round(value / MB)} MB`;
  if (value >= MB) return `${(value / MB).toFixed(1)} MB`;
  return `${Math.round(value / KB)} KB`;
}

/** Local time of day, `10:42`, from seconds since 1970. */
export function clock(seconds: number): string {
  const date = new Date(seconds * 1000);
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** Rounded percent, 0 when the size is unknown. */
export function percent(completed: number, total: number): number {
  return total > 0 ? Math.min(100, Math.round((completed / total) * 100)) : 0;
}
