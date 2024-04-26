
<script setup lang="ts">
import { ref, reactive, onBeforeMount, onMounted, nextTick, watch, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { confirm } from '@tauri-apps/api/dialog';
import { appWindow, WebviewWindow } from "@tauri-apps/api/window";

import {
  MouseStatus,
  WheelStatus,
  KeyboardStatus,
  MessageType,
  InputEventType,
} from "../common/Constans";
import { handleKeyboardEvent, handleMouseEvent } from "../common/InputEvent";

// 用于存储响应式数据的对象
const data = reactive({
  account: {
    id: "",
    password: "",
  },
  receiverAccount: {
    id: "",
    password: "",
  },
  isShowRemoteDesktop: false,
  isConnecting: false, //连接状态
});

// 对象用于引用视频元素，DOM对象s
const desktop = ref<HTMLVideoElement>();

// WebSocket 连接和RTC其他变量
let ws: WebSocket;
let pc: RTCPeerConnection;
let dc: RTCDataChannel;
let webcamStream: MediaStream;
//分辨率
let remoteDesktopDpi: Record<string, any>;

// 在组件挂载之前执行的异步操作
onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});

/********************************* connect *************************************/

// 初始化 WebSocket 连接
const initWebSocket = () => {
  ws = new WebSocket(`ws://10.134.169.24:8081/conn/${data.account.id}`);

  ws.onopen = (e: Event) => {
    // 向服务器发送心跳消息
    setInterval(() => {
      sendToServer({
        msg_type: "heartbeat",
        receiver: "",
        sender: "",
        // videoId:"",
        msg: "",
      });
    }, 1000 * 60);
  };

  ws.onmessage = async (e: MessageEvent) => {
    const msg: Record<string, any> = JSON.parse(e.data);
    switch (msg.msg_type) {
      case MessageType.VIDEO_OFFER: // 视频通话邀请
        handleVideoOfferMsg(msg);
        break;
      case MessageType.VIDEO_ANSWER: // 对方已接受邀请
        handleVideoAnswerMsg(msg);
        break;
      case MessageType.NEW_ICE_CANDIDATE: // 收到新的 ICE 候选项
        handleNewICECandidateMsg(msg);
        break;
      case MessageType.REMOTE_DESKTOP:
        handleRemoteDesktopRequest(msg);
        break;
      case MessageType.CLOSE_REMOTE_DESKTOP:
        close();
        break;
    }
  };

  ws.onerror = (e: Event) => {
    console.log("WebSocket 连接错误:", e);
  };
};

// 处理视频邀请消息
const handleVideoOfferMsg = async (msg: Record<string, any>) => {
  data.receiverAccount.id = msg.sender;

  await initRTCPeerConnection();

  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc);

  await pc.setLocalDescription(await pc.createAnswer());
  sendToServer({
    msg_type: MessageType.VIDEO_ANSWER,
    receiver: data.receiverAccount.id,
    msg: JSON.stringify(pc.localDescription),
    sender: data.account.id,
  });
};

// 处理视频回应消息
const handleVideoAnswerMsg = async (msg: Record<string, any>) => {
  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc).catch(reportError);
};

// 处理新的 ICE 候选项消息
const handleNewICECandidateMsg = async (msg: Record<string, any>) => {
  const candidate = new RTCIceCandidate(JSON.parse(msg.msg));
  try {
    await pc.addIceCandidate(candidate);
  } catch (err) {
    reportError(err);
  }
};

// 处理远程桌面请求消息
const handleRemoteDesktopRequest = async (msg: Record<string, any>) => {
  try {
    if (msg.msg != data.account.password) {
      console.log("密码错误！");
      return;
    }

    data.receiverAccount.id = msg.sender;

    // 初始化单独的 RTCPeerConnection
    const peerConnection = await initRTCPeerConnection();

    // 添加 Peer 连接到视频对象中
    addVideoPeerConnection(peerConnection);

    initRTCDataChannel();

    // 获取本地桌面流
    webcamStream = await navigator.mediaDevices.getDisplayMedia({
      video: true,
      audio: false,
    });

    webcamStream.getTracks().forEach((track: MediaStreamTrack) =>
      peerConnection.addTrack(track, webcamStream)
    );

    sendOffer(peerConnection);
  } catch (error) {
    console.error("处理远程桌面请求时出错:", error);
    // 在发生错误时需要重置连接状态
    data.isConnecting = false;
  }
};

