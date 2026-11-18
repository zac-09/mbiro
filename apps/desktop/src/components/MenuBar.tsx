import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { COPIED_LINK_EVENT, api } from "../api";
import { percent, speedMb, timeLeft } from "../format";
import { connect, run, useStore } from "../store";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";
import { SegmentBar } from "./SegmentBar";

/** How many running downloads fit in the popover. */
const SHOWN = 3;

/** The menu bar popover: total speed, what is running, and two actions. */
export function MenuBar() {
  const jobs = useStore((state) => state.jobs);
  useEffect(connect, []);

  const [copied, setCopied] = useState<string | null>(null);
  useEffect(() => {
    void api.copiedLink().then(setCopied);
    const unlisten = listen<string | null>(COPIED_LINK_EVENT, (event) => setCopied(event.payload));
    return () => void unlisten.then((stop) => stop());
  }, []);

  const active = jobs.filter((job) => job.state === "active");
  const paused = jobs.filter((job) => job.state === "paused").length;
  const failed = jobs.filter((job) => job.state === "failed").length;
  const totalSpeed = active.reduce((sum, job) => sum + job.speedBps, 0);

  return (
    <div className="box-border flex h-screen flex-col gap-4 p-[18px]">
      <header className="flex items-end justify-between border-b border-rule-strong pb-3">
        <span className="font-display text-[20px] font-bold tracking-[0.16em] text-accent">
          {strings.wordmark}
        </span>
        <div className="animate-drop flex items-baseline gap-2">
          <span className="font-display text-[58px] leading-[0.86] font-bold text-accent">
            {speedMb(totalSpeed)}
          </span>
          <span className="font-display text-[18px] font-semibold tracking-[0.08em] text-muted">
            {strings.speedUnit}
          </span>
        </div>
      </header>

      {copied && (
        <div className="flex flex-col gap-[10px] rounded-[2px] border border-accent p-3">
          <div className="flex flex-col gap-1">
            <span className={fieldLabel}>{strings.menuBar.linkCopied}</span>
            <span className="line-clamp-2 text-[12.5px] [overflow-wrap:anywhere]">
              {copied.replace(/^https?:\/\//, "")}
            </span>
          </div>
          <button
            type="button"
            className={`${primaryButton} min-h-[44px] text-[18px]`}
            onClick={() => run(api.useCopiedLink())}
          >
            {strings.menuBar.downloadLink}
          </button>
        </div>
      )}

      <div className="flex min-h-0 flex-col gap-[14px]">
        {active.slice(0, SHOWN).map((job) => {
          const pct = percent(job);
          return (
            <div key={job.id} className="flex flex-col gap-[7px]">
              <div className="truncate text-[12.5px] font-medium">{job.name}</div>
              <SegmentBar percent={pct} state={job.state} blocks={28} height="h-4" />
              <div className="text-[11.5px] tracking-[0.04em] text-muted">
                {pct}% · {strings.row.speed(speedMb(job.speedBps), timeLeft(job))}
              </div>
            </div>
          );
        })}
        <div className="text-[11.5px] tracking-[0.08em] text-muted">
          {strings.menuBar.summary(
            active.length,
            paused,
            failed,
            Math.max(0, active.length - SHOWN),
          )}
        </div>
      </div>

      <div className="mt-auto grid grid-cols-2 gap-[10px] border-t border-rule-strong pt-[14px]">
        <button
          type="button"
          className={`${secondaryButton} px-0`}
          onClick={() => run(api.pauseAll())}
        >
          {strings.pauseAll}
        </button>
        <button
          type="button"
          className={`${secondaryButton} px-0`}
          onClick={() => run(api.openMainWindow())}
        >
          {strings.menuBar.openWindow}
        </button>
      </div>
    </div>
  );
}
