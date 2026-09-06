#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
video=deliverables/OpenTake-beta6-official-1080p.mp4
ffprobe -v error -show_format -show_streams -of json "$video" > review/ffprobe-full.json
ffmpeg -hide_banner -v error -xerror -i "$video" -f null - > review/full-decode.log 2>&1
for seconds in 0 3 9 16 23 30 34 40 47 50 56 59; do
 ffmpeg -hide_banner -loglevel error -y -ss "$seconds" -i "$video" -frames:v 1 "review/frame-${seconds}s.png"
done
ffmpeg -hide_banner -i "$video" -af volumedetect -vn -sn -dn -f null - > review/audio-levels.log 2>&1
printf '%s\n' 'ffprobe, full decode, frame extraction, audio levels complete.'
python3 scripts/check_metadata.py > review/metadata-check.log
