import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const cli=path.join(root,'node_modules/@remotion/cli/remotion-cli.js');
const command=process.argv[2]??'film';
const output='review/remotion-render.mp4';
const args=command==='cover'?['still','src/index.ts','Cover','deliverables/OpenTake-beta6-cover.png','--frame=95']:command==='motion'?['render','src/index.ts','OriginalMotion','public/assets/original-motion.mp4','--codec=h264','--crf=18']:['render','src/index.ts','OpenTakeOfficial',output,'--codec=h264','--crf=18','--pixel-format=yuv420p','--audio-codec=aac','--audio-bitrate=256k'];
const result=spawnSync(process.execPath,[cli,...args],{cwd:root,stdio:'inherit'});
if(result.status!==0) process.exit(result.status??1);
if(command==='film'){const mastered=spawnSync('sh',['scripts/master.sh'],{cwd:root,stdio:'inherit'});process.exit(mastered.status??1);}
