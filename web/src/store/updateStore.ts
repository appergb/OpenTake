import { create } from "zustand";
import {
  checkForAppUpdate,
  closeAppUpdate,
  installAppUpdate,
  type AppUpdateMetadata,
  type UpdateInstallEvent,
} from "../lib/api";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "upToDate"
  | "closing"
  | "downloading"
  | "installing"
  | "restarting"
  | "error";

export type UpdateCheckSource = "background" | "manual";

export interface UpdateBackend {
  check: () => Promise<AppUpdateMetadata | null>;
  close: (rid: number) => Promise<void>;
  install: (rid: number, onEvent: (event: UpdateInstallEvent) => void) => Promise<void>;
}

interface UpdateState {
  phase: UpdatePhase;
  dialogOpen: boolean;
  source: UpdateCheckSource | null;
  update: AppUpdateMetadata | null;
  progress: number | null;
  error: string | null;
  check: (source: UpdateCheckSource) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => Promise<void>;
}

const nativeBackend: UpdateBackend = {
  check: checkForAppUpdate,
  close: closeAppUpdate,
  install: installAppUpdate,
};

const BUSY_PHASES: ReadonlySet<UpdatePhase> = new Set([
  "checking",
  "closing",
  "downloading",
  "installing",
  "restarting",
]);

export function isUpdateInstallationBlocking(phase: UpdatePhase): boolean {
  return phase === "downloading" || phase === "installing" || phase === "restarting";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function createUpdateStore(backend: UpdateBackend = nativeBackend) {
  let closeInFlight: Promise<void> | null = null;

  return create<UpdateState>((set, get) => ({
    phase: "idle",
    dialogOpen: false,
    source: null,
    update: null,
    progress: null,
    error: null,

    check: async (source) => {
      const previous = get();
      if (previous.phase === "checking") {
        if (source === "manual") set({ source: "manual", dialogOpen: true });
        return;
      }
      if (BUSY_PHASES.has(previous.phase)) return;
      if (source === "background" && previous.dialogOpen) return;

      // Claim the operation synchronously so two menu/startup checks cannot
      // race across the first await. A previous native Update RID must be
      // closed before Rust will allow another check.
      set({
        phase: "checking",
        source,
        dialogOpen: source === "manual" ? true : previous.dialogOpen,
        progress: null,
        error: null,
      });
      let previousClosed = false;
      try {
        if (previous.update) {
          await backend.close(previous.update.rid);
          previousClosed = true;
        }
        set({ update: null });
        const update = await backend.check();
        const completionSource = get().source ?? source;
        if (update) {
          set({ phase: "available", dialogOpen: true, update, error: null });
        } else if (completionSource === "manual") {
          set({ phase: "upToDate", dialogOpen: true, update: null, error: null });
        } else {
          set({ phase: "idle", dialogOpen: false, source: null, update: null, error: null });
        }
      } catch (error) {
        const message = errorMessage(error);
        const completionSource = get().source ?? source;
        set(
          completionSource === "manual"
            ? {
                phase: "error",
                dialogOpen: true,
                update: previousClosed ? null : previous.update,
                error: message,
              }
            : {
                phase: "idle",
                dialogOpen: false,
                source: null,
                update: null,
                error: null,
              },
        );
      }
    },

    install: async () => {
      const pending = get();
      if (pending.phase !== "available" || !pending.update) return;

      const rid = pending.update.rid;
      let contentLength: number | null = null;
      set({ phase: "downloading", progress: null, error: null });
      try {
        await backend.install(rid, (event) => {
          switch (event.event) {
            case "started":
              contentLength = event.data.contentLength;
              set({
                phase: "downloading",
                progress: contentLength !== null && contentLength > 0 ? 0 : null,
              });
              break;
            case "progress":
              set({
                phase: "downloading",
                progress:
                  contentLength !== null && contentLength > 0
                    ? Math.min(100, Math.max(0, Math.round((event.data.downloaded / contentLength) * 100)))
                    : null,
              });
              break;
            case "installing":
              set({ phase: "installing", progress: 100 });
              break;
            case "restarting":
              set({ phase: "restarting", progress: 100 });
              break;
          }
        });
      } catch (error) {
        // Rust consumes the Update RID before download/install. Never reuse it
        // after any install failure; a retry starts with a fresh signed check.
        set({
          phase: "error",
          dialogOpen: true,
          update: null,
          progress: null,
          error: errorMessage(error),
        });
      }
    },

    dismiss: () => {
      if (closeInFlight) return closeInFlight;
      const current = get();
      if (BUSY_PHASES.has(current.phase)) return Promise.resolve();
      if (!current.update) {
        set({
          phase: "idle",
          dialogOpen: false,
          source: null,
          update: null,
          progress: null,
          error: null,
        });
        return Promise.resolve();
      }

      const pending = current.update;
      set({ phase: "closing", error: null });
      const operation = (async () => {
        try {
          await backend.close(pending.rid);
        } catch (error) {
          const latest = get();
          if (latest.phase === "closing" && latest.update?.rid === pending.rid) {
            set({ phase: "error", dialogOpen: true, error: errorMessage(error) });
          }
          return;
        }

        const latest = get();
        if (latest.phase === "closing" && latest.update?.rid === pending.rid) {
          set({
            phase: "idle",
            dialogOpen: false,
            source: null,
            update: null,
            progress: null,
            error: null,
          });
        }
      })();
      closeInFlight = operation.finally(() => {
        closeInFlight = null;
      });
      return closeInFlight;
    },
  }));
}

export const useUpdateStore = createUpdateStore();
