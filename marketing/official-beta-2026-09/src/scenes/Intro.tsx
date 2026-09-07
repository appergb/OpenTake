import React from 'react';
import {AbsoluteFill,Img,interpolate,staticFile,useCurrentFrame} from 'remotion';
import {Background,VERSION} from '../theme';
export const Intro:React.FC=()=>{const f=useCurrentFrame();return <Background>
 <div style={{position:'absolute',left:1160,top:180,width:600,height:600,borderRadius:'50%',background:'radial-gradient(circle at 25% 25%, #bdffe5, #44a78e 37%, #0d3433 64%, #040b0d 74%)',boxShadow:'0 0 120px #3aa68522',scale:interpolate(f,[0,180],[.88,1.04])}}/>
 <div style={{position:'absolute',left:1290,top:300,width:365,height:365,borderRadius:'50%',background:'#0b1517',boxShadow:'-15px -8px 40px #b4eed933'}}/>
 <AbsoluteFill style={{padding:104,justifyContent:'center',opacity:interpolate(f,[0,24],[0,1],{extrapolateRight:'clamp'})}}>
 <div style={{display:'flex',alignItems:'center',gap:27,marginBottom:65}}><Img src={staticFile('assets/logo.png')} style={{width:108,height:108}}/><span style={{fontFamily:'Arial',fontSize:98,letterSpacing:-4,fontWeight:600}}>OpenTake</span></div>
 <div style={{fontSize:84,lineHeight:1.4,fontWeight:500,letterSpacing:-2}}>把想法，<br/><span style={{color:'#a4efd1'}}>剪成作品。</span></div>
 <div style={{marginTop:55,fontSize:28,color:'#a5b9b0',letterSpacing:2}}>{VERSION} <span style={{color:'#568c77',padding:'0 18px'}}> / </span> PUBLIC BETA</div>
 </AbsoluteFill>
 <div style={{position:'absolute',right:100,bottom:80,fontSize:21,letterSpacing:5,color:'#7c9c8d'}}>A NEW TAKE ON EDITING</div>
 </Background>};
