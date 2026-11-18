import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { TOOL_PROGRESS_EVENT, api, type AddDefaults, type ToolProgress } from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";

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
    const link = url.trim();
    if (!link) return;
    setError(null);
    const added = kind === "video" ? api.addVideo(link, dir) : api.add(link, dir, connections);
    added.then(onClose).catch((reason: unknown) => setError(String(reason)));
  };

  const option = (value: Kind, name: string, detail: string) => {
    const on = kind === value;
    return (
      <label
        className={`flex min-h-[52px] cursor-pointer items-center gap-3 rounded-[2px] border px-[14px] transition-colors duration-200 ${
          on ? "border-accent bg-accent-wash" : "border-border bg-transparent"
        }`}
      >
        <input
          type="radio"
          name="kind"
          checked={on}
          onChange={() => setKind(value)}
          className="m-0 h-4 w-4 accent-accent"
        />
        <span className="flex-auto font-display text-[21px] font-semibold tracking-[0.06em]">
          {name}
        </span>
        <span className="text-[11.5px] tracking-[0.06em] text-muted">{detail}</span>
      </label>
    );
  };

  const needsTools = kind === "video" && !toolsReady;
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
      className="m-auto w-[560px] max-w-[calc(100vw-32px)] rounded-[2px] border border-rule-strong bg-bg p-0 text-ink backdrop:bg-black/70"
    >
      <form onSubmit={submit} className="box-border flex min-h-[560px] flex-col gap-5 p-7">
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

        <fieldset className="m-0 flex flex-col gap-2 border-0 p-0">
          <legend className={`${fieldLabel} p-0 pb-2`}>{strings.add.type}</legend>
          {option("file", strings.add.file, strings.add.fileDetail)}
          {option("video", strings.add.video, strings.add.videoDetail)}
        </fieldset>

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
          <button type="submit" className={`${primaryButton} px-7`} disabled={needsTools}>
            {strings.add.download}
          </button>
        </div>
      </form>
    </dialog>
  );
}
