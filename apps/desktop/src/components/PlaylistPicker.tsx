import type { PlaylistInfo, Quality } from "../api";
import { duration } from "../format";
import { strings } from "../strings";
import { fieldLabel, secondaryButton } from "../ui";
import { QualityOption } from "./QualityPicker";

/** A playlist is not looked into video by video, so its qualities carry no sizes. */
const CHOICES: { quality: Quality; name: string }[] = [
  { quality: { kind: "best" }, name: strings.add.best },
  { quality: { kind: "max", height: 720 }, name: strings.add.height(720) },
  { quality: { kind: "max", height: 480 }, name: strings.add.height(480) },
  { quality: { kind: "audio" }, name: strings.add.audioOnly },
];

interface Props {
  info: PlaylistInfo;
  picked: Quality;
  onPick: (quality: Quality) => void;
  /** Links of the videos left unticked. */
  skipped: ReadonlySet<string>;
  onSkipped: (skipped: ReadonlySet<string>) => void;
}

/** A playlist: one quality for all of it, and a checklist of its videos. */
export function PlaylistPicker({ info, picked, onPick, skipped, onSkipped }: Props) {
  const key = (quality: Quality) => JSON.stringify(quality);
  const ticked = info.entries.filter((entry) => !skipped.has(entry.url)).length;

  const toggle = (url: string) => {
    const next = new Set(skipped);
    if (!next.delete(url)) next.add(url);
    onSkipped(next);
  };

  return (
    <>
      <div className="flex flex-col gap-[6px] border-y border-rule-strong py-3">
        <div className="line-clamp-2 text-[14px] font-medium [overflow-wrap:anywhere]">
          {info.title}
        </div>
        <div className="text-[11.5px] tracking-[0.06em] text-muted">
          {strings.add.playlistMeta(info.entries.length, info.host)}
        </div>
      </div>

      <fieldset className="m-0 grid grid-cols-2 gap-2 border-0 p-0">
        <legend className={`${fieldLabel} p-0 pb-2`}>{strings.add.quality}</legend>
        {CHOICES.map((choice) => (
          <QualityOption
            key={key(choice.quality)}
            text={{ name: choice.name, detail: "" }}
            on={key(choice.quality) === key(picked)}
            onPick={() => onPick(choice.quality)}
          />
        ))}
      </fieldset>

      <fieldset className="m-0 flex min-h-0 flex-col gap-2 border-0 p-0">
        <legend className="sr-only">{strings.add.videos(ticked, info.entries.length)}</legend>
        <div className="flex items-center gap-[10px]">
          <span className={`${fieldLabel} flex-auto`} aria-hidden="true">
            {strings.add.videos(ticked, info.entries.length)}
          </span>
          <button type="button" className={secondaryButton} onClick={() => onSkipped(new Set())}>
            {strings.add.all}
          </button>
          <button
            type="button"
            className={secondaryButton}
            onClick={() => onSkipped(new Set(info.entries.map((entry) => entry.url)))}
          >
            {strings.add.none}
          </button>
        </div>
        <div className="flex max-h-[180px] flex-col overflow-y-auto border-y border-rule">
          {info.entries.map((entry, index) => (
            <label
              key={`${index}-${entry.url}`}
              className="flex min-h-[44px] cursor-pointer items-center gap-3 border-b border-rule px-1 last:border-b-0"
            >
              <input
                type="checkbox"
                checked={!skipped.has(entry.url)}
                onChange={() => toggle(entry.url)}
                className="m-0 h-4 w-4 flex-none accent-accent"
              />
              <span className="min-w-0 flex-auto truncate text-[12.5px]">{entry.title}</span>
              {entry.durationSecs !== null && (
                <span className="flex-none text-[11.5px] tracking-[0.04em] text-muted">
                  {duration(entry.durationSecs)}
                </span>
              )}
            </label>
          ))}
        </div>
      </fieldset>
    </>
  );
}
