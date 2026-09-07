import React from 'react';
import {Img,staticFile} from 'remotion';
import {Background,Brand,Footer,Heading,Screen} from '../theme';
export const Edit:React.FC=()=> <Background><Brand/><Heading eyebrow="02 / FIND YOUR RHYTHM" title="每一刀，都有节奏。" sub="定位播放头 · 分割片段 · 调整时长 · 撤销"/><Screen src="edit.png" left={1010} top={160} width={800} height={495}/><div style={{position:'absolute',left:96,top:660,right:96,height:282,border:'1px solid #527161',borderRadius:10,overflow:'hidden',boxShadow:'0 20px 65px #0008'}}><Img src={staticFile('assets/timeline-detail.png')} style={{width:'100%',height:'100%',objectFit:'cover'}}/></div><Footer note="历史安装包时间线实拍与局部放大 · 2026-08-07"/></Background>;
