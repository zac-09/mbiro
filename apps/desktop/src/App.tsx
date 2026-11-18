import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, type FormEvent } from "react";
import { formatBytes, formatSpeed, percent } from "./format";
import { strings } from "./strings";
import { SNAPSHOT_EVENT, type EngineHealth, type JobStatus } from "./types";

export function App() {
  const [health, setHealth] = useState<EngineHealth>({ status: "starting" });
  const [jobs, setJobs] = useState<JobStatus[]>([]);
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void invoke<EngineHealth>("engine_health").then((h) => {
      if (!cancelled) setHealth(h);
    });
    void invoke<JobStatus[]>("list_downloads").then((list) => {
      if (!cancelled) setJobs(list);
    });
    const unlisten = listen<JobStatus[]>(SNAPSHOT_EVENT, (event) => setJobs(event.payload));
    return () => {
      cancelled = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const run = (command: string, args: Record<string, unknown>) => {
    setError(null);
    invoke(command, args).catch((reason: unknown) => setError(String(reason)));
  };

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = url.trim();
    if (!trimmed) return;
    setError(null);
    invoke("add_download", { url: trimmed })
      .then(() => setUrl(""))
      .catch((reason: unknown) => setError(String(reason)));
  };

  const totalSpeed = jobs.reduce((sum, job) => sum + job.speedBps, 0);

  return (
    <main>
      <h1>{strings.title}</h1>

      {health.status === "starting" && <p className="notice">{strings.engineStarting}</p>}
      {health.status === "failed" && (
        <p className="error" role="alert">
          {strings.engineFailed} {health.reason}
        </p>
      )}

      <form onSubmit={onSubmit}>
        <label>
          {strings.urlLabel}
          <input
            type="url"
            value={url}
            placeholder={strings.urlPlaceholder}
            onChange={(event) => setUrl(event.target.value)}
            autoFocus
            spellCheck={false}
          />
        </label>
        <button type="submit" disabled={health.status !== "ready"}>
          {strings.add}
        </button>
      </form>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      <p className="total">
        {strings.totalSpeed}: {formatSpeed(totalSpeed)}
      </p>

      {jobs.length === 0 ? (
        <p className="notice">{strings.empty}</p>
      ) : (
        <ul>
          {jobs.map((job) => {
            const pct = percent(job.completedBytes, job.totalBytes);
            return (
              <li key={job.id}>
                <span className="name">{job.name}</span>
                <span className="actions">
                  {job.state === "active" || job.state === "queued" ? (
                    <button type="button" onClick={() => run("pause_download", { id: job.id })}>
                      {strings.pause}
                    </button>
                  ) : null}
                  {job.state === "paused" ? (
                    <button type="button" onClick={() => run("resume_download", { id: job.id })}>
                      {strings.resume}
                    </button>
                  ) : null}
                  <button type="button" onClick={() => run("remove_download", { id: job.id })}>
                    {strings.remove}
                  </button>
                </span>
                <progress max={100} value={pct} aria-label={`${job.name} ${pct}%`} />
                <span className={job.state === "failed" ? "detail error" : "detail"}>
                  {strings.state[job.state]} · {pct}% · {formatBytes(job.completedBytes)} /{" "}
                  {formatBytes(job.totalBytes)} · {formatSpeed(job.speedBps)} ·{" "}
                  {strings.connections(job.connections)}
                  {job.error ? ` · ${job.error}` : ""}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </main>
  );
}
