"""Prepare a private demo bundle; does not impersonate GUI import evidence."""
import json,copy
from pathlib import Path
root=Path(__file__).resolve().parents[1]
bundle=root/'OpenTake Official Demo.opentake'
bundle.mkdir(exist_ok=True)
base={'fps':30,'width':1920,'height':1080,'settingsConfigured':True,'tracks':[]}
def clip(id,media,typ,start,duration,trim=0):
 return {'id':id,'mediaRef':media,'mediaType':typ,'sourceClipType':typ,'startFrame':start,'durationFrames':duration,'trimStartFrame':trim,'trimEndFrame':0,'speed':1.,'volume':.6,'opacity':1.,'transform':{'centerX':.5,'centerY':.5,'width':1.,'height':1.,'rotation':0.,'flipHorizontal':False,'flipVertical':False}}
text=clip('official-title','official-title-media','text',0,240)
text.update(textContent='以你的节奏，继续创作。',textStyle={'fontName':'PingFang SC','fontSize':52,'alignment':'center','color':{'r':1.,'g':1.,'b':1.,'a':1.}})
text['transform'].update(centerX=.66,centerY=.89,width=.6,height=.12)
for id,kind,clips in [('official-text','video',[text]),('official-video','video',[clip('shot-a','official-motion','video',0,90),clip('shot-b','official-motion','video',90,60,90),clip('shot-c','official-motion','video',150,90,150)]),('official-audio','audio',[clip('bed','official-score','audio',0,240)])]:
 base['tracks'].append({'id':id,'type':kind,'muted':False,'hidden':False,'syncLocked':True,'clips':clips})
media={'version':2,'entries':[],'folders':[]}
for id,name,typ,path in [('official-motion','Original Motion','video','original-motion.mp4'),('official-score','Original Score','audio','demo-bed.wav')]:
 e={'id':id,'name':name,'type':typ,'source':{'external':{'absolutePath':str(root/'public/assets'/path)}},'duration':8.,'hasAudio':typ=='audio'}
 if typ=='video':e.update(sourceWidth=1920,sourceHeight=1080,sourceFPS=30.)
 media['entries'].append(e)
for file,value in [('project.json',base),('media.json',media)]: (bundle/file).write_text(json.dumps(value,ensure_ascii=False,indent=2))
print(bundle)
