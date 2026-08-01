import {Renderer, Vector2} from '@motion-canvas/core';
import {parseMotionCanvasJob} from '../src/job';

export function index() {
  throw new Error('OpenTake Motion Canvas runner requires exactly one project');
}

export async function editor(project) {
  const job = parseMotionCanvasJob(window.__OPENTAKE_MOTION_CONFIG__);
  const config = {...job.params, ...job};
  window.__OPENTAKE_MOTION_CONFIG__ = config;
  const fps = Math.max(1, Math.round(config.fps ?? 30));
  const width = Math.max(2, Math.round(config.width ?? 1920));
  const height = Math.max(2, Math.round(config.height ?? 1080));
  const size = new Vector2(width, height);
  const renderer = new Renderer(project);
  document.getElementById('stage').append(renderer.stage.finalBuffer);
  const settings = {
    name: 'OpenTake Motion Canvas Studio',
    range: [0, Math.max(1, config.durationSeconds ?? 3)],
    fps,
    size,
    resolutionScale: 1,
    colorSpace: 'srgb',
    background: config.background ?? '#11131a',
    exporter: {name: '@motion-canvas/core/image-sequence', options: {}},
  };

  window.OpenTake.onSeek(seconds => renderer.renderFrame(settings, Math.max(0, seconds)));
  window.__OPENTAKE_MOTION_CANVAS_READY__ = true;
}
