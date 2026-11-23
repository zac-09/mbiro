import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  OPEN_ADD_EVENT,
  UPDATE_EVENT,
  api,
  type Job,
  type LicenceStatus,
  type Settings,
  type UpdateStatus,
} from "./api";
import { AddDialog } from "./components/AddDialog";
import { DownloadRow, primaryAction } from "./components/DownloadRow";
import { LicenceDialog } from "./components/LicenceDialog";
import { Onboarding } from "./components/Onboarding";
import { SettingsDialog } from "./components/SettingsDialog";
import { speedMb } from "./format";
import { connect, run, useStore } from "./store";
import { strings } from "./strings";
import { primaryButton, secondaryButton } from "./ui";

type Filter = "all" | "active" | "done" | "failed";
const FILTERS: Filter[] = ["all", "active", "done", "failed"];

/** "Active" in the tabs means still in play: downloading, waiting or paused. */
function inFilter(job: Job, filter: Filter): boolean {
  if (filter === "all") return true;
  if (filter === "done" || filter === "failed") return job.state === filter;
  return job.state === "active" || job.state === "queued" || job.state === "paused";
}

/** The window: the first-launch steps once, and the download list ever after. */
export function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    void api.getSettings().then(setSettings);
  }, []);

  // Nothing is drawn until it is known which of the two to show.
  if (!settings) return null;
  if (!settings.onboarded) {
    return (
      <Onboarding
        settings={settings}
        onDone={(chosen) => {
          setSettings(chosen);
          run(api.setSettings(chosen));
        }}
      />
    );
  }
  return <Downloads />;
}

