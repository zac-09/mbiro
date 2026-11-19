import { DOWNLOAD_PAGE_URL, type JobSummary } from "@mbiro/protocol";
import { StrictMode, useCallback, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { browser } from "wxt/browser";
import { fetchStatus, openApp } from "../../app";
import { DEFAULT_SETTINGS, isExcluded, siteOf, type Settings } from "../../capture";
import { loadSentToday } from "../../sent";
import { loadSettings, saveSettings } from "../../settings";
import { strings } from "../../strings";
import { Switch } from "../../Switch";
import "../../ui.css";

/** What the app said about today's downloads; null while it cannot be reached. */
type Status = JobSummary[] | null | "checking";

function Popup() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [site, setSite] = useState<string | undefined>();
  const [status, setStatus] = useState<Status>("checking");

  const check = useCallback(async () => {
    const sent = await loadSentToday();
    const jobs = await fetchStatus(sent.map((entry) => entry.id));
    // Newest first, in the order they were sent.
    const order = new Map(sent.map((entry, index) => [entry.id, index]));
    setStatus(jobs && [...jobs].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0)));
  }, []);

  useEffect(() => {
    void loadSettings().then(setSettings);
    void browser.tabs
      .query({ active: true, currentWindow: true })
      .then(([tab]) => setSite(siteOf(tab?.url)));
    // Ask at once, then keep asking: live progress while the popup is open.
    const first = setTimeout(() => void check(), 0);
    const timer = setInterval(() => void check(), 1000);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [check]);

  const change = (next: Settings) => {
    setSettings(next);
    void saveSettings(next);
  };
  const open = () => void openApp().then(check);

  if (status === "checking") return <div className="popup offline" />;

  if (status === null) {
    return (
      <div className="popup offline">
        <header>
          <span className="wordmark">{strings.wordmark}</span>
          <span className="status down">{strings.popup.notFound}</span>
        </header>
        <div className="offline-heading drop">{strings.popup.offlineHeading}</div>
        <p className="offline-body">{strings.popup.offlineBody}</p>
        <div className="offline-actions">
          <button type="button" className="primary" onClick={open}>
            {strings.popup.openApp}
          </button>
          <div className={DOWNLOAD_PAGE_URL ? "pair" : undefined}>
            {DOWNLOAD_PAGE_URL && (
              <button
                type="button"
                className="secondary"
                onClick={() => void browser.tabs.create({ url: DOWNLOAD_PAGE_URL ?? undefined })}
              >
                {strings.popup.getApp}
              </button>
            )}
            <button
              type="button"
              className="secondary"
              style={{ width: "100%" }}
              onClick={() => void check()}
            >
              {strings.popup.tryAgain}
            </button>
          </div>
        </div>
      </div>
    );
  }

  const siteOn = !isExcluded(site, settings.excludedSites);
  const setSiteOn = (on: boolean) => {
    if (!site) return;
    const others = settings.excludedSites.filter((entry) => !isExcluded(site, [entry]));
    change({ ...settings, excludedSites: on ? others : [...others, site] });
  };

  return (
    <div className="popup connected">
      <header>
        <span className="wordmark">{strings.wordmark}</span>
        <span className="status">{strings.popup.connected}</span>
      </header>

      <div className="setting">
        <div className="text">
          <span className="name">{strings.popup.catchDownloads}</span>
          <span className="label">{strings.popup.allSites}</span>
        </div>
        <Switch
          label={strings.popup.catchDownloads}
          on={settings.autoCatch}
          onChange={(autoCatch) => change({ ...settings, autoCatch })}
        />
      </div>

      {site && (
        <div className="setting">
          <div className="text">
            <span className="name">{strings.popup.thisSite}</span>
            <span className="site">{site}</span>
          </div>
          <Switch label={`${strings.popup.thisSite}: ${site}`} on={siteOn} onChange={setSiteOn} />
        </div>
      )}

      <div className="sent">
        <span className="label">{strings.popup.sentToday}</span>
        {status.length === 0 ? (
          <ul>
            <li>
              <span className="detail">{strings.popup.nothingSent}</span>
            </li>
          </ul>
        ) : (
          <ul>
            {status.map((job) => (
              <li key={job.id}>
                <span className="file">{job.name}</span>
                <span className="detail">{strings.popup.job(job)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <footer className="pair">
        <button
          type="button"
          className="secondary"
          onClick={() => void browser.runtime.openOptionsPage()}
        >
          {strings.popup.settings}
        </button>
        <button type="button" className="primary" onClick={open}>
          {strings.popup.openApp}
        </button>
      </footer>
    </div>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <Popup />
  </StrictMode>,
);
