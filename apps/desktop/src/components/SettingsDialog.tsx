import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  TOOL_PROGRESS_EVENT,
  api,
  type BrowserStatus,
  type Category,
  type Settings,
  type ToolProgress,
  type ToolStatus,
} from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";
import { Switch } from "./Switch";

const t = strings.settings;
const field =
  "box-border min-h-[44px] rounded-[2px] border border-border bg-field px-3 text-[13px] text-ink";

/** Shows a folder under the home folder as `~/…`. */
const tilde = (path: string) => path.replace(/^\/Users\/[^/]+/, "~");

/** One setting: its name and a line of help on the left, its control on the right. */
function Row({ name, help, children }: { name: string; help?: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border-t border-rule py-[14px] [h2+&]:border-t-0">
      <div className="flex min-w-0 flex-[1_1_220px] flex-col gap-[6px]">
        <span className="font-display text-[21px] font-semibold tracking-[0.06em]">{name}</span>
        {help && <span className="text-[12px] leading-[1.5] text-muted">{help}</span>}
      </div>
      <div className="flex flex-none items-center gap-[10px]">{children}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col border-t border-rule-strong pt-3">
      <h2 className={`${fieldLabel} m-0 font-normal`}>{title}</h2>
      {children}
    </section>
  );
}

/** A number that is saved when the field is left or Enter is pressed, never mid-typing. */
function NumberField(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    const value = Number(draft);
    if (draft !== null && draft.trim() !== "" && Number.isFinite(value)) {
      props.onCommit(Math.min(props.max, Math.max(props.min, value)));
    }
    setDraft(null);
  };
  return (
    <input
      type="number"
      aria-label={props.label}
      min={props.min}
      max={props.max}
      step={props.step ?? 1}
      value={draft ?? String(props.value)}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => event.key === "Enter" && commit()}
      className={`${field} w-[88px] text-right`}
    />
  );
}

/** A folder with a button to change it and, once changed, one to go back to the default. */
function FolderControl(props: {
  dir: string | null;
  fallback: string;
  onChange: (dir: string | null) => void;
}) {
  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: props.dir ?? props.fallback });
    if (typeof picked === "string") props.onChange(picked);
  };
  return (
    <>
      <span className="max-w-[220px] truncate text-[13px]" title={props.dir ?? props.fallback}>
        {tilde(props.dir ?? props.fallback)}
      </span>
      <button type="button" className={secondaryButton} onClick={() => void choose()}>
        {t.change}
      </button>
      {props.dir !== null && (
        <button type="button" className={secondaryButton} onClick={() => props.onChange(null)}>
          {t.useDefault}
        </button>
      )}
    </>
  );
}

/** A category's file types as text, saved when the field is left. */
function TypesField({
  category,
  onCommit,
}: {
  category: Category;
  onCommit: (types: string[]) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft !== null) onCommit(draft.split(/[,\s]+/).filter(Boolean));
    setDraft(null);
  };
  return (
    <input
      aria-label={t.typesLabel(category.name)}
      spellCheck={false}
      value={draft ?? category.extensions.join(", ")}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => event.key === "Enter" && commit()}
      className={`${field} w-full`}
    />
  );
}

