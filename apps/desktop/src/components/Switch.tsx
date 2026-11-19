import { strings } from "../strings";

interface Props {
  /** What the switch controls, for screen readers. */
  label: string;
  on: boolean;
  onChange: (on: boolean) => void;
}

/** The design's on/off switch: two joined buttons, the active side filled. */
export function Switch({ label, on, onChange }: Props) {
  const side = (active: boolean) =>
    `min-h-[44px] min-w-[48px] cursor-pointer border px-[10px] text-[12px] font-semibold tracking-[0.08em] ${
      active ? "border-accent bg-accent text-bg" : "border-border bg-transparent text-muted"
    }`;
  return (
    <div className="flex flex-none" role="group" aria-label={label}>
      <button type="button" aria-pressed={on} className={side(on)} onClick={() => onChange(true)}>
        {strings.on}
      </button>
      <button
        type="button"
        aria-pressed={!on}
        className={side(!on)}
        onClick={() => onChange(false)}
      >
        {strings.off}
      </button>
    </div>
  );
}
