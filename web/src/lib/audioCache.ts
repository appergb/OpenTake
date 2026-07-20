/**
 * LRU cache of decoded `AudioBuffer`s for the streaming PCM pipeline (#160).
 *
 * The Rust backend (`extract_pcm_chunk`) decodes short chunks of audio to
 * interleaved f32; the front end re-wraps them as `AudioBuffer`s for the Web
 * Audio API. This cache avoids re-decoding the same chunk when the playhead
 * oscillates within a window (scrubbing, loop playback) — the most common
 * re-decode trigger.
 *
 * This is a UTILITY for future use by the playback layer; it is NOT wired into
 * `TimelinePlaybackLayer` yet (the `<audio>` elements still drive playback
 * natively). The seam is intentionally narrow: `getCached` / `setCached` +
 * LRU eviction, nothing else.
 *
 * Key: `"<mediaRef>@<floor(startTime)>"` — the chunk boundary is quantized to
 * the nearest second so repeated requests for the same chunk (within ±1 s) hit
 * the same entry, regardless of float jitter in the playhead.
 */

/** Maximum number of decoded chunks to keep resident. Each 5 s stereo 48 kHz
 *  chunk is ~1.9 MB of f32, so 10 entries ≈ 19 MB — a generous working set for
 *  scrubbing without ballooning the WebView's heap. */
const MAX_ENTRIES = 10;

/** Quantize a start time (seconds) to a 1-second chunk boundary for the cache
 *  key. Float jitter in the playhead (e.g. 1.501 vs 1.499) collapses to the
 *  same key so the cache isn't fragmented. */
function chunkKey(mediaRef: string, startTime: number): string {
  return `${mediaRef}@${Math.floor(startTime)}`;
}

/**
 * A bounded LRU cache of decoded `AudioBuffer`s. Uses a `Map` (which preserves
 * insertion order in JS) to track recency: `getCached` re-inserts on hit to
 * mark the entry as most-recently-used; `setCached` evicts the oldest entry
 * when the cache is full.
 */
export class AudioBufferCache {
  private readonly entries: Map<string, AudioBuffer> = new Map();
  private readonly maxEntries: number;

  constructor(maxEntries: number = MAX_ENTRIES) {
    this.maxEntries = maxEntries;
  }

  /**
   * Look up a cached `AudioBuffer` for `mediaRef` starting at `startTime`.
   * A hit re-inserts the entry so it becomes the most-recently-used (LRU
   * semantics). Returns `null` on miss.
   */
  getCached(mediaRef: string, startTime: number): AudioBuffer | null {
    const key = chunkKey(mediaRef, startTime);
    const hit = this.entries.get(key);
    if (hit === undefined) return null;
    // Re-insert to mark as recently used (Map preserves insertion order, so
    // delete + set moves the entry to the end = most recent).
    this.entries.delete(key);
    this.entries.set(key, hit);
    return hit;
  }

  /**
   * Store a decoded `AudioBuffer` for `mediaRef` starting at `startTime`. When
   * the cache is full, the oldest entry (least recently used) is evicted first.
   */
  setCached(mediaRef: string, startTime: number, buffer: AudioBuffer): void {
    const key = chunkKey(mediaRef, startTime);
    // If the key already exists, delete first so the re-insert moves it to the
    // end (most recent) and we don't double-count the size.
    if (this.entries.has(key)) {
      this.entries.delete(key);
    } else if (this.entries.size >= this.maxEntries) {
      // Evict the oldest entry (first key in insertion order).
      const oldest = this.entries.keys().next();
      if (!oldest.done) {
        this.entries.delete(oldest.value as string);
      }
    }
    this.entries.set(key, buffer);
  }

  /** Drop all cached buffers (e.g. when the project closes or media is
   *  relinked, so stale buffers don't leak across sessions). */
  clear(): void {
    this.entries.clear();
  }

  /** Current number of cached entries (useful for tests / diagnostics). */
  get size(): number {
    return this.entries.size;
  }
}

/** Shared singleton instance for the playback layer. One cache per WebView —
 *  the working set is small (10 chunks) and shared across all clips. */
export const audioBufferCache = new AudioBufferCache();
