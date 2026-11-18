import type { Job } from "./api";

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

/** `1.8 GB`, `120 MB`, `4.2 MB`, `640 KB`: the forms the design shows. */
export function formatSize(bytes: number): string {
  const value = Math.max(0, bytes);
  if (value >= 1000 * MB) return `${(value / GB).toFixed(1)} GB`;
  if (value >= 10 * MB) return `${Math.round(value / MB)} MB`;
  if (value >= MB) return `${(value / MB).toFixed(1)} MB`;
  return `${Math.round(value / KB)} KB`;
}

/** Speed in MB/s with one decimal, without the unit. */
export function speedMb(bytesPerSecond: number): string {
  return (Math.max(0, bytesPerSecond) / MB).toFixed(1);
}

const two = (n: number) => String(n).padStart(2, "0");

/** `02:15` under an hour, `1:02:15` above. Null when it cannot be known. */
export function timeLeft(
  job: Pick<Job, "totalBytes" | "completedBytes" | "speedBps">,
): string | null {
  if (job.speedBps <= 0 || job.totalBytes <= 0) return null;
  const secs = Math.floor((job.totalBytes - job.completedBytes) / job.speedBps);
  if (secs < 0) return null;
  const [h, m, s] = [Math.floor(secs / 3600), Math.floor((secs % 3600) / 60), secs % 60];
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}

/** Local time of day, `10:42`, from seconds since 1970. */
export function clock(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** Rounded percent. 0 when the size is not known yet; 100 only when finished. */
export function percent(job: Pick<Job, "state" | "totalBytes" | "completedBytes">): number {
  if (job.state === "done") return 100;
  if (job.totalBytes <= 0) return 0;
  return Math.min(99, Math.round((job.completedBytes / job.totalBytes) * 100));
}

/** How many blocks of a segment bar are filled: `round(percent / 100 * blocks)`, at least 1. */
export function filledBlocks(pct: number, blocks: number): number {
  return Math.min(blocks, Math.max(1, Math.round((pct / 100) * blocks)));
}

/** A length as `42:18`, or `1:02:15` from an hour up. */
export function duration(seconds: number): string {
  const secs = Math.max(0, Math.round(seconds));
  const [h, m, s] = [Math.floor(secs / 3600), Math.floor((secs % 3600) / 60), secs % 60];
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}
