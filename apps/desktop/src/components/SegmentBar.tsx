import type { JobState } from "../api";
import { filledBlocks } from "../format";

const FILL: Record<JobState, string> = {
  active: "bg-accent",
  queued: "bg-seg-paused",
  paused: "bg-seg-paused",
  done: "bg-accent-dim",
  failed: "bg-danger",
};

interface Props {
  percent: number;
  state: JobState;
  /** 32 in the main window, 28 in the menu bar. */
  blocks: number;
  /** Tailwind height class of one block. */
  height: string;
  /** Blocks stop growing at 10px in the main window. */
  capWidth?: boolean;
}

/** Progress as a row of square blocks. Decorative: the row states the same thing in text. */
export function SegmentBar({ percent, state, blocks, height, capWidth }: Props) {
  const filled = filledBlocks(percent, blocks);
  return (
    <div aria-hidden="true" className="flex min-w-0 gap-[2px]">
      {Array.from({ length: blocks }, (_, index) => {
        const on = index < filled;
        const live = state === "active" && index === filled - 1;
        return (
          <span
            key={index}
            className={`flex-1 ${capWidth ? "max-w-[10px]" : ""} ${height} ${on ? FILL[state] : "bg-seg-empty"} ${live ? "animate-blink" : ""}`}
          />
        );
      })}
    </div>
  );
}
