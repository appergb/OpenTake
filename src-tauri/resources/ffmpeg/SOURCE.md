# Bundled FFmpeg

OpenTake packages checksum-pinned FFmpeg and FFprobe binaries. The exact
per-platform download URL, archive member (when applicable), archive SHA-256,
extracted-binary SHA-256, and reported version are recorded in
`scripts/ffmpeg-sidecars.lock.json`.

The Apple Silicon pair is the FFmpeg 7.0 arm64 build published by
[OSXExperts](https://www.osxexperts.net/). Its reported configure line enables
GPL components such as x264/x265 but does **not** enable FFmpeg's `nonfree`
option. Both programs report GNU GPL terms through `-L`. The provisioning script
fails closed when a binary reports `--enable-nonfree` or says it is not legally
redistributable. Other target records remain pinned to the
[`eugeneware/ffmpeg-static` b6.1.1 release](https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.1.1)
and are subject to the same fail-closed runtime license check on their native
build hosts.

FFmpeg is distributed under GPL-compatible terms; OpenTake itself is
GPL-3.0-or-later and packages its complete GPL license and this source notice.

FFmpeg source tags used by the locked target assets:
https://github.com/FFmpeg/FFmpeg/tree/n7.0 and
https://github.com/FFmpeg/FFmpeg/tree/n6.1.1
