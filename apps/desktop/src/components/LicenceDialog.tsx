import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { api, type LicenceStatus } from "../api";
import { strings } from "../strings";
import { fieldLabel, primaryButton, secondaryButton } from "../ui";

const t = strings.licence;

interface Props {
  onClose: () => void;
  /** A key brought by an activation link: it is activated without a click. */
  linkedKey?: string;
  /** Called with the new status once a key has been accepted. */
  onActivated: (status: LicenceStatus) => void;
}

/** Where a licence key is pasted. The core checks it and activates it. */
export function LicenceDialog({ onClose, linkedKey, onActivated }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [key, setKey] = useState(linkedKey ?? "");
  const [error, setError] = useState<string | null>(null);
  // Activation asks the licence server, which can take a few seconds.
  const [busy, setBusy] = useState(Boolean(linkedKey));

  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  const send = useCallback(
    (entered: string) => {
      api
        .enterLicence(entered)
        .then((status) => {
          onActivated(status);
          onClose();
        })
        .catch((reason: unknown) => {
          setError(String(reason));
          setBusy(false);
        });
    },
    [onActivated, onClose],
  );

  // A linked key is sent once, as the dialog opens. If that fails the key
  // stays in the field with the reason, to try again or cancel.
  const sent = useRef(false);
  useEffect(() => {
    if (!linkedKey || sent.current) return;
    sent.current = true;
    send(linkedKey);
  }, [linkedKey, send]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!key.trim() || busy) return;
    setError(null);
    setBusy(true);
    send(key);
  };

  return (
    <dialog
      ref={dialog}
      onClose={onClose}
      aria-labelledby="licence-heading"
      className="m-auto w-[560px] max-w-[calc(100vw-32px)] rounded-[2px] border border-rule-strong bg-bg p-0 text-ink backdrop:bg-black/70"
    >
      <form onSubmit={submit} className="box-border flex flex-col gap-5 p-7">
        <h1
          id="licence-heading"
          className="animate-drop m-0 font-display text-[40px] leading-[0.9] font-bold tracking-[0.06em] text-accent"
        >
          {t.heading}
        </h1>
        <p className="m-0 text-[12.5px] leading-[1.55] text-soft">{t.body}</p>
        <div className="flex flex-col gap-2">
          <label htmlFor="licence-key" className={fieldLabel}>
            {t.keyLabel}
          </label>
          <textarea
            id="licence-key"
            required
            autoFocus
            rows={4}
            spellCheck={false}
            value={key}
            onChange={(event) => setKey(event.target.value)}
            className="box-border resize-none rounded-[2px] border border-border bg-field p-3 font-mono text-[12.5px] leading-[1.5] break-all text-ink select-text"
          />
        </div>
        {error && (
          <p role="alert" className="m-0 text-[12.5px] text-danger-ink">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-[10px] border-t border-rule-strong pt-4">
          <button
            type="button"
            className={`${secondaryButton} min-h-[48px] px-[18px]`}
            onClick={onClose}
          >
            {t.cancel}
          </button>
          <button type="submit" disabled={busy} className={`${primaryButton} px-7`}>
            {busy ? t.activating : t.activate}
          </button>
        </div>
      </form>
    </dialog>
  );
}
