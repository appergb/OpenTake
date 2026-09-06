import React from 'react';
import {Sequence} from 'remotion';
import {Background,Brand,Footer,Heading,Screen} from '../theme';
export const TextEffects:React.FC=()=> <Background><Brand/><Heading eyebrow="04 / TEXT & EFFECTS" title="让表达，多一层质感。" sub="文字标题与片段效果，在同一工作区调整。"/><div style={{position:'absolute',left:100,top:520,fontSize:60,lineHeight:1.8,color:'#afd6c2'}}>写下想说的<br/><span style={{color:'#648b77'}}>调出你的风格</span></div><Sequence durationInFrames={132}><Screen src="text.png"/></Sequence><Sequence from={132}><Screen src="effects.png"/></Sequence><Footer note="历史安装包文字 / 效果面板实拍 · 2026-07-31"/></Background>;
