import {Layout, Rect, Txt, makeScene2D} from '@motion-canvas/2d';
import {all, createRef, easeOutCubic, waitFor} from '@motion-canvas/core';

interface OpenTakeMotionConfig {
  title?: string;
  subtitle?: string;
  accent?: string;
  background?: string;
  durationSeconds?: number;
}

declare global {
  interface Window {
    __OPENTAKE_MOTION_CONFIG__?: OpenTakeMotionConfig;
  }
}

export default makeScene2D(function* (view) {
  const config = window.__OPENTAKE_MOTION_CONFIG__ ?? {};
  const card = createRef<Rect>();
  const bar = createRef<Rect>();
  const duration = Math.max(0.7, config.durationSeconds ?? 3);

  view.fill(config.background ?? '#11131a');
  view.add(
    <Rect
      ref={card}
      width={'72%'}
      padding={70}
      radius={32}
      fill={'rgba(255,255,255,0.12)'}
      stroke={'rgba(255,255,255,0.30)'}
      lineWidth={2}
      opacity={0}
      y={72}
    >
      <Layout direction={'column'} gap={28} width={'100%'}>
        <Rect ref={bar} width={260} height={10} radius={5} fill={config.accent ?? '#7c5cff'} />
        <Txt
          text={config.title ?? 'OpenTake'}
          fill={'#ffffff'}
          fontFamily={'Arial, sans-serif'}
          fontSize={92}
          fontWeight={700}
          lineHeight={108}
        />
        <Txt
          text={config.subtitle ?? 'Motion Canvas'}
          fill={'rgba(255,255,255,0.76)'}
          fontFamily={'Arial, sans-serif'}
          fontSize={42}
          lineHeight={54}
        />
      </Layout>
    </Rect>,
  );

  yield* all(card().opacity(1, 0.55, easeOutCubic), card().y(0, 0.55, easeOutCubic));
  yield* bar().width(320, Math.max(0.1, duration - 0.75), easeOutCubic);
  yield* waitFor(0.2);
});
