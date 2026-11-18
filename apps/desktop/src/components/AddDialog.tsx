import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  TOOL_PROGRESS_EVENT,
  api,
  type AddDefaults,
  type Found,
  type Quality,
  type ToolProgress,
} from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";
import { PlaylistPicker } from "./PlaylistPicker";
import { QualityPicker } from "./QualityPicker";

type Kind = "file" | "video";

interface Props {
  initialUrl: string;
  onClose: () => void;
}

/** Shows a folder under the home folder as `~/…`, as the design does. */
function tilde(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

export function AddDialog({ initialUrl, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [url, setUrl] = useState(initialUrl);
  const [kind, setKind] = useState<Kind>("file");
  const [defaults, setDefaults] = useState<AddDefaults | null>(null);
  const [dir, setDir] = useState<string | null>(null);
  const [connections, setConnections] = useState(8);
  const [toolsReady, setToolsReady] = useState(true);
  const [fetching, setFetching] = useState<ToolProgress | "starting" | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** What yt-dlp says about the link: not asked yet, being asked, or its answer. */
  const [video, setVideo] = useState<{ link: string; info: Found } | null>(null);
  /** The playlist videos left unticked, by link. Everything starts ticked. */
  const [skipped, setSkipped] = useState<ReadonlySet<string>>(new Set());
  const [quality, setQuality] = useState<Quality>({ kind: "best" });
  /** A link yt-dlp could make nothing of, so it is not shown as still being read. */
  const [failedLink, setFailedLink] = useState<string | null>(null);

  useEffect(() => {
    dialog.current?.showModal();
    void api.addDefaults().then((loaded) => {
      setDefaults(loaded);
      setConnections(loaded.connections);
    });
    void api.toolsStatus().then((tools) => setToolsReady(tools.every((tool) => tool.installed)));
    const unlisten = listen<ToolProgress>(TOOL_PROGRESS_EVENT, (event) =>
      setFetching(event.payload),
    );
    return () => void unlisten.then((stop) => stop());
  }, []);

  // A video page is looked up once the link has stopped changing.
  const probeable = kind === "video" && toolsReady && /^https?:\/\/\S+\.\S+/i.test(url.trim());
  useEffect(() => {
    if (!probeable) return;
    const link = url.trim();
    if (video?.link === link || failedLink === link) return;
    let current = true;
    const timer = setTimeout(() => {
      setError(null);
      api
        .probeVideo(link)
        .then((info) => {
          if (!current) return;
          setVideo({ link, info });
          setSkipped(new Set());
          setQuality({ kind: "best" });
        })
        .catch((reason: unknown) => {
          if (!current) return;
          setFailedLink(link);
          setError(String(reason));
        });
    }, 600);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [url, probeable, video, failedLink]);
  // An answer only counts while the link it was for is still the one in the field.
  const link = url.trim();
  const found: Found | "reading" | null = !probeable
    ? null
    : video?.link === link
      ? video.info
      : failedLink === link
        ? null
        : "reading";

  const chooseFolder = async () => {
    const picked = await open({ directory: true, defaultPath: dir ?? defaults?.dir });
    if (typeof picked === "string") setDir(picked);
  };

  const fetchTools = () => {
    setError(null);
    setFetching("starting");
    api
      .fetchTools()
      .then(() => setToolsReady(true))
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setFetching(null));
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!link) return;
    setError(null);
    const info = found !== null && found !== "reading" ? found : null;
    const added =
      kind !== "video"
        ? api.add(link, dir, connections)
        : info?.kind === "playlist"
          ? api.addPlaylist(
              info.entries
                .filter((entry) => !skipped.has(entry.url))
                .map(({ url, title }) => ({ url, title })),
              dir,
              quality,
            )
          : api.addVideo(link, dir, info ? quality : null, info?.title ?? null);
    added.then(onClose).catch((reason: unknown) => setError(String(reason)));
  };

  // Two joined buttons, as the design's on/off switch: the chosen side is filled.
  const side = (value: Kind, name: string) => (
    <button
      type="button"
      aria-pressed={kind === value}
      onClick={() => setKind(value)}
      className={`min-h-[44px] cursor-pointer border px-4 text-[12px] font-medium tracking-[0.08em] first:rounded-l-[2px] last:rounded-r-[2px] ${
        kind === value
          ? "border-accent bg-accent text-bg"
          : "border-border bg-transparent text-muted"
      }`}
    >
      {name}
    </button>
  );

  const needsTools = kind === "video" && !toolsReady;
  const nothingTicked =
    found !== null &&
    found !== "reading" &&
    found.kind === "playlist" &&
    found.entries.every((entry) => skipped.has(entry.url));
  const fetchLabel =
    fetching === null
      ? strings.add.toolsFetch
      : fetching === "starting"
        ? strings.add.toolsFetching(null, "")
        : strings.add.toolsFetching(
            fetching.total ? Math.floor((fetching.received / fetching.total) * 100) : null,
            fetching.name,
          );

  return (
    <dialog
      ref={dialog}
      onClose={onClose}
      aria-labelledby="add-heading"
      className="m-auto max-h-[calc(100vh-32px)] w-[560px] max-w-[calc(100vw-32px)] overflow-y-auto rounded-[2px] border border-rule-strong bg-bg p-0 text-ink backdrop:bg-black/70"
    >
      <form onSubmit={submit} className="box-border flex min-h-[420px] flex-col gap-5 p-7">
        <h1
          id="add-heading"
          className="animate-drop m-0 font-display text-[40px] leading-[0.9] font-bold tracking-[0.06em] text-accent"
        >
          {strings.add.heading}
        </h1>

        <div className="flex flex-col gap-2">
          <label htmlFor="add-url" className={fieldLabel}>
            {strings.add.link}
          </label>
          <input
            id="add-url"
            type="url"
            required
            autoFocus
            spellCheck={false}
            value={url}
            placeholder={strings.add.linkPlaceholder}
            onChange={(event) => setUrl(event.target.value)}
            className="box-border min-h-[46px] rounded-[2px] border border-border bg-field px-3 text-[13px] text-ink"
          />
        </div>

        <div className="flex items-center gap-3" role="group" aria-label={strings.add.type}>
          <span className={`${fieldLabel} flex-auto`}>{strings.add.type}</span>
          <div className="flex">
            {side("file", strings.add.file)}
            {side("video", strings.add.video)}
          </div>
        </div>

        {kind === "file" && (
          <div className="flex items-center gap-3">
            <label htmlFor="add-connections" className={`${fieldLabel} flex-auto`}>
              {strings.add.connections}
            </label>
            <input
              id="add-connections"
              type="number"
              min={1}
              max={defaults?.maxConnections ?? 16}
              value={connections}
              onChange={(event) => setConnections(Number(event.target.value))}
              className="box-border min-h-[44px] w-[88px] rounded-[2px] border border-border bg-field px-3 text-[13px] text-ink"
            />
          </div>
        )}

        {found === "reading" && (
          <p className="m-0 border-y border-rule-strong py-3 text-[11.5px] tracking-[0.1em] text-muted">
            {strings.add.reading}
          </p>
        )}
        {found !== null && found !== "reading" && found.kind === "video" && (
          <QualityPicker info={found} picked={quality} onPick={setQuality} />
        )}
        {found !== null && found !== "reading" && found.kind === "playlist" && (
          <PlaylistPicker
            info={found}
            picked={quality}
            onPick={setQuality}
            skipped={skipped}
            onSkipped={setSkipped}
          />
        )}

        {needsTools && (
          <div className="flex flex-col gap-3 border-y border-rule-strong py-3">
            <p className="m-0 text-[12.5px] leading-[1.5] text-soft">{strings.add.toolsNeeded}</p>
            <button
              type="button"
              className={`${secondaryButton} self-start`}
              disabled={fetching !== null}
              onClick={fetchTools}
            >
              {fetchLabel}
            </button>
          </div>
        )}

        <div className="flex items-center gap-3">
          <div className="flex min-w-0 flex-auto flex-col gap-1">
            <span className={fieldLabel}>{strings.add.saveTo}</span>
            <span className="truncate text-[13px]">{tilde(dir ?? defaults?.dir ?? "")}</span>
          </div>
          <button type="button" className={secondaryButton} onClick={() => void chooseFolder()}>
            {strings.add.change}
          </button>
        </div>

        {error && (
          <p role="alert" className="m-0 text-[12.5px] text-danger-ink">
            {error}
          </p>
        )}

        <div className="mt-auto flex justify-end gap-[10px] border-t border-rule-strong pt-4">
          <button
            type="button"
            className={`${secondaryButton} min-h-[48px] px-[18px]`}
            onClick={onClose}
          >
            {strings.add.cancel}
          </button>
          <button
            type="submit"
            className={`${primaryButton} px-7`}
            disabled={needsTools || found === "reading" || nothingTicked}
          >
            {strings.add.download}
          </button>
        </div>
      </form>
    </dialog>
  );
}