// 初始化 RTCPeerConnections
const initRTCPeerConnection = () => {
  const iceServer: object = {
    iceServers: [
      {
        url: "stun:stun.l.google.com:19302",
      },
      {
        url: "turn:numb.viagenie.ca",
        username: "webrtc@live.com",
        credential: "muazkh",
      },
    ],
  };

  pc = new RTCPeerConnection(iceServer);

  pc.onicecandidate = handleICECandidateEvent;
  pc.oniceconnectionstatechange = handleICEConnectionStateChangeEvent;
  pc.onicegatheringstatechange = handleICEGatheringStateChangeEvent;
  pc.onsignalingstatechange = handleSignalingStateChangeEvent;
  pc.ontrack = handleTrackEvent;
  pc.ondatachannel = handleDataChannel;
};
const addVideoPeerConnection = (peerConnection) => {
  // 找到最后一个视频对象并添加 Peer 连接
  const lastIndex = videos.length - 1;
  if (lastIndex >= 0) {
    videos[lastIndex].peerConnection = peerConnection;
  }
};

// 处理 ICE 候选项事件
const handleICECandidateEvent = (event: RTCPeerConnectionIceEvent) => {
  if (event.candidate) {
    sendToServer({
      msg_type: MessageType.NEW_ICE_CANDIDATE,
      receiver: data.receiverAccount.id,
      msg: JSON.stringify(event.candidate),
      sender: data.account.id,
    });
  }
};

// 处理 ICE 连接状态变化事件
const handleICEConnectionStateChangeEvent = (event: Event) => {
  console.log("*** ICE 连接状态变为" + pc.iceConnectionState);
};

// 处理 ICE 聚集状态变化事件
const handleICEGatheringStateChangeEvent = (event: Event) => {
  console.log("*** ICE 聚集状态变为" + pc.iceGatheringState);
};

// 处理 WebRTC 信令状态变化事件
const handleSignalingStateChangeEvent = (event: Event) => {
  console.log("*** WebRTC 信令状态变为: " + pc.signalingState);
};

// 获取数据流事件处理
// 处理新视频流事件
const handleTrackEvent = (event) => {
  const stream = event.streams[0];
  addVideo(stream, `Video ${videos.length + 1}`);

  // 使用 nextTick 来确保 DOM 更新
  nextTick(() => {
    const elems = videoElements.value; // 确保你有正确的 ref 指向视频元素数组
    const elem = elems[elems.length - 1];
    if (elem) {
      elem.srcObject = stream;
    } else {
      console.error('Video element not found');
    }
  });
};


// 数据通道事件处理
const handleDataChannel = (e: RTCDataChannelEvent) => {
  data.isConnecting = false;
  dc = e.channel;
  dc.onopen = (e: Event) => {
    console.log("数据通道已打开");
  };

  dc.onmessage = (event: MessageEvent) => {
    remoteDesktopDpi = JSON.parse(event.data);
  };

  dc.onclose = (e: Event) => {
    console.log("数据通道已关闭");
  };

  console.log("数据通道:", dc);
};

// 初始化 WebRTC 数据通道
const initRTCDataChannel = () => {
  dc = pc.createDataChannel("my channel", {
    ordered: true,
  });

  //计算分辨率，鼠标属于哪个位置
  dc.onopen = (e: Event) => {
    console.log("数据通道已打开");
    dc.send(
      JSON.stringify({
        width: window.screen.width * window.devicePixelRatio,
        height: window.screen.height * window.devicePixelRatio,
      })
    );
    console.log("数据通道:", dc);
  };

  dc.onmessage = (event: MessageEvent) => {
    let msg: Record<string, any> = JSON.parse(event.data);
    switch (msg.type) {
      case InputEventType.MOUSE_EVENT:
        handleMouseEvent(msg.data);
        break;
      case InputEventType.KEY_EVENT:
        handleKeyboardEvent(msg.data);
        break;
    }
  };

  dc.onclose = (e: Event) => {
    console.log("数据通道已关闭");
  };

  console.log("数据通道:", dc);
};

// 发送共享桌面邀请
const sendOffer = async (peerConnection) => { // 接收 Peer 连接作为参数
  const offer = await peerConnection.createOffer();

  await peerConnection.setLocalDescription(offer);

  sendToServer({
    msg_type: MessageType.VIDEO_OFFER,
    receiver: data.receiverAccount.id,
    msg: JSON.stringify(peerConnection.localDescription),
    sender: data.account.id,
  });
};

/********************************* user event *************************************/

// 请求远程桌面
const remoteDesktop = async () => {
  appWindow.setFullscreen(false);
  // 显示远程桌面面板
  data.isConnecting = true;
  setTimeout(() => {
    data.isShowRemoteDesktop = true;
  }, 0);
};

