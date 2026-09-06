import React from 'react';
import {Background,Brand,Footer,Heading,Screen} from '../theme';
export const ImportScene:React.FC=()=> <Background><Brand/><Heading eyebrow="01 / IMPORT" title="从第一份素材开始。" sub="导入视频与音频，整理你的创作起点。"/><div style={{position:'absolute',left:100,top:500,fontSize:59,lineHeight:1.6,color:'#abcbbd'}}>素材<br/><span style={{color:'#548573',fontSize:38}}>↓</span><br/>预览<br/><span style={{color:'#548573',fontSize:38}}>↓</span><br/>时间线</div><Screen src="import.png"/><Footer note="历史安装包素材预览实拍 · 2026-08-07"/></Background>;
