"""Original 60 s electronic score; deterministic synthesis, no samples."""
from pathlib import Path
import numpy as np
import wave
sr=48000; duration=60; n=sr*duration
score=np.zeros((n,2),dtype=np.float64)
rng=np.random.default_rng(20260906)
def tone(start,dur,freq,amp,pan=0,kind='pad'):
 count=min(int(dur*sr),n-int(start*sr)); t=np.arange(count)/sr
 if kind=='pad':
  env=np.minimum(t/.65,1)*np.minimum((dur-t)/1.25,1)
  signal=(np.sin(2*np.pi*freq*t)+.28*np.sin(2*np.pi*freq*2.002*t)+.1*np.sin(2*np.pi*freq*3*t))*env
 elif kind=='pluck':
  env=np.minimum(t/.007,1)*np.exp(-t*5)
  signal=(np.sin(2*np.pi*freq*t)+.22*np.sin(2*np.pi*freq*2*t))*env
 else:
  signal=np.sin(2*np.pi*(45*t+45*.04*(1-np.exp(-t/.04))))*np.exp(-t*16)
 s=int(start*sr);score[s:s+count,0]+=signal*amp*(1-pan*.4);score[s:s+count,1]+=signal*amp*(1+pan*.4)
# E minor / Cmaj7 / G / D: understated, 96 BPM.
chords=[[52,55,59,66],[48,55,59,64],[43,55,59,62],[50,57,62,66]]
for bar in range(24):
 start=bar*2.5
 for j,m in enumerate(chords[(bar//2)%4]):tone(start,3.5,440*2**((m-69)/12),.035,(j-1.5)/2)
 for beat in range(4):
  at=start+beat*.625
  if at>4 and at<54:tone(at,.38,55,.10,kind='kick')
  m=chords[(bar//2)%4][(beat+bar)%4]+12
  tone(at,.8,440*2**((m-69)/12),.052,(-1)**beat*.6,'pluck')
  if at>6 and at<52:
   s=int((at+.3125)*sr); c=min(int(.085*sr),n-s)
   if c>0:
    noise=rng.normal(0,1,c);noise=np.diff(np.r_[0,noise]);noise*=np.exp(-np.arange(c)/sr*65)*.014
    score[s:s+c,:]+=noise[:,None]
# Small delay widens the arpeggio; musical fade ends gently.
delay=int(.3125*sr); score[delay:]+=score[:-delay,::-1]*.12
t=np.arange(n)/sr;score*= (np.minimum(t/2,1)*np.minimum((duration-t)/4,1))[:,None]
score=np.tanh(score);score*=.72/max(np.abs(score).max(),.01)
p=Path(__file__).resolve().parents[1]/'public/assets/original-score.wav'
with wave.open(str(p),'wb') as f:f.setnchannels(2);f.setsampwidth(2);f.setframerate(sr);f.writeframes((score*32767).astype('<i2').tobytes())
print(p)
