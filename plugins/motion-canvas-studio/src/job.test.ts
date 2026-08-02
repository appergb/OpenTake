import assert from 'node:assert/strict';
import {describe, it} from 'node:test';
import {parseMotionCanvasJob, resultMetadata} from './job.ts';

describe('Motion Canvas job contract', () => {
  it('normalizes the fixed template and produces deterministic result metadata', () => {
    const raw = {
      templateId: 'title-card',
      params: {title: 'Beta', subtitle: 'OpenTake', accent: '#7c5cff', background: '#11131a'},
      fps: 24,
      width: 1920,
      height: 1080,
      durationFrames: 72,
    };
    const first = parseMotionCanvasJob(raw);
    const second = parseMotionCanvasJob(structuredClone(raw));
    assert.deepEqual(second, first);
    assert.deepEqual(
      resultMetadata(first, 'ab'.repeat(32)),
      resultMetadata(second, 'ab'.repeat(32)),
    );
  });

  it('rejects traversal-like templates and malformed dimensions', () => {
    assert.throws(
      () => parseMotionCanvasJob({
        templateId: '../title-card',
        params: {},
        fps: 30,
        width: 1920,
        height: 1080,
        durationFrames: 90,
      }),
      /unsupported/,
    );
    assert.throws(
      () => parseMotionCanvasJob({
        templateId: 'title-card',
        params: {title: '', subtitle: '', accent: '', background: ''},
        fps: 30,
        width: 0,
        height: 1080,
        durationFrames: 90,
      }),
      /width/,
    );
  });
});
