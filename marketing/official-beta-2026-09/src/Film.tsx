import React from 'react';
import {Audio} from '@remotion/media';
import {staticFile} from 'remotion';
import {TransitionSeries,linearTiming} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {Intro} from './scenes/Intro';
import {ImportScene} from './scenes/Import';
import {Edit} from './scenes/Edit';
import {Tracks} from './scenes/Tracks';
import {TextEffects} from './scenes/TextEffects';
import {Agent} from './scenes/Agent';
import {ExportScene} from './scenes/Export';
import {Outro} from './scenes/Outro';
export const Film:React.FC=()=> <><Audio src={staticFile('assets/score.m4a')} volume={.8}/><TransitionSeries>
 <TransitionSeries.Sequence durationInFrames={180}><Intro/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={225}><ImportScene/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={225}><Edit/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={225}><Tracks/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={255}><TextEffects/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={285}><Agent/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={255}><ExportScene/></TransitionSeries.Sequence>
 <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames:15})}/>
 <TransitionSeries.Sequence durationInFrames={255}><Outro/></TransitionSeries.Sequence>
 </TransitionSeries></>;
