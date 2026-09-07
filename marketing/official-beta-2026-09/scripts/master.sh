#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
ffmpeg -hide_banner -y -i review/remotion-render.mp4 -t 60 \
 -vf 'scale=in_color_matrix=bt601:out_color_matrix=bt709:in_range=full:out_range=tv,format=yuv420p' \
 -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p \
 -color_primaries bt709 -color_trc bt709 -colorspace bt709 -color_range tv \
 -c:a aac -b:a 256k -ar 48000 -movflags +faststart \
 -metadata title='OpenTake 1.0.0-beta.6 | Public Beta' \
 -metadata comment='Historical app captures; original motion and synthesized score. Candidate version, not a release announcement.' \
 deliverables/OpenTake-beta6-official-1080p.mp4
