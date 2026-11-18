import { useState } from "react";
import type { FormatRow, Preset, Quality, VideoInfo } from "../api";
import { duration, formatSize } from "../format";
import { strings } from "../strings";
import { fieldLabel } from "../ui";

const dot = (parts: (string | null)[]) => parts.filter(Boolean).join(" · ");
const size = (bytes: number | null) => (bytes ? strings.add.approx(formatSize(bytes)) : null);

function presetText(preset: Preset): { name: string; detail: string } {
  const ext = preset.ext.toUpperCase();
  switch (preset.quality.kind) {
    case "best":
      return {
        name: strings.add.best,
        detail: dot([
          preset.height ? strings.add.height(preset.height) : null,
          ext,
          size(preset.approxBytes),
        ]),
      };
    case "max":
      return {
        name: strings.add.height(preset.quality.height),
        detail: dot([ext, size(preset.approxBytes)]),
      };
    default:
      return { name: strings.add.audioOnly, detail: dot([ext, size(preset.approxBytes)]) };
  }
}

function formatText(row: FormatRow): { name: string; detail: string } {
  return {
    name: row.hasVideo && row.height ? strings.add.height(row.height) : strings.add.audioOnly,
    detail: dot([
      row.ext.toUpperCase(),
      row.hasVideo && row.fps ? strings.add.fps(row.fps) : null,
      row.hasVideo && !row.hasAudio ? strings.add.noSound : null,
      size(row.approxBytes),
    ]),
  };
}

interface OptionProps {
  text: { name: string; detail: string };
  on: boolean;
  onPick: () => void;
}

/** One line of a quality choice: the design's "selected option". */
export function QualityOption({ text, on, onPick }: OptionProps) {
  return (
    <label
      className={`flex min-h-[52px] cursor-pointer items-center gap-3 rounded-[2px] border px-[14px] transition-colors duration-200 ${
        on ? "border-accent bg-accent-wash" : "border-border bg-transparent"
      }`}
    >
      <input
        type="radio"
        name="quality"
        checked={on}
        onChange={onPick}
        className="m-0 h-4 w-4 accent-accent"
      />
      <span className="flex-auto font-display text-[21px] font-semibold tracking-[0.06em]">
        {text.name}
      </span>
      <span className="text-[11.5px] tracking-[0.06em] text-muted">{text.detail}</span>
    </label>
  );
}

interface Props {
  info: VideoInfo;
  picked: Quality;
  onPick: (quality: Quality) => void;
}

/** What yt-dlp found at the link, and the choice of quality, as in the design. */
export function QualityPicker({ info, picked, onPick }: Props) {
  const [all, setAll] = useState(false);
  const key = (quality: Quality) => JSON.stringify(quality);

  const option = (quality: Quality, text: { name: string; detail: string }) => (
    <QualityOption
      key={key(quality)}
      text={text}
      on={key(quality) === key(picked)}
      onPick={() => onPick(quality)}
    />
  );

  return (
    <>
      <div className="flex gap-[14px] border-y border-rule-strong py-3">
        {info.thumbnail ? (
          <img
            src={info.thumbnail}
            alt=""
            className="h-[72px] w-[128px] flex-none bg-raised object-cover"
          />
        ) : (
          <div className="flex h-[72px] w-[128px] flex-none items-center justify-center bg-raised text-[11px] tracking-[0.1em] text-muted">
            {strings.add.thumbnail}
          </div>
        )}
        <div className="flex min-w-0 flex-col justify-center gap-[6px]">
          <div className="line-clamp-2 text-[14px] font-medium [overflow-wrap:anywhere]">
            {info.title}
          </div>
          <div className="text-[11.5px] tracking-[0.06em] text-muted">
            {strings.add.videoMeta(
              info.durationSecs === null ? null : duration(info.durationSecs),
              info.host,
            )}
          </div>
        </div>
      </div>

      <fieldset className="m-0 flex flex-col gap-2 border-0 p-0">
        <legend className={`${fieldLabel} p-0 pb-2`}>{strings.add.quality}</legend>
        {info.presets.map((preset) => option(preset.quality, presetText(preset)))}
        {all && (
          <div className="flex max-h-[232px] flex-col gap-2 overflow-y-auto pr-1">
            {info.formats.map((row) => option(row.quality, formatText(row)))}
          </div>
        )}
        {info.formats.length > 0 && (
          <button
            type="button"
            onClick={() => setAll(!all)}
            className="min-h-[44px] cursor-pointer self-start border-0 bg-transparent p-0 text-[12px] tracking-[0.08em] text-accent underline hover:brightness-125"
          >
            {all ? strings.add.showFewer : strings.add.showAll}
          </button>
        )}
      </fieldset>
    </>
  );
}
