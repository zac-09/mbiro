import { StrictMode, useEffect, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";
import { DEFAULT_SETTINGS, cleanSite, cleanType, type Settings } from "../../capture";
import { loadSettings, onSettingsChanged, saveSettings } from "../../settings";
import { strings } from "../../strings";
import { Switch } from "../../Switch";
import "../../ui.css";

function Rules() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  /** The file-type list as text while it is being edited; null when it is not. */
  const [typesDraft, setTypesDraft] = useState<string | null>(null);
  /** The site being typed; null while the add row is closed. */
  const [siteDraft, setSiteDraft] = useState<string | null>(null);
  const [siteError, setSiteError] = useState(false);

  useEffect(() => {
    document.title = strings.rules.title;
    void loadSettings().then(setSettings);
    // The popup can change the same rules while this page is open.
    return onSettingsChanged(setSettings);
  }, []);

  const change = (next: Settings) => {
    setSettings(next);
    void saveSettings(next);
  };

  const finishTypes = () => {
    if (typesDraft !== null) {
      const types = typesDraft.split(/[,\s]+/).flatMap((entry) => cleanType(entry) ?? []);
      change({ ...settings, fileTypes: [...new Set(types)] });
    }
    setTypesDraft(null);
  };

  const addSite = (event: FormEvent) => {
    event.preventDefault();
    const site = cleanSite(siteDraft ?? "");
    if (!site) {
      setSiteError(true);
      return;
    }
    change({ ...settings, excludedSites: [...new Set([...settings.excludedSites, site])] });
    setSiteDraft(null);
    setSiteError(false);
  };

  return (
    <main className="rules">
      <header>
        <h1>{strings.rules.heading}</h1>
        <span className="product">{strings.rules.product}</span>
      </header>

      <div className="rule">
        <div className="text">
          <span className="name">{strings.rules.auto}</span>
          <span className="help">{strings.rules.autoHelp}</span>
        </div>
        <Switch
          label={strings.rules.auto}
          on={settings.autoCatch}
          onChange={(autoCatch) => change({ ...settings, autoCatch })}
        />
      </div>

      <div className="rule">
        <div className="text">
          <label htmlFor="minsize" className="name">
            {strings.rules.larger}
          </label>
          <span className="help">{strings.rules.largerHelp}</span>
        </div>
        <div className="size">
          <input
            id="minsize"
            className="field"
            type="number"
            min={0}
            step={1}
            value={settings.minSizeMb}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value) && value >= 0) change({ ...settings, minSizeMb: value });
            }}
          />
          <span className="unit">{strings.rules.mb}</span>
        </div>
      </div>

      <section className="block">
        <div className="head">
          <span className="name">{strings.rules.types}</span>
          {typesDraft === null ? (
            <button
              type="button"
              className="secondary"
              onClick={() => setTypesDraft(settings.fileTypes.join(", "))}
            >
              {strings.rules.editList}
            </button>
          ) : (
            <button type="button" className="secondary" onClick={finishTypes}>
              {strings.rules.done}
            </button>
          )}
        </div>
        {typesDraft === null ? (
          <div className="chips">
            {settings.fileTypes.map((type) => (
              <span key={type} className="chip">
                {type.toUpperCase()}
              </span>
            ))}
          </div>
        ) : (
          <input
            className="field wide"
            autoFocus
            spellCheck={false}
            aria-label={strings.rules.typesLabel}
            value={typesDraft}
            onChange={(event) => setTypesDraft(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && finishTypes()}
          />
        )}
      </section>

      <section className="block">
        <div className="head">
          <span className="name">{strings.rules.sites}</span>
          {siteDraft === null && (
            <button type="button" className="secondary" onClick={() => setSiteDraft("")}>
              {strings.rules.addSite}
            </button>
          )}
        </div>
        {siteDraft !== null && (
          <form className="add-site" onSubmit={addSite}>
            <input
              className="field"
              autoFocus
              spellCheck={false}
              aria-label={strings.rules.siteLabel}
              placeholder={strings.rules.sitePlaceholder}
              value={siteDraft}
              onChange={(event) => {
                setSiteDraft(event.target.value);
                setSiteError(false);
              }}
            />
            <button type="submit" className="secondary">
              {strings.rules.add}
            </button>
          </form>
        )}
        {siteError && (
          <p className="error" role="alert">
            {strings.rules.siteInvalid}
          </p>
        )}
        {settings.excludedSites.length === 0 ? (
          <p className="help">{strings.rules.noSites}</p>
        ) : (
          <ul className="sites">
            {settings.excludedSites.map((site) => (
              <li key={site}>
                <span>{site}</span>
                <button
                  type="button"
                  className="secondary"
                  onClick={() =>
                    change({
                      ...settings,
                      excludedSites: settings.excludedSites.filter((entry) => entry !== site),
                    })
                  }
                >
                  {strings.rules.remove}
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <Rules />
  </StrictMode>,
);
