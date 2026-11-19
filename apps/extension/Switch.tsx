import { strings } from "./strings";

interface Props {
  /** What the switch controls, for screen readers. */
  label: string;
  on: boolean;
  onChange: (on: boolean) => void;
}

/** The design's on/off switch: two joined buttons, the active side filled. */
export function Switch({ label, on, onChange }: Props) {
  return (
    <div className="switch" role="group" aria-label={label}>
      <button type="button" aria-pressed={on} onClick={() => onChange(true)}>
        {strings.popup.on}
      </button>
      <button type="button" aria-pressed={!on} onClick={() => onChange(false)}>
        {strings.popup.off}
      </button>
    </div>
  );
}
