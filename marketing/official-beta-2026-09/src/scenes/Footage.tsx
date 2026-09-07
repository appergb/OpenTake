import React from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame} from 'remotion';
export const Footage: React.FC = () => {
 const f=useCurrentFrame();
 return <AbsoluteFill style={{background:'#081419',overflow:'hidden',fontFamily:'Arial',color:'#e9fcf5'}}>
  <div style={{position:'absolute',width:1300,height:1300,left:650,top:-160,borderRadius:'50%',background:'radial-gradient(circle at 25% 30%, #ddfff1 0%, #82dfc1 23%, #199c89 49%, #063339 66%, #020b10 73%)',rotate:`${f*.14}deg`,scale:1+Math.sin(f/70)*.045,boxShadow:'0 0 150px #12675444'}}/>
  <div style={{position:'absolute',width:760,height:760,left:930,top:100,borderRadius:'50%',background:'#081419',boxShadow:'inset 5px 15px 60px #000, -20px -5px 40px #c3ffe540',translate:`${Math.sin(f/60)*18}px ${Math.cos(f/60)*18}px`}}/>
  {Array.from({length:20},(_,i)=><div key={i} style={{position:'absolute',left:80+i*44,top:720+Math.sin(i*.28+f/50)*80,width:2,height:210,background:`rgba(140,235,204,${.1+i*.008})`,rotate:'-24deg'}}/>)}
  <div style={{position:'absolute',left:105,top:125,fontSize:24,letterSpacing:9,color:'#97c3b8'}}>OPENTAKE / ORIGINAL MOTION</div>
  <div style={{position:'absolute',left:100,top:285,fontSize:144,fontWeight:600,lineHeight:1.03,letterSpacing:-7,translate:`0px ${interpolate(f,[0,45],[22,0],{extrapolateRight:'clamp'})}px`}}>MAKE<br/>YOUR<br/><span style={{color:'#9aead0'}}>NEXT CUT.</span></div>
  <div style={{position:'absolute',left:110,bottom:80,fontSize:22,letterSpacing:6}}>FORM / LIGHT / RHYTHM</div>
 </AbsoluteFill>
};