// // 关闭远程桌面
// const closeRemoteDesktop = async () => {
//   const confirmed = await confirm('是否确认关闭', '提示');
//   if (confirmed) {
//     appWindow.setFullscreen(false);
//     data.isShowRemoteDesktop = false;
//     appWindow.close();
//     close();
//     sendToServer({
//       msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
//       receiver: data.receiverAccount.id,
//       msg: data.receiverAccount.password,
//       sender: data.account.id,
//     });
//   }
// };
// const closeVideo = (video) => {
//   console.log("Closing video with ID:", video.id);
//   close();
//   // 停止视频流
//   const videoStream = video.stream;
//   if (videoStream) {
//     videoStream.getTracks().forEach(track => {
//       console.log("Stopping track:", track.id);
//       track.stop();  // 停止该流的所有轨道
//     });
  
//   }
  
//   // 从数组中移除该视频对象
//   const index = videos.findIndex(v => v.id === video.id);
//   if (index !== -1) {
//     videos.splice(index, 1);
//     sendToServer({
//       msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
//       receiver: data.receiverAccount.id,
//       msg: data.receiverAccount.password,
//       sender: data.account.id,
//     });
//   }
// };
const closeVideo = (video) => {
  console.log("Closing video with ID:", video.id);
  const peerConnection = video.peerConnection;
  if (peerConnection) {
    close(peerConnection);
  }
  // 从数组中移除该视频对象
  const index = videos.findIndex((v) => v.id === video.id);
  if (index !== -1) {
    videos.splice(index, 1);
    sendToServer({
      msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
      receiver: data.receiverAccount.id,
      msg: data.receiverAccount.password,
      sender: data.account.id,
    });
  }
};


// 鼠标事件处理改动，传递事件对象和视频元素
const mouseDown = (e, videoElement) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_DOWN);
};

const mouseUp = (e, videoElement) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_UP);
};

const mouseMove = (e, videoElement) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_MOVE);
};

const wheel = (e, videoElement) => {
  const type = e.deltaY > 0 ? WheelStatus.WHEEL_DOWN : WheelStatus.WHEEL_UP;
  sendMouseEvent(e, videoElement, type);
};


// 更新后的 sendMouseEvent 函数
const sendMouseEvent = (e, videoElement, eventType) => {
  const x = e.clientX;
  const y = e.clientY;
  if (!videoElement) return;

  const widthRatio = remoteDesktopDpi.width / videoElement.clientWidth;
  const heightRatio = remoteDesktopDpi.height / videoElement.clientHeight;

  const data = {
    x: Math.round(x * widthRatio),
    y: Math.round(y * heightRatio),
    eventType: eventType,
  };

  sendToClient({
    type: InputEventType.MOUSE_EVENT,
    data: data,
  });
};


// 获取鼠标事件类型
const mouseType = (mouseStatus: MouseStatus, button: number) => {
  let type = "";
  switch (button) {
    case 0:
      type = "left-" + mouseStatus;
      break;
    case 2:
      type = "right-" + mouseStatus;
      break;
    // TODO 更多的按钮
  }

  return type;
};

// 关闭远程桌面
const close = (peerConnection) => {
  if (peerConnection) {
    peerConnection.getSenders().forEach(sender => {
      peerConnection.removeTrack(sender);
    });
    peerConnection.close();
  }
};
const videoElements = ref([]);

// 发送消息给服务器
const sendToServer = (msg: Record<string, any>) => {
  let msgJSON = JSON.stringify(msg);
  ws.send(msgJSON);
};

// 发送消息给客户端
const sendToClient = (msg: Record<string, any>) => {
  let msgJSON = JSON.stringify(msg);
  dc.readyState == "open" && dc.send(msgJSON);
};
const videos = reactive([]);
let activeVideoIndex = ref(null);
const setActiveVideo = (index) => {
  activeVideoIndex.value = index;
};

const addVideo = (stream) => {
  const videoObj = {
    id: Date.now().toString(), // 使用时间戳生成唯一ID
    stream,
    name: `Video ${videos.length + 1}`,
    peerConnection: null // 新增属性存储 Peer 连接
  };
  videos.push(videoObj);
};

