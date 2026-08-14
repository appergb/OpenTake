import { Pause, Play, RotateCcw } from "lucide-react";
import { useT } from "../../i18n";
import type { MotionStudioStore } from "../../store/motionStudioStore";
import { Icon } from "../ui/Icon";

export function MotionPreview({ store }: { store: MotionStudioStore }) {
  const t = useT();
  const frame = store((state) => state.frame);
  const parameters = store((state) => state.parameters);
  const playing = store((state) => state.playing);
  const phase = store((state) => state.previewPhase);
  const preview = store((state) => state.lastGoodPreview);
  const play = store((state) => state.play);
  const pause = store((state) => state.pause);
  const replay = store((state) => state.replay);
  const setFrame = store((state) => state.setFrame);

  return (
    <figure
      role="region"
      aria-label={t("motionStudio.preview")}
      className="motion-studio__preview motion-panel"
    >
      <figcaption className="motion-panel__header">
        <span>{t("motionStudio.preview")}</span>
        <span className="motion-panel__meta">
          {frame + 1} / {parameters.durationFrames}
        </span>
      </figcaption>
      <div className="motion-preview__canvas" style={{ aspectRatio: `${parameters.width} / ${parameters.height}` }}>
        {preview ? (
          <img src={preview.pngDataUrl} alt={t("motionStudio.previewFrame")} />
        ) : (
          <span>{t("motionStudio.previewPending")}</span>
        )}
        {phase === "loading" && (
          <span role="status" className="motion-preview__loading">
            {t("motionStudio.previewLoading")}
          </span>
        )}
      </div>
      <div className="motion-preview__controls" role="group" aria-label={t("motionStudio.playbackControls")}>
        <button type="button" className="motion-icon-button" aria-label={t("motionStudio.replay")} onClick={replay}>
          <Icon icon={RotateCcw} size={13} />
        </button>
        <button
          type="button"
          className="motion-icon-button"
          aria-label={t("motionStudio.play")}
          onClick={play}
        >
          <Icon icon={Play} size={13} />
        </button>
        <button
          type="button"
          className="motion-icon-button"
          aria-label={t("motionStudio.pause")}
          disabled={!playing}
          onClick={pause}
        >
          <Icon icon={Pause} size={13} />
        </button>
        <input
          type="range"
          min={0}
          max={Math.max(0, parameters.durationFrames - 1)}
          step={1}
          value={frame}
          aria-label={t("motionStudio.scrub")}
          onChange={(event) => setFrame(Number(event.currentTarget.value))}
        />
        <output aria-label={t("motionStudio.currentFrame")}>{frame}</output>
      </div>
    </figure>
  );
}
