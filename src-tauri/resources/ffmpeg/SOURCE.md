# Bundled FFmpeg

OpenTake packages FFmpeg and FFprobe binaries from the checksum-pinned
[`eugeneware/ffmpeg-static` b6.1.1 release](https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.1.1).
The exact per-platform download URLs and SHA-256 digests are recorded in
`scripts/ffmpeg-sidecars.lock.json`, including the version actually reported by
each target binary (the release's Apple Silicon assets report 6.0). The upstream build metadata and license are
available with that release. FFmpeg is distributed under GPL-compatible terms;
OpenTake itself is GPL-3.0-or-later and packages its complete GPL license.

FFmpeg source tags used by the locked target assets:
https://github.com/FFmpeg/FFmpeg/tree/n6.0 and
https://github.com/FFmpeg/FFmpeg/tree/n6.1.1