/** A status square and its label: accent when good, danger when not. */
function Status({ good, label }: { good: boolean; label: string }) {
  return (
    <span
      className={`flex items-center gap-2 text-[11.5px] tracking-[0.08em] ${good ? "text-ink" : "text-danger-ink"}`}
    >
      <span aria-hidden="true" className={`h-2 w-2 ${good ? "bg-accent" : "bg-danger"}`} />
      {label}
    </span>
  );
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [downloads, setDownloads] = useState("");
  const [browsers, setBrowsers] = useState<BrowserStatus[]>([]);
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const [fetching, setFetching] = useState<ToolProgress | "starting" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    dialog.current?.showModal();
    void api.getSettings().then(setSettings);
    void api.addDefaults().then((defaults) => setDownloads(defaults.dir));
    void api.browserStatus().then(setBrowsers);
    void api.toolsStatus().then(setTools);
    const unlisten = listen<ToolProgress>(TOOL_PROGRESS_EVENT, (event) =>
      setFetching(event.payload),
    );
    return () => void unlisten.then((stop) => stop());
  }, []);

  /** Saves a change at once; the core answers with the settings as it stored them. */
  const change = (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    setError(null);
    api
      .setSettings(next)
      .then((stored) => {
        setSettings(stored);
        void api.addDefaults().then((defaults) => setDownloads(defaults.dir));
      })
      .catch((reason: unknown) => setError(String(reason)));
  };

  const changeCategory = (index: number, patch: Partial<Category>) => {
    if (!settings) return;
    change({
      categories: settings.categories.map((category, at) =>
        at === index ? { ...category, ...patch } : category,
      ),
    });
  };

  const fetchTools = () => {
    setError(null);
    setFetching("starting");
    api
      .fetchTools()
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => {
        setFetching(null);
        void api.toolsStatus().then(setTools);
      });
  };

  const toolsReady = tools.length > 0 && tools.every((tool) => tool.installed);
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
      aria-labelledby="settings-heading"
      className="m-auto max-h-[calc(100vh-32px)] w-[720px] max-w-[calc(100vw-32px)] overflow-y-auto rounded-[2px] border border-rule-strong bg-bg p-0 text-ink backdrop:bg-black/70"
    >
      <div className="box-border flex flex-col gap-5 p-7">
        <h1
          id="settings-heading"
          className="animate-drop m-0 font-display text-[40px] leading-[0.9] font-bold tracking-[0.06em] text-accent"
        >
          {t.heading}
        </h1>

        {error && (
          <p role="alert" className="m-0 text-[12.5px] text-danger-ink">
            {error}
          </p>
        )}

        {settings && (
          <>
            <Section title={t.general}>
              <Row name={t.saveTo} help={t.saveToHelp}>
                <FolderControl
                  dir={settings.defaultDir}
                  fallback={downloads}
                  onChange={(defaultDir) => change({ defaultDir })}
                />
              </Row>
              <Row name={t.clipboard} help={t.clipboardHelp}>
                <Switch
                  label={t.clipboard}
                  on={settings.watchClipboard}
                  onChange={(watchClipboard) => change({ watchClipboard })}
                />
              </Row>
              <Row name={t.notify} help={t.notifyHelp}>
                <Switch
                  label={t.notify}
                  on={settings.notifyOnFinish}
                  onChange={(notifyOnFinish) => change({ notifyOnFinish })}
                />
              </Row>
            </Section>

            <Section title={t.downloads}>
              <Row name={t.atOnce} help={t.atOnceHelp}>
                <NumberField
                  label={t.atOnce}
                  value={settings.maxConcurrent}
                  min={1}
                  max={10}
                  onCommit={(maxConcurrent) => change({ maxConcurrent: Math.round(maxConcurrent) })}
                />
              </Row>
              <Row name={t.connections} help={t.connectionsHelp}>
                <NumberField
                  label={t.connections}
                  value={settings.defaultConnections}
                  min={1}
                  max={16}
                  onCommit={(value) => change({ defaultConnections: Math.round(value) })}
                />
              </Row>
              <Row name={t.speedLimit} help={t.speedLimitHelp}>
                <NumberField
                  label={t.speedLimit}
                  value={Number(((settings.speedLimitKbps ?? 0) / 1024).toFixed(1))}
                  min={0}
                  max={9000}
                  step={0.5}
                  onCommit={(mb) =>
                    change({ speedLimitKbps: mb > 0 ? Math.round(mb * 1024) : null })
                  }
                />
                <span className="text-[12px] tracking-[0.08em] text-muted">
                  {strings.speedUnit}
                </span>
              </Row>
            </Section>

            <Section title={t.categories}>
              <p className="m-0 pt-2 text-[12px] leading-[1.5] text-muted">{t.categoriesHelp}</p>
              {settings.categories.map((category, index) => (
                <div
                  key={category.name}
                  className="flex flex-col gap-3 border-t border-rule py-[14px] first:border-t-0"
                >
                  <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
                    <span className="font-display text-[21px] font-semibold tracking-[0.06em]">
                      {category.name.toUpperCase()}
                    </span>
                    <div className="flex items-center gap-[10px]">
                      <FolderControl
                        dir={category.dir}
                        fallback={settings.defaultDir ?? downloads}
                        onChange={(dir) => changeCategory(index, { dir })}
                      />
                    </div>
                  </div>
                  <TypesField
                    category={category}
                    onCommit={(extensions) => changeCategory(index, { extensions })}
                  />
                </div>
              ))}
            </Section>
          </>
        )}

        <Section title={t.browsers}>
          <p className="m-0 pt-2 text-[12px] leading-[1.5] text-muted">{t.browsersHelp}</p>
          {browsers.map((browser) => (
            <Row key={browser.name} name={browser.name.toUpperCase()}>
              <Status
                good={browser.installed && browser.connected}
                label={
                  !browser.installed ? t.notInstalled : browser.connected ? t.ready : t.notConnected
                }
              />
            </Row>
          ))}
        </Section>

        <Section title={t.tools}>
          <p className="m-0 pt-2 text-[12px] leading-[1.5] text-muted">{strings.add.toolsNeeded}</p>
          {tools.map((tool) => (
            <Row key={tool.name} name={tool.name.toUpperCase()} help={tool.version}>
              <Status good={tool.installed} label={tool.installed ? t.installed : t.missing} />
            </Row>
          ))}
          {!toolsReady && (
            <button
              type="button"
              className={`${secondaryButton} self-start`}
              disabled={fetching !== null}
              onClick={fetchTools}
            >
              {fetchLabel}
            </button>
          )}
        </Section>

        <div className="flex justify-end border-t border-rule-strong pt-4">
          <button type="button" className={`${primaryButton} px-7`} onClick={onClose}>
            {t.done}
          </button>
        </div>
      </div>
    </dialog>
  );
}
