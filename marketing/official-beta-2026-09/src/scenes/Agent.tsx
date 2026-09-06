import React from 'react';
import {Img,staticFile,interpolate,useCurrentFrame} from 'remotion';
import {Background,Brand,Footer} from '../theme';
export const Agent:React.FC=()=>{const f=useCurrentFrame();return <Background><Brand/>
 <div style={{position:'absolute',left:98,top:230,width:850}}><div style={{color:'#97dfc4',fontFamily:'Arial',fontSize:22,letterSpacing:5,marginBottom:28}}>05 / EDIT WITH AI</div><div style={{fontSize:86,lineHeight:1.32,letterSpacing:-3}}>让 AI，<br/><span style={{color:'#a4efd1'}}>参与剪辑。</span></div><div style={{marginTop:46,color:'#bed0c7',fontSize:35,lineHeight:1.8}}>Agent 对话与工具调用<br/>通过 MCP 连接外部助手</div><div style={{marginTop:42,fontSize:26,color:'#81978d',lineHeight:1.65}}>需配置模型服务与凭据<br/>可用能力取决于服务与配置</div></div>
 <div style={{position:'absolute',right:180,top:125,width:590,height:800,overflow:'hidden',border:'1px solid #486456',borderRadius:16,boxShadow:'0 25px 80px #0009',opacity:interpolate(f,[8,30],[0,1],{extrapolateLeft:'clamp',extrapolateRight:'clamp'}),translate:`0px ${interpolate(f,[0,40],[24,0],{extrapolateRight:'clamp'})}px`}}><Img src={staticFile('assets/agent-history-crop.png')} style={{width:'100%',height:'100%',objectFit:'contain',background:'#191919'}}/></div>
 <Footer note="历史安装包 Agent 工具调用实拍 · 2026-08-13"/></Background>};
