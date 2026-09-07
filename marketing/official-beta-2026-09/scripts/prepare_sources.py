from pathlib import Path
from PIL import Image
import shutil,json
root=Path(__file__).resolve().parents[1];repo=root.parents[1]
mapping={
 'import.png':'docs/audit/2026-08-07/editor-core-after-fix-assets/03-source-preview-fixed.png',
 'edit.png':'docs/audit/2026-08-07/editor-core-after-fix-assets/06-single-track-seek-midpoint.png',
 'tracks.png':'docs/audit/2026-08-07/editor-core-after-fix-assets/08-dual-track-composite-stable.png',
 'text.png':'docs/audit/2026-07-14/runtime-artifacts/automated/generic-effects-packaged-ui-2026-07-31.png',
 'effects.png':'docs/audit/2026-07-14/runtime-artifacts/automated/lgg-packaged-ui-2026-07-31.jpg',
 'export.png':'docs/audit/2026-08-07/editor-core-after-fix-assets/12-export-dialog-contrast.png',
 'logo.png':'assets/opentake-logo.png'}
for name,source in mapping.items():
 dest=root/'public/assets'/name
 if name=='tracks.png' and dest.exists() and not (root/'review/unusable-current-window.png').exists():shutil.copy2(dest,root/'review/unusable-current-window.png')
 Image.open(repo/source).save(dest)
# Real screenshot detail crops, no synthetic controls or modified content.
im=Image.open(root/'public/assets/edit.png');im.crop((0,568,1224,768)).save(root/'public/assets/timeline-detail.png')
im=Image.open(root/'public/assets/text.png');im.crop((1080,0,1224,768)).save(root/'public/assets/inspector-detail.png')
im=Image.open(repo/'docs/audit/2026-08-13/screenshots/beta5-packaged-agent.png');im.crop((0,62,635,1310)).save(root/'public/assets/agent-history-crop.png')
(root/'review/source-map.json').write_text(json.dumps(mapping,ensure_ascii=False,indent=2))
