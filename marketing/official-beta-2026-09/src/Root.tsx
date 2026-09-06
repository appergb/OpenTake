import React from 'react';
import {Composition} from 'remotion';
import {Footage} from './scenes/Footage';
import {Film} from './Film';
import {Intro} from './scenes/Intro';
export const Root: React.FC = () => <>
 <Composition id="OriginalMotion" component={Footage} durationInFrames={240} fps={30} width={1920} height={1080}/>
 <Composition id="OpenTakeOfficial" component={Film} durationInFrames={1800} fps={30} width={1920} height={1080}/>
 <Composition id="Cover" component={Intro} durationInFrames={180} fps={30} width={1920} height={1080}/>
 </>;
