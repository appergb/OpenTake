export interface MotionCanvasJob {
  templateId: 'title-card';
  params: {
    title: string;
    subtitle: string;
    accent: string;
    background: string;
  };
  fps: number;
  width: number;
  height: number;
  durationFrames: number;
  durationSeconds: number;
}

export interface MotionCanvasResult {
  renderer: 'motion-canvas';
  rendererVersion: '3.17.2';
  outputFile: 'output.mp4';
  fps: number;
  width: number;
  height: number;
  durationFrames: number;
  durationSeconds: number;
  contentHash: string;
}

export function parseMotionCanvasJob(value: unknown): MotionCanvasJob {
  if (!value || typeof value !== 'object') throw new Error('motion job must be an object');
  const job = value as Record<string, unknown>;
  if (job.templateId !== 'title-card') throw new Error('unsupported Motion Canvas template');
  const positiveInteger = (name: string) => {
    const candidate = job[name];
    if (!Number.isInteger(candidate) || Number(candidate) < 1) {
      throw new Error(`${name} must be a positive integer`);
    }
    return Number(candidate);
  };
  const fps = positiveInteger('fps');
  const durationFrames = positiveInteger('durationFrames');
  const params = job.params;
  if (!params || typeof params !== 'object') throw new Error('params must be an object');
  const source = params as Record<string, unknown>;
  const string = (name: string) => {
    const candidate = source[name];
    if (typeof candidate !== 'string') throw new Error(`params.${name} must be a string`);
    return candidate;
  };
  return {
    templateId: 'title-card',
    params: {
      title: string('title'),
      subtitle: string('subtitle'),
      accent: string('accent'),
      background: string('background'),
    },
    fps,
    width: positiveInteger('width'),
    height: positiveInteger('height'),
    durationFrames,
    durationSeconds: durationFrames / fps,
  };
}

export function resultMetadata(job: MotionCanvasJob, contentHash: string): MotionCanvasResult {
  if (!/^[0-9a-f]{64}$/.test(contentHash)) throw new Error('content hash must be SHA-256 hex');
  return {
    renderer: 'motion-canvas',
    rendererVersion: '3.17.2',
    outputFile: 'output.mp4',
    fps: job.fps,
    width: job.width,
    height: job.height,
    durationFrames: job.durationFrames,
    durationSeconds: job.durationSeconds,
    contentHash,
  };
}
