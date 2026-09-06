import json,struct
from pathlib import Path
root=Path(__file__).resolve().parents[1]
data=json.loads((root/'review/ffprobe-full.json').read_text())
video=next(s for s in data['streams'] if s['codec_type']=='video')
audio=next(s for s in data['streams'] if s['codec_type']=='audio')
assert (video['codec_name'],video['width'],video['height'],video['pix_fmt'],video['avg_frame_rate'],video['nb_frames'])==('h264',1920,1080,'yuv420p','30/1','1800')
assert audio['codec_name']=='aac' and audio['sample_rate']=='48000' and audio['channels']==2
assert float(data['format']['duration'])==60
assert (root/'review/full-decode.log').stat().st_size==0
# Confirm progressive-download atom ordering rather than only trusting the flag.
path=root/'deliverables/OpenTake-beta6-official-1080p.mp4';atoms=[]
with path.open('rb') as f:
 while True:
  header=f.read(8)
  if len(header)<8:break
  size,kind=struct.unpack('>I4s',header)
  if size==1:size=struct.unpack('>Q',f.read(8))[0];f.seek(size-16,1)
  elif size>0:f.seek(size-8,1)
  else:break
  atoms.append(kind.decode('ascii','replace'))
assert atoms.index('moov')<atoms.index('mdat')
print('PASS: 60s, 1800 frames, 1920x1080 H.264/yuv420p, AAC/48kHz/stereo, decode clean, faststart.')
print('Top-level MP4 atoms:', ', '.join(atoms))