const toggleFullScreen = (videoElement, video, index) => {
  if (!document.fullscreenElement) {
    videoElement.requestFullscreen().then(() => {
      videos[index].isFullscreen = true;  // 设置全屏标志
      handleFullscreenChange(); // 触发全屏状态变化处理函数
    }).catch(err => {
      console.error(`Error attempting to enable full-screen mode: ${err.message} (${err.name})`);
    });
  } else {
    document.exitFullscreen().then(() => {
      videos.forEach(v => v.isFullscreen = false);  // 清除所有全屏标志
      handleFullscreenChange(); // 触发全屏状态变化处理函数
    }).catch(err => {
      console.error(`Error attempting to disable full-screen mode: ${err.message} (${err.name})`);
    });
  }
};


// const handleClick(event, video) => {
//       if (video.paused) {
//         video.play(); // 如果视频暂停，则播放视频
//       }
//     },
// 添加变量用于跟踪视频是否在全屏模式下
const isVideoFullscreen = ref(false);

// 监听视频全屏状态变化
document.addEventListener('fullscreenchange', handleFullscreenChange);

// 处理视频全屏状态变化
function handleFullscreenChange() {
  isVideoFullscreen.value = document.fullscreenElement !== null;

  // 更新按钮位置和样式
  const buttons = document.querySelectorAll('.close-btn');
  buttons.forEach(button => {
    if (isVideoFullscreen.value) {
      button.style.bottom = '20px';
      button.style.right = '20px';
    } else {
      button.style.bottom = '5%';
      button.style.right = '5%';
    }
  });
}

document.addEventListener('fullscreenchange', handleFullscreenChange);

// 调整按钮位置以适应全屏模式
function adjustButtonPositionForFullscreen() {
  const buttons = document.querySelectorAll('.close-btn');
  buttons.forEach(button => {
    button.style.bottom = '5%';
    button.style.right = '5%';
    button.style.transform = 'translate(50%, 50%)';
  });
}

// 从全屏模式恢复按钮位置
function restoreButtonPositionFromFullscreen() {
  const buttons = document.querySelectorAll('.close-btn');
  buttons.forEach(button => {
    button.style.bottom = '';
    button.style.right = '';
    button.style.transform = '';
  });
}
onMounted(() => {
  remoteDesktop(); // 在组件挂载时调用 remoteDesktop 方法
});
onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', handleFullscreenChange);
});
</script>
<template>
  <div class="container">
    <div class="video-grid">
      <div v-for="(video, index) in videos" :key="video.id" class="video-container"
        :class="{ 'fullscreen': video.isFullscreen }" @click="setActiveVideo(index)">
        <div class="video-wrapper">
          <video v-show="data.isShowRemoteDesktop " class="video" ref="videoElements" :srcObject="video.stream" autoplay controls
            @mousedown="e => mouseDown(e, $refs.videoElements[index])"
            @mouseup="e => mouseUp(e, $refs.videoElements[index])"
            @mousemove="e => mouseMove(e, $refs.videoElements[index])"
            @wheel="e => wheel(e, $refs.videoElements[index])"
            @contextmenu.prevent="e => rightClick(e, $refs.videoElements[index])"
            @dblclick="e => toggleFullScreen($refs.videoElements[index], video, index)" 
            x5-video-player-type="h5-page"></video>
          <button v-if="data.isShowRemoteDesktop" class="close-btn" @click="closeVideo(video)">
            关闭
          </button>
        </div>
      </div>
    </div>
  </div>
</template>


<style lang="less" scoped>
.container {
  display: flex;
}

.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  grid-gap: 10px;
}

.video-container {
  position: relative;
  width: 100%;
}

.video-wrapper {
  position: relative;
  width: 100%;
}

video {
  display: block;
  width: 100%;
  height: auto;
}

.close-btn {
  position: absolute;
  z-index: 999;
  background: #d71526;
  font-size: 12px;
  bottom: 5%;
  right: 5%;
}

.close-btn.fullscreen {
  bottom: 20px;
  right: 20px;
  transform: translate(50%, 50%);
}

/* 隐藏video 播放按钮 */
/* 
.video::-webkit-media-controls-play-button {
  pointer-events: none; 
}
.video::-webkit-media-controls-start-playback-button {
  pointer-events: none; 
}
.video::-webkit-media-controls-enclosure { 
  pointer-events: none; 
}

.video::-webkit-media-controls-timeline {
  display: none;
}

.video::-webkit-media-controls-current-time-display {
  display: none;            
}

.video::-webkit-media-controls-time-remaining-display {
  display: none;            
}

.video::-webkit-media-controls-mute-button {
  display: none;            
}
.video::-webkit-media-controls-toggle-closed-captions-button {
  display: none;            
}

.video::-webkit-media-controls-volume-slider {
  display: none;            
}

*/
</style>