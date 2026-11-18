import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { SNAPSHOT_EVENT, api, type EngineHealth, type Job } from "./api";

interface Store {
  jobs: Job[];
  health: EngineHealth | null;
  /** The last thing that went wrong, as a sentence for the user. */
  error: string | null;
  setError: (error: string | null) => void;
}

/** The window's copy of what the core knows. The core is the only writer of `jobs`. */
export const useStore = create<Store>((set) => ({
  jobs: [],
  health: null,
  error: null,
  setError: (error) => set({ error }),
}));

/** Runs a command and shows its failure, if any, in the window. */
export function run(command: Promise<unknown>): void {
  useStore.getState().setError(null);
  command.catch((reason: unknown) => useStore.getState().setError(String(reason)));
}

/** Starts following the core. Returns a function that stops. */
export function connect(): () => void {
  let stopped = false;
  void api.engineHealth().then((health) => !stopped && useStore.setState({ health }));
  void api.list().then((jobs) => !stopped && useStore.setState({ jobs }));
  const unlisten = listen<Job[]>(SNAPSHOT_EVENT, (event) =>
    useStore.setState({ jobs: event.payload }),
  );
  return () => {
    stopped = true;
    void unlisten.then((stop) => stop());
  };
}
