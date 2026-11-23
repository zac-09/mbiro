import { useEffect, useRef, useState } from "react";
import { api, type Acknowledgement } from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";

const t = strings.acknowledgements;

function Entry({ entry }: { entry: Acknowledgement }) {
  const [open, setOpen] = useState(false);
  return (
    <li className="flex flex-col gap-2 border-t border-rule py-[14px] first:border-t-0">
      <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1">
        <span className="font-display text-[21px] font-semibold tracking-[0.06em]">
          {entry.name.toUpperCase()}
          {entry.version && <span className="text-muted"> {entry.version}</span>}
        </span>
        <span className="text-[11.5px] tracking-[0.08em] text-soft">
          {entry.licence.toUpperCase()}
        </span>
      </div>
      <p className="m-0 text-[12.5px] leading-[1.55] text-soft">{entry.role}</p>
      <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[12px] leading-[1.5]">
        <dt className="text-muted">{t.source}</dt>
        <dd className="m-0 break-all text-ink select-text">{entry.source}</dd>
        {entry.licenceLink && (
          <>
            <dt className="text-muted">{t.licence}</dt>
            <dd className="m-0 break-all text-ink select-text">{entry.licenceLink}</dd>
          </>
        )}
      </dl>
      {entry.text && (
        <>
          <button
            type="button"
            className={`${secondaryButton} self-start`}
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            {open ? t.hideText : t.showText}
          </button>
          {open && (
            <pre
              tabIndex={0}
              className="m-0 max-h-[320px] overflow-auto rounded-[2px] border border-border bg-field p-3 font-mono text-[11.5px] leading-[1.5] whitespace-pre-wrap text-soft select-text"
            >
              {entry.text}
            </pre>
          )}
        </>
      )}
    </li>
  );
}

/** The open-source work the app uses, with each licence. Opened from Settings. */
export function AcknowledgementsDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [entries, setEntries] = useState<Acknowledgement[]>([]);

  useEffect(() => {
    dialog.current?.showModal();
    void api.acknowledgements().then(setEntries);
  }, []);

  const group = (shipped: boolean) => entries.filter((entry) => entry.shipped === shipped);

  return (
    <dialog
      ref={dialog}
      onClose={onClose}
      aria-labelledby="acknowledgements-heading"
      className="m-auto max-h-[calc(100vh-32px)] w-[720px] max-w-[calc(100vw-32px)] rounded-[2px] border border-rule-strong bg-bg p-0 text-ink backdrop:bg-black/70"
    >
      <div className="box-border flex flex-col gap-5 p-7">
        <h1
          id="acknowledgements-heading"
          className="animate-drop m-0 font-display text-[40px] leading-[0.9] font-bold tracking-[0.06em] text-accent"
        >
          {t.heading}
        </h1>
        <p className="m-0 text-[12.5px] leading-[1.55] text-soft">{t.body}</p>
        {[
          { title: t.shipped, list: group(true) },
          { title: t.fetched, list: group(false) },
        ].map(
          ({ title, list }) =>
            list.length > 0 && (
              <section key={title} className="flex flex-col border-t border-rule-strong pt-3">
                <h2 className={`${fieldLabel} m-0 font-normal`}>{title}</h2>
                <ul className="m-0 flex list-none flex-col p-0">
                  {list.map((entry) => (
                    <Entry key={entry.name} entry={entry} />
                  ))}
                </ul>
              </section>
            ),
        )}
        <div className="flex justify-end border-t border-rule-strong pt-4">
          <button type="button" className={`${primaryButton} px-7`} onClick={onClose}>
            {t.done}
          </button>
        </div>
      </div>
    </dialog>
  );
}
