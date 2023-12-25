'use strict'

var localVideo = document.querySelector('video#localvideo');
var remoteVideo = document.querySelector('video#remotevideo');

var btnConn = document.querySelector('button#connserver');
var btnLeave = document.querySelector('button#leave');

var localStream = null;

var roomid = '222222';
var socket =null;

var state = 'init';

var pc = null;

var pcConfig={
		'iceServers':[{
			'urls':'turn:121.41.76.43:3478',
			'credential':'123456',
			'username':'huang'
		}]
	}

function sendMessage(roomid,data){
	if(socket){
		socket.emit('message',roomid,data);
	}
}

function getAnswer(desc){
	pc.setLocalDescription(desc);
	sendMessage(roomid,desc);
}

function handleAnswerError(err){
	console.error('Failed to get Answer!',err);
}

function getOffer(desc){
	pc.setLocalDescription(desc);
	sendMessage(roomid,desc)
}
function handleOfferError(err){
	console.error('Failed to get Offer!',err);
}

//接收远端流通道
function call(){
	if(state === 'joined_conn'){
		if(pc){
			var options = {
				offerToReceiveAudio:1,
				offerToReceiveVideo:1
			}
			pc.createOffer(options)
			  .then(getOffer)
			  .catch(handleOfferError);
		}
	}	
}

// 第一步：开始服务
function connSignalServer(){
	//开启本地视频
	start();
	return true;
}

function conn(){
	//1 触发socke连接
	socket = io.connect();
	
	//2 加入房间后的回调
	socket.on('joined',(roomid,id)=>{
		
		state = 'joined';
		
		createPeerConnection();
		
		btnConn.disabled = true;
		btnLeave.disabled =false;
		
		console.log("reveive joined message:state=",state);	
	});
	socket.on('otherjoin',(roomid,id)=>{
		
		if (state === 'joined_unbind') {
			createPeerConnection();
		}
		state = 'joined_conn';
		
		//媒体协商
		call();
		console.log("reveive otherjoin message:state=",state);	
	});
	socket.on('full',(roomid,id)=>{
		console.log('receive full message ', roomid, id);

		closePeerConnection();
		closeLocalMedia();
		
		state = 'leaved';
		
		btnConn.disabled = false;
		btnLeave.disabled = true;
		console.log("reveive full message:state=",state);
		alert("the room is full!");
	});
	
	socket.on('leaved',(roomid,id)=>{
		
		state = 'leaved';
		socket.disconnect();
		btnConn.disabled = false;
		btnLeave.disabled = true;
		console.log("reveive leaved message:state=",state);
	});
	
	socket.on('bye',(roomid,id)=>{
		
		state = 'joined_unbind';
		closePeerConnection();
		console.log("reveive bye message:state=",state);	
	});
	socket.on('disconnect', (socket) => {
		console.log('receive disconnect message!', roomid);
		if(!(state === 'leaved')){
			closePeerConnection();
			closeLocalMedia();
		}
		state = 'leaved';
	
	});
	socket.on('message',(roomid,id,data)=>{
		
		//媒体协商
		if(data){
			if(data.type === 'offer'){
				pc.setRemoteDescription(new RTCSessionDescription(data));
				pc.createAnswer()
				  .then(getAnswer)
				  .catch(handleAnswerError);
			}else if(data.type === 'answer'){
				console.log("reveive client message=====>",data);
				pc.setRemoteDescription(new RTCSessionDescription(data));
			}else if(data.type === 'candidate'){
				var candidate = new RTCIceCandidate({
					sdpMLineIndex:data.label,
					candidate:data.candidate
				});
				pc.addIceCandidate(candidate);
				
			}else{
				console.error('the message is invalid!',data)
			}
		}
		
		console.log("reveive client message",roomid,id,data);	
	});
	
	socket.emit('join',roomid);
	return;
}

// 扑捉本地视频
function getMediaStream(stream){
	
	localStream =stream;
	//2 ===============显示本地视频===============
	localVideo.srcObject = localStream;
	
	//这个函数的调用时机特别重要 一定要在getMediaStream之后再调用，否则会出现绑定失败的情况
	conn();
}

function handleError(err){
	if(err){
		console.error("getUserMedia  error:",err);	
	}
}

// 第二步：采集本地视频
function start(){
	
	
	if (!navigator.mediaDevices||
			!navigator.mediaDevices.getDisplayMedia) {			
		  console.log("getDisplayMedia is not supported!")
		  return;
	} else {
		
		//1 ===============配置音视频参数===============
		var constraints={
			video:true,
			audio: true
		}
		
		navigator.mediaDevices.getDisplayMedia(constraints)
							  .then(getMediaStream)
							  .catch(handleError)
	}
}

//关闭流通道
function closeLocalMedia(){
	if (localStream&&localStream.getTracks()) {
		localStream.getTracks().forEach((track)=>{
			track.stop();	
		});
	}
	localStream = null;
}


function leave(){
	if(socket){
		socket.emit('leave',roomid);
	}
	
	//释放资源
	closePeerConnection();
	closeLocalMedia();
		
	btnConn.disabled = false;
	btnLeave.disabled = true;
}

//创建本地流媒体链接
function createPeerConnection(){
	console.log('create RTCPeerConnection!');
	if(!pc){
		pc = new RTCPeerConnection(pcConfig);
		pc.onicecandidate = (e) =>{
			if(e.candidate){
				sendMessage(roomid,{
					type:'candidate',
					label:e.candidate.sdpMLineIndex,
					id:e.candidate.sdpMid,
					candidate:e.candidate.candidate
				});
			}
		}
		pc.ontrack = (e)=>{
			remoteVideo.srcObject = e.streams[0];
		}
	}
	if(pc === null || pc === undefined){
			console.error('pc is null or undefined!');
			return;
	}

	if(localStream === null || localStream === undefined){
		console.error('localStream is null or undefined!');
		return;
	}

	if(localStream){
		localStream.getTracks().forEach((track)=>{
			pc.addTrack(track,localStream);
		})
	}
}

//关闭本地媒体流链接
function closePeerConnection(){
	console.log('close RTCPeerConnection!');
	if(pc){
		pc.close();
		pc = null;
	}
}

btnConn.onclick = connSignalServer;

btnLeave.onclick = leave;