function Downloads() {
  const { jobs, health, error } = useStore();
  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  const [confirming, setConfirming] = useState<number | null>(null);
  /** The add dialog is open when this is a string: the link to start with. */
  const [adding, setAdding] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [licenceOpen, setLicenceOpen] = useState(false);
  const [licence, setLicence] = useState<LicenceStatus | null>(null);
  // A newer version of the app, once the core has found one.
  const [newVersion, setNewVersion] = useState<string | null>(null);

  useEffect(() => {
    const show = (status: UpdateStatus) => setNewVersion(status.available?.version ?? null);
    void api.updateStatus().then(show);
    const unlisten = listen<UpdateStatus>(UPDATE_EVENT, (event) => show(event.payload));
    return () => void unlisten.then((stop) => stop());
  }, []);

  // The trial can run out while the window is open; ask again every minute.
  useEffect(() => {
    const ask = () => void api.licenceStatus().then(setLicence);
    const first = setTimeout(ask, 0);
    const timer = setInterval(ask, 60_000);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [settingsOpen]);

  useEffect(connect, []);

  // The menu bar popover hands over a copied link the user chose to download.
  useEffect(() => {
    const unlisten = listen<string>(OPEN_ADD_EVENT, (event) => setAdding(event.payload));
    return () => void unlisten.then((stop) => stop());
  }, []);

  // Command-comma opens Settings, as on any Mac app.
  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.metaKey && event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Pasting a link anywhere outside a text field opens the add dialog with it.
  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if (event.target instanceof HTMLInputElement) return;
      const text = event.clipboardData?.getData("text").trim() ?? "";
      if (/^https?:\/\/\S+$/i.test(text)) setAdding(text);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, []);

  const counts = useMemo(
    () => ({
      all: jobs.length,
      active: jobs.filter((job) => inFilter(job, "active")).length,
      done: jobs.filter((job) => job.state === "done").length,
      failed: jobs.filter((job) => job.state === "failed").length,
      downloading: jobs.filter((job) => job.state === "active").length,
      paused: jobs.filter((job) => job.state === "paused").length,
    }),
    [jobs],
  );
  const totalSpeed = jobs.reduce((sum, job) => sum + job.speedBps, 0);

  // Newest first, as a download list reads.
  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return jobs
      .filter((job) => inFilter(job, filter))
      .filter((job) => !needle || job.name.toLowerCase().includes(needle))
      .slice()
      .reverse();
  }, [jobs, filter, search]);

  const onListKey = (event: KeyboardEvent) => {
    if (event.target instanceof HTMLButtonElement && event.key === " ") return;
    const index = visible.findIndex((job) => job.id === selected);
    const current = index >= 0 ? visible[index] : undefined;
    const move = (to: number) => {
      const next = visible[Math.max(0, Math.min(visible.length - 1, to))];
      if (!next) return;
      setSelected(next.id);
      setConfirming(null);
      document.getElementById(`job-${next.id}`)?.scrollIntoView({ block: "nearest" });
    };
    switch (event.key) {
      case "ArrowDown":
        move(index + 1);
        break;
      case "ArrowUp":
        move(index < 0 ? 0 : index - 1);
        break;
      case " ":
        // Space pauses or resumes; it does nothing to a finished or failed job.
        if (current && current.state !== "done" && current.state !== "failed") {
          primaryAction(current).act();
        }
        break;
      case "Delete":
      case "Backspace":
        if (current) setConfirming(current.id);
        break;
      case "Escape":
        setConfirming(null);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  return (
    <div className="box-border flex min-h-screen flex-col gap-[14px] px-9 pt-[22px] pb-6">
      <header className="flex flex-wrap items-center justify-between gap-x-6 gap-y-[10px]">
        <div className="font-display text-[26px] font-bold tracking-[0.16em] text-accent">
          {strings.wordmark}
        </div>
        <div className="flex flex-wrap items-center gap-1">
          {FILTERS.map((name) => (
            <button
              key={name}
              type="button"
              aria-pressed={filter === name}
              onClick={() => setFilter(name)}
              className={`min-h-[44px] cursor-pointer border-0 border-b-2 bg-transparent px-3 text-[12px] tracking-[0.08em] ${
                filter === name
                  ? "border-accent font-semibold text-ink"
                  : "border-transparent font-medium text-muted hover:text-ink"
              }`}
            >
              {strings.tabs[name]} {counts[name]}
            </button>
          ))}
          <div className="ml-4 box-border flex min-h-[44px] items-center gap-2 rounded-[2px] border border-border px-3 focus-within:border-accent">
            <svg
              aria-hidden="true"
              width="14"
              height="14"
              viewBox="0 0 16 16"
              fill="none"
              stroke="#A7ADB2"
              strokeWidth="1.6"
              strokeLinecap="square"
            >
              <circle cx="7" cy="7" r="4.25" />
              <path d="M10.5 10.5L13.5 13.5" />
            </svg>
            <input
              type="search"
              aria-label={strings.searchLabel}
              placeholder={strings.search}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className="w-[150px] border-0 bg-transparent text-[12px] tracking-[0.08em] text-ink outline-0 placeholder:text-muted"
            />
          </div>
        </div>
      </header>

      <section className="flex flex-wrap items-end justify-between gap-x-6 gap-y-4 border-y border-rule-strong pt-[14px] pb-[18px]">
        <div className="animate-drop flex items-baseline gap-4" aria-live="off">
          <span className="font-display text-[156px] leading-[0.86] font-bold tracking-[-0.01em] text-accent">
            {speedMb(totalSpeed)}
          </span>
          <span className="font-display text-[34px] font-semibold tracking-[0.08em] text-muted">
            {strings.speedUnit}
          </span>
        </div>
        <div className="flex flex-col items-end gap-[14px]">
          <span className="text-[12px] tracking-[0.08em] text-muted">
            {strings.counts(counts.downloading, counts.paused, counts.failed)}
          </span>
          <div className="flex flex-wrap justify-end gap-[10px]">
            <button type="button" className={secondaryButton} onClick={() => run(api.pauseAll())}>
              {strings.pauseAll}
            </button>
            <button type="button" className={secondaryButton} onClick={() => run(api.resumeAll())}>
              {strings.resumeAll}
            </button>
            <button type="button" className={`${primaryButton} px-6`} onClick={() => setAdding("")}>
              {strings.addDownload}
            </button>
          </div>
        </div>
      </section>

      {(error || health?.status === "failed") && (
        <p role="alert" className="m-0 text-[12.5px] text-danger-ink select-text">
          {error ?? strings.engineFailed}
        </p>
      )}

      {visible.length === 0 ? (
        <p className="m-0 py-6 text-[12.5px] text-muted">
          {jobs.length === 0 ? strings.empty : strings.emptyFiltered}
        </p>
      ) : (
        <ul
          role="listbox"
          tabIndex={0}
          aria-label={strings.listLabel}
          aria-activedescendant={selected === null ? undefined : `job-${selected}`}
          onKeyDown={onListKey}
          className="m-0 flex list-none flex-col p-0"
        >
          {visible.map((job, index) => (
            <DownloadRow
              key={job.id}
              job={job}
              selected={job.id === selected}
              confirming={job.id === confirming}
              onSelect={() => setSelected(job.id)}
              onAskRemove={() => setConfirming(job.id)}
              onCancelRemove={() => setConfirming(null)}
              last={index === visible.length - 1}
            />
          ))}
        </ul>
      )}

      <footer className="mt-auto flex flex-wrap items-center justify-between gap-[10px] border-t border-rule-strong pt-3">
        <span
          className={`text-[12px] tracking-[0.08em] ${licence?.state === "expired" ? "text-danger-ink" : "text-muted"}`}
        >
          {licence?.state === "trial" && strings.licence.trial(licence.daysLeft)}
          {licence?.state === "expired" && strings.licence.expired}
          {licence?.state === "licensed" && strings.licence.licensed(licence.email)}
        </span>
        <div className="flex flex-wrap gap-[10px]">
          {licence && licence.state !== "licensed" && (
            <button type="button" className={secondaryButton} onClick={() => setLicenceOpen(true)}>
              {strings.licence.enter}
            </button>
          )}
          {newVersion && (
            <button type="button" className={secondaryButton} onClick={() => setSettingsOpen(true)}>
              {strings.update.footer(newVersion)}
            </button>
          )}
          <button type="button" className={secondaryButton} onClick={() => setSettingsOpen(true)}>
            {strings.openSettings}
          </button>
        </div>
      </footer>

      {adding !== null && <AddDialog initialUrl={adding} onClose={() => setAdding(null)} />}
      {settingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} />}
      {licenceOpen && (
        <LicenceDialog onClose={() => setLicenceOpen(false)} onActivated={setLicence} />
      )}
    </div>
  );
}
