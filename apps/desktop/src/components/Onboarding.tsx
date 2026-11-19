import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api, type BrowserStatus, type LicenceStatus, type Settings } from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";

const t = strings.onboarding;
const STEPS = 3;

/** Shows a folder under the home folder as `~/…`. */
const tilde = (path: string) => path.replace(/^\/Users\/[^/]+/, "~");

interface Props {
  settings: Settings;
  /** Called with the settings to keep once the last step is done. */
  onDone: (settings: Settings) => void;
}

/** The first-launch steps: where to save, the browser extension, and the trial. */
export function Onboarding({ settings, onDone }: Props) {
  const [step, setStep] = useState(1);
  const [dir, setDir] = useState<string | null>(settings.defaultDir);
  const [fallback, setFallback] = useState("");
  const [browsers, setBrowsers] = useState<BrowserStatus[]>([]);
  const [licence, setLicence] = useState<LicenceStatus | null>(null);

  useEffect(() => {
    void api.addDefaults().then((defaults) => setFallback(defaults.dir));
    void api.browserStatus().then(setBrowsers);
    void api.licenceStatus().then(setLicence);
  }, []);

  const choose = async () => {
    const picked = await open({ directory: true, defaultPath: dir ?? fallback });
    if (typeof picked === "string") setDir(picked);
  };

  const here = browsers.filter((browser) => browser.installed);
  const last = step === STEPS;
  const trialHeading =
    licence?.state === "licensed"
      ? t.licensedHeading
      : licence?.state === "expired"
        ? t.trialEnded
        : t.trialHeading(licence?.daysLeft ?? 14);
  const heading = step === 1 ? t.folderHeading : step === 2 ? t.browserHeading : trialHeading;
  const body = step === 1 ? t.folderBody : step === 2 ? t.browserBody : t.trialBody;

  return (
    <main className="box-border flex min-h-screen flex-col gap-6 px-9 pt-[22px] pb-6">
      <header className="flex items-center justify-between border-b border-rule-strong pb-[14px]">
        <div className="font-display text-[26px] font-bold tracking-[0.16em] text-accent">
          {strings.wordmark}
        </div>
        <span className="text-[12px] tracking-[0.08em] text-muted">{t.step(step, STEPS)}</span>
      </header>

      <div className="flex max-w-[640px] flex-col gap-5">
        <h1
          key={step}
          className="animate-drop m-0 font-display text-[46px] leading-[0.92] font-bold tracking-[0.02em]"
        >
          {heading}
        </h1>
        <p className="m-0 text-[12.5px] leading-[1.55] text-soft">{body}</p>

        {step === 1 ? (
          <div className="flex items-center gap-3 border-y border-rule-strong py-[14px]">
            <div className="flex min-w-0 flex-auto flex-col gap-1">
              <span className={fieldLabel}>{t.folderLabel}</span>
              <span className="truncate text-[13px]">{tilde(dir ?? fallback)}</span>
            </div>
            <button type="button" className={secondaryButton} onClick={() => void choose()}>
              {t.change}
            </button>
          </div>
        ) : step === 2 ? (
          <div className="flex flex-col border-t border-rule-strong pt-3">
            <span className={fieldLabel}>{t.browsersLabel}</span>
            {here.length === 0 ? (
              <p className="m-0 py-3 text-[12.5px] text-muted">{t.noBrowsers}</p>
            ) : (
              here.map((browser) => (
                <div
                  key={browser.name}
                  className="flex min-h-[52px] items-center justify-between gap-3 border-b border-rule last:border-b-0"
                >
                  <span className="font-display text-[21px] font-semibold tracking-[0.06em]">
                    {browser.name.toUpperCase()}
                  </span>
                  <span
                    className={`flex items-center gap-2 text-[11.5px] tracking-[0.08em] ${browser.connected ? "text-ink" : "text-danger-ink"}`}
                  >
                    <span
                      aria-hidden="true"
                      className={`h-2 w-2 ${browser.connected ? "bg-accent" : "bg-danger"}`}
                    />
                    {browser.connected ? t.ready : t.notConnected}
                  </span>
                </div>
              ))
            )}
          </div>
        ) : null}
      </div>

      <footer className="mt-auto flex justify-end gap-[10px] border-t border-rule-strong pt-4">
        {step > 1 && (
          <button
            type="button"
            className={`${secondaryButton} min-h-[48px] px-[18px]`}
            onClick={() => setStep(step - 1)}
          >
            {t.back}
          </button>
        )}
        <button
          type="button"
          className={`${primaryButton} px-7`}
          onClick={() =>
            last ? onDone({ ...settings, defaultDir: dir, onboarded: true }) : setStep(step + 1)
          }
        >
          {last ? t.finish : t.next}
        </button>
      </footer>
    </main>
  );
}
