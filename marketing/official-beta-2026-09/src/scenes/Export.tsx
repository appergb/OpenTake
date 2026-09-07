import React from 'react';
import {Sequence,AbsoluteFill,staticFile} from 'remotion';
import {Video} from '@remotion/media';
import {Background,Brand,Footer,Heading,Screen} from '../theme';
export const ExportScene:React.FC=()=> <Background><Brand/><Heading eyebrow="06 / MAKE IT A FILM" title="把时间线，变成成片。" sub="选择导出设置，把剪辑带出工作区。"/>
 <Sequence durationInFrames={130}><Screen src="export.png"/><div style={{position:'absolute',left:100,top:530,color:'#aedcc6',fontSize:52,lineHeight:1.8}}>设置画幅<br/>选择格式<br/><span style={{color:'#6a9782'}}>导出作品</span></div></Sequence>
 <Sequence from={130}><AbsoluteFill style={{left:694,top:345,width:1130,height:610,borderRadius:12,overflow:'hidden',border:'1px solid #486456',background:'#101010',justifyContent:'center'}}><Video src={staticFile('assets/demo-export.mp4')} muted style={{width:'100%',height:'100%',objectFit:'contain'}}/></AbsoluteFill><div style={{position:'absolute',left:100,top:560,fontSize:50,lineHeight:1.6,color:'#acd6be'}}>H.264<br/><span style={{fontSize:30,color:'#83a390'}}>历史实机导出样片</span></div></Sequence>
 <Footer note="历史安装包导出界面与真实输出 · 2026-08-07"/></Background>;
