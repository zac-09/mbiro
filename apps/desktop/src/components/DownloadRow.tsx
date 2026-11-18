import { api, type Job } from "../api";
import { clock, formatSize, percent, speedMb, timeLeft } from "../format";
import { run } from "../store";
import { strings } from "../strings";
import { SegmentBar } from "./SegmentBar";

const rowButton =
  "min-h-[44px] flex-[0_0_116px] rounded-[2px] border bg-transparent text-[12px] font-medium " +
  "tracking-[0.08em] text-ink cursor-pointer hover:border-muted";

/** The left detail line: sizes, then what kind of job it is. A failure shows its reason instead. */
function meta(job: Job): string {
  // A failure is two sentences; the row has room for the first.
  if (job.state === "failed") {
    const first = (job.error ?? strings.row.failed).split(". ")[0] ?? strings.row.failed;
    return first.replace(/\.$/, "").toUpperCase();
  }
  const size =
    job.state === "done" || job.totalBytes <= 0
      ? formatSize(job.state === "done" ? job.totalBytes : job.completedBytes)
      : strings.row.progress(formatSize(job.completedBytes), formatSize(job.totalBytes));
  if (job.kind === "video") return `${size} · ${strings.row.video}`;
  if (job.state !== "active" || job.connections <= 0) return size;
  const connections = `${size} · ${strings.row.connections(job.connections)}`;
  return job.resumeDoubtful ? `${connections} · ${strings.row.mayNotResume}` : connections;
}

/** The line under the percentage. */
function sub(job: Job): string {
  switch (job.state) {
    case "active":
      return strings.row.speed(speedMb(job.speedBps), timeLeft(job));
    case "queued":
      return strings.row.queued;
    case "paused":
      return strings.row.paused;
    case "done":
      return strings.row.done(clock(job.finishedAt ?? job.createdAt));
    case "failed":
      return strings.row.failed;
  }
}

/** What the row's button does in each state. */
export function primaryAction(job: Job): { label: string; act: () => void } {
  switch (job.state) {
    case "active":
    case "queued":
      return { label: strings.row.pause, act: () => run(api.pause(job.id)) };
    case "paused":
      return { label: strings.row.resume, act: () => run(api.resume(job.id)) };
    case "done":
      return { label: strings.row.showFile, act: () => run(api.showFile(job.id)) };
    case "failed":
      return { label: strings.row.retry, act: () => run(api.retry(job.id)) };
  }
}

interface Props {
  job: Job;
  selected: boolean;
  /** True while this row is asking whether to remove it. */
  confirming: boolean;
  onSelect: () => void;
  onAskRemove: () => void;
  onCancelRemove: () => void;
  last: boolean;
}

export function DownloadRow(props: Props) {
  const { job, selected, confirming } = props;
  const pct = percent(job);
  const failed = job.state === "failed";
  const dim = job.state === "paused" || job.state === "queued" || job.state === "done";
  const action = primaryAction(job);

  return (
    <li
      id={`job-${job.id}`}
      role="option"
      aria-selected={selected}
      aria-label={`${job.name}, ${strings.row.stateLabel[job.state]}, ${pct}%`}
      onMouseDown={props.onSelect}
      className={`group flex min-h-[78px] flex-wrap items-center gap-x-6 gap-y-[10px] py-[10px] ${props.last ? "" : "border-b border-rule"} ${selected ? "shadow-[inset_2px_0_0_var(--color-accent)] pl-3" : ""}`}
    >
      <div className="flex min-w-0 flex-[1_1_260px] flex-col gap-[5px]">
        <div className="text-[13.5px] font-medium [overflow-wrap:anywhere]">{job.name}</div>
        <div
          title={failed ? (job.error ?? undefined) : undefined}
          className={`text-[11.5px] tracking-[0.06em] ${failed ? "text-danger-ink" : "text-muted"}`}
        >
          {meta(job)}
        </div>
      </div>

      <div className="min-w-0 flex-[0_1_382px]">
        <SegmentBar percent={pct} state={job.state} blocks={32} height="h-[26px]" capWidth />
      </div>

      <div className="flex flex-[0_0_170px] flex-col items-end gap-[2px]">
        <span
          className={`font-display text-[36px] leading-none font-semibold ${failed ? "text-danger-ink" : dim ? "text-muted" : "text-ink"}`}
        >
          {pct}%
        </span>
        <span
          className={`text-[11.5px] tracking-[0.04em] ${failed ? "text-danger-ink" : "text-muted"}`}
        >
          {sub(job)}
        </span>
      </div>

      {/* Two fixed slots, so showing REMOVE never moves or pushes anything. */}
      <div className="flex flex-[0_0_242px] justify-end gap-[10px]">
        {confirming ? (
          <>
            <button
              type="button"
              autoFocus
              className={`${rowButton} border-danger-ink`}
              onClick={() => run(api.remove(job.id))}
            >
              {strings.row.confirmRemove}
            </button>
            <button
              type="button"
              className={`${rowButton} border-border`}
              onClick={props.onCancelRemove}
            >
              {strings.row.keep}
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className={`${rowButton} border-border ${selected ? "" : "invisible group-hover:visible"}`}
              onClick={props.onAskRemove}
            >
              {strings.row.remove}
            </button>
            <button
              type="button"
              className={`${rowButton} ${failed ? "border-danger-ink" : "border-border"}`}
              onClick={action.act}
            >
              {action.label}
            </button>
          </>
        )}
      </div>
    </li>
  );
}
