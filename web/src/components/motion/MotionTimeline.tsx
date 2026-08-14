import { useMemo } from "react";
import { cssLanguage } from "@codemirror/lang-css";
import { useT } from "../../i18n";
import type { MotionStudioStore } from "../../store/motionStudioStore";

export function motionKeyframeNames(css: string): string[] {
  const names: string[] = [];
  const seen = new Set<string>();
  cssLanguage.parser.parse(css).iterate({
    enter(node) {
      if (node.name !== "KeyframeName" || names.length >= 24) return;
      const name = css.slice(node.from, node.to);
      if (seen.has(name)) return;
      seen.add(name);
      names.push(name);
    },
  });
  return names;
}

export function MotionTimeline({ store }: { store: MotionStudioStore }) {
  const t = useT();
  const css = store((state) => state.css);
  const frame = store((state) => state.frame);
  const durationFrames = store((state) => state.parameters.durationFrames);
  const setFrame = store((state) => state.setFrame);
  const names = useMemo(() => motionKeyframeNames(css), [css]);
  const progress = durationFrames <= 1 ? 0 : (frame / (durationFrames - 1)) * 100;

  return (
    <section aria-label={t("motionStudio.timeline")} className="motion-studio__timeline motion-panel">
      <header className="motion-panel__header">
        <span>{t("motionStudio.timeline")}</span>
        <span className="motion-panel__meta">{t("motionStudio.integerFrames")}</span>
      </header>
      <div className="motion-timeline__ruler" aria-hidden="true">
        {[0, 25, 50, 75, 100].map((percent) => (
          <span key={percent} style={{ left: `${percent}%` }}>
            {Math.round(((durationFrames - 1) * percent) / 100)}
          </span>
        ))}
        <i style={{ left: `${progress}%` }} />
      </div>
      <div className="motion-timeline__tracks">
        {names.length > 0 ? names.map((name) => (
          <div key={name} className="motion-timeline__track">
            <code>{`@keyframes ${name}`}</code>
            <div>
              <button type="button" aria-label={t("motionStudio.seekKeyframe", { name, frame: 0 })} onClick={() => setFrame(0)} />
              <button
                type="button"
                aria-label={t("motionStudio.seekKeyframe", { name, frame: durationFrames - 1 })}
                onClick={() => setFrame(durationFrames - 1)}
              />
            </div>
          </div>
        )) : (
          <p>{t("motionStudio.noKeyframes")}</p>
        )}
      </div>
    </section>
  );
}
