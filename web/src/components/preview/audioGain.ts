/**
 * WebKit preview gain routing.
 *
 * HTMLMediaElement.volume is limited to [0, 1], while OpenTake's clip volume
 * model can represent gains above unity. Once a preview element needs a boost,
 * route it through one MediaElementAudioSourceNode + GainNode pair and keep
 * the requested gain there. Elements that never need a boost retain the
 * simpler native volume path.
 */

interface GainGraph {
  source: MediaElementAudioSourceNode;
  gain: GainNode;
  requestedGain: number;
  muted: boolean;
}

export type AudioContextFactory = () => AudioContext;

export interface PreviewAudioGainController {
  setGain(element: HTMLMediaElement, gain: number, muted: boolean): void;
  setMuted(element: HTMLMediaElement, muted: boolean): void;
  remove(element: HTMLMediaElement): void;
  dispose(): void;
}

function defaultAudioContextFactory(): AudioContext {
  const scope = typeof window === "undefined" ? undefined : window as Window & {
    AudioContext?: typeof AudioContext;
    webkitAudioContext?: typeof AudioContext;
  };
  const Context = scope?.AudioContext ?? scope?.webkitAudioContext;
  if (!Context) throw new Error("Web Audio is unavailable");
  return new Context();
}

function finiteGain(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function nativeVolume(value: number): number {
  return Math.min(1, finiteGain(value));
}

export function createPreviewAudioGainController(
  createContext: AudioContextFactory = defaultAudioContextFactory,
): PreviewAudioGainController {
  const graphs = new Map<HTMLMediaElement, GainGraph>();
  let context: AudioContext | null = null;
  let unavailable = false;

  function ensureContext(): AudioContext | null {
    if (context) return context;
    if (unavailable) return null;
    try {
      context = createContext();
      return context;
    } catch {
      unavailable = true;
      return null;
    }
  }

  function resumeContext(): void {
    if (!context || context.state === "running") return;
    void context.resume().catch(() => undefined);
  }

  function setGraphGain(graph: GainGraph): void {
    graph.gain.gain.value = graph.muted ? 0 : graph.requestedGain;
  }

  return {
    setGain(element, rawGain, muted) {
      const requestedGain = finiteGain(rawGain);
      const existing = graphs.get(element);
      if (!existing && requestedGain <= 1) {
        element.muted = muted;
        element.volume = nativeVolume(requestedGain);
        return;
      }

      const graph = existing ?? (() => {
        const audioContext = ensureContext();
        if (!audioContext) return null;
        try {
          const source = audioContext.createMediaElementSource(element);
          const gain = audioContext.createGain();
          source.connect(gain);
          gain.connect(audioContext.destination);
          const created: GainGraph = {
            source,
            gain,
            requestedGain,
            muted,
          };
          graphs.set(element, created);
          return created;
        } catch {
          unavailable = true;
          return null;
        }
      })();

      if (!graph) {
        element.muted = muted;
        element.volume = nativeVolume(requestedGain);
        return;
      }

      graph.requestedGain = requestedGain;
      graph.muted = muted;
      // The element's native path is muted after it is connected. The GainNode
      // remains the sole source of audible volume, including temporary mutes.
      element.muted = true;
      setGraphGain(graph);
      resumeContext();
    },

    setMuted(element, muted) {
      const graph = graphs.get(element);
      if (!graph) {
        element.muted = muted;
        return;
      }
      graph.muted = muted;
      element.muted = true;
      setGraphGain(graph);
      resumeContext();
    },

    remove(element) {
      const graph = graphs.get(element);
      if (!graph) return;
      graph.source.disconnect();
      graph.gain.disconnect();
      graphs.delete(element);
    },

    dispose() {
      for (const graph of graphs.values()) {
        graph.source.disconnect();
        graph.gain.disconnect();
      }
      graphs.clear();
      const closing = context;
      context = null;
      if (closing) void closing.close().catch(() => undefined);
    },
  };
}
