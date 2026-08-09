# TitleBar controls automated + real-device evidence — 2026-07-30

Scope: `home-shell-implementation.md` Task 13 / `implementation-slice-1f94f01d3b65a701`, covering seven title-bar control records.

## Automated evidence

The exact planned tests now exist in `web/src/components/shell/TitleBar.interaction.test.tsx`:

- `control-f52cc89817361a19 return from editor to Home`
- `control-4bda8f075e1f3a14 open the global Library`
- `control-ff132f94a8c87906 open Settings from the editor`
- `control-d7ba227c6447e43e open Video Export`
- `control-c035467e6746e570 open/close subtitle export formats`
- `control-229710d0115f07bc open/close interchange export menu`
- `control-02d1bf7fff7c1e3a open Video Export from the interchange menu`

The focused filter passed all seven candidates. It proves the exact UI-store transitions, both popup menus' expanded state, Escape/outside-click dismissal, empty-timeline video-export disablement, populated-timeline enablement, and menu-to-dialog return path.

Regression:

```text
pnpm -C web test
pnpm -C web build
cargo fmt --check
git diff --check
```

Result: 70 files / 715 web tests passed; the production build completed. Vite emitted only the existing ineffective-dynamic-import and large-chunk warnings. Formatting and diff checks passed.

## Real macOS application loop

Application: `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-generation/target/debug/bundle/macos/OpenTake.app`.

Project: `/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`, 890 frames at 30 fps with the saved three-fragment linked A/V timeline and four captions.

Observed in the real interface:

1. `设置` opened the Settings overlay at `通用`; `完成` closed it and returned to the unchanged editor.
2. `素材库` opened the global Library. `返回主页` reached Home; the `TalkingHeadQA` recent card was visible with the exact saved path, and double-click reopened the same 890-frame project.
3. The direct `导出视频` control was enabled for the populated timeline, opened the video export dialog, and `取消` returned to the editor.
4. `导出` opened the interchange menu with MP4, XMEML, FCPXML, OTIO, and EDL entries; Escape dismissed the menu.
5. Reopening `导出` and selecting `渲染为视频（MP4）` closed the popup and opened the same video export dialog; `取消` returned to the editor.
6. The previously validated `导出字幕` popup remained present and usable; its SRT/VTT native export evidence is recorded separately in `subtitle-export-real-device-2026-07-30.md`.

The final accessibility tree still showed `TalkingHeadQA` at `00:29:20`, all six linked A/V clip fragments, and no modal or popup left open.

## Result

All seven controls in Task 13 are verified by exact owning tests and a real application round trip. This closes only the planned title-bar control slice; Home launcher controls, interchange file contents, and other shell surfaces remain independently tracked.
