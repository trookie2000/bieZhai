
<script setup lang="ts">
import { ref, reactive, onBeforeMount, onMounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { confirm } from '@tauri-apps/api/dialog';
import { appWindow } from "@tauri-apps/api/window";

import {
  MouseStatus,
  WheelStatus,
  KeyboardStatus,
  MessageType,
  InputEventType,
} from "../common/Constans";
import { handleKeyboardEvent, handleMouseEvent } from "../common/InputEvent";
//可以显示多个画面但不能键鼠控制
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
  isConnecting: false,
});

// 对象用于引用视频元素，DOM对象s
const desktop = ref<HTMLVideoElement>();

// WebSocket 连接和RTC其他变量
let ws: WebSocket;
let pcs = reactive({}); // 使用对象存储多个RTCPeerConnection实例
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
  ws = new WebSocket(`ws://10.134.130.12:8081/conn/${data.account.id}`);
  ws.onopen = (e) => {
    setInterval(() => {
      sendToServer({
        msg_type: "heartbeat",
        receiver: "",
        sender: "",
        msg: "",
      });
    }, 60000);
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
const handleVideoOfferMsg = async (msg) => {
  const receiverId = msg.sender;
  const pc = await initRTCPeerConnection(receiverId);
  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc);
  const answer = await pc.createAnswer();
  await pc.setLocalDescription(answer);
  sendToServer({
    msg_type: MessageType.VIDEO_ANSWER,
    receiver: receiverId,
    msg: JSON.stringify(answer),
    sender: data.account.id,
  });
};

const handleVideoAnswerMsg = async (msg) => {
  const pc = pcs[msg.sender].connection;
  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc).catch(console.error);
};

const handleNewICECandidateMsg = async (msg) => {
  const pc = pcs[msg.sender].connection;
  const candidate = new RTCIceCandidate(JSON.parse(msg.msg));
  await pc.addIceCandidate(candidate).catch(console.error);
};


// 处理远程桌面请求消息
const handleRemoteDesktopRequest = async (msg) => {
  if (msg.msg !== data.account.password) {
    console.error("密码错误！");
    return;
  }
  const receiverId = msg.sender;
  const pc = await initRTCPeerConnection(receiverId);
  webcamStream = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: false });
  webcamStream.getTracks().forEach(track => pc.addTrack(track, webcamStream));
  sendOffer(pc, receiverId);
};

const initRTCPeerConnection = async (id) => {
  const pc = new RTCPeerConnection({
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      {
        urls: "turn:numb.viagenie.ca",
        username: "webrtc@live.com",
        credential: "muazkh",
      },
    ],
  });

  // 确保在 pcs[id] 初始化时包括 dpi 对象
  pcs[id] = {
    connection: pc,
    dpi: {width: undefined, height: undefined},  // 明确地初始化 DPI 值
  };

  pc.onicecandidate = (event) => {
    if (event.candidate) {
      sendToServer({
        msg_type: MessageType.NEW_ICE_CANDIDATE,
        receiver: id,
        msg: JSON.stringify(event.candidate),
        sender: data.account.id,
      });
    }
  };

  pc.ontrack = (event) => {
    handleTrackEvent(event, id);
  };

  pc.ondatachannel = (event) => {
    handleDataChannel(event.channel, id);
  };

  return pc;
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
// 在 handleTrackEvent 函数中使用 ref 的字符串形式来查找视频元素
const handleTrackEvent = (event, id) => {
  const stream = event.streams[0];
  videos.push({
    id,
    stream,
    name: `Video ${videos.length + 1}`
  });

  nextTick(() => {
    const videoElement = document.querySelector(`[id="videoElement${id}"]`);

    if (videoElement) {
      videoElement.srcObject = stream;
    } else {
      console.error('Video element not found for ID:', id);
    }
  });
};


const handleDataChannel = (dc, id) => {
  dc.onopen = () => console.log("数据通道已打开");
  dc.onmessage = event => {
    const msg = JSON.parse(event.data);
    if (msg.type === 'SET_DPI') {
      if (pcs[id]) {
        pcs[id].dpi = msg.dpi;  // 正确更新 DPI
        console.log(`DPI set for ${id}:`, pcs[id].dpi);
      } else {
        console.error(`DPI data received but no peer connection is available for id ${id}`);
      }
    } else if (msg.type === InputEventType.MOUSE_EVENT) {
      handleMouseEvent(msg.data, id);
    } else if (msg.type === InputEventType.KEY_EVENT) {
      handleKeyboardEvent(msg.data);
    }
  };
  dc.onclose = () => console.log("数据通道已关闭");
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

// 发送offer的实现，需要传递 RTCPeerConnection 实例
const sendOffer = async (pc, id) => {
  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  sendToServer({
    msg_type: MessageType.VIDEO_OFFER,
    receiver: id,
    msg: JSON.stringify(pc.localDescription),
    sender: data.account.id,
  });
};

/********************************* user event *************************************/

// 请求远程桌面
const remoteDesktop = async () => {
  appWindow.setFullscreen(false);
  data.isConnecting = true;
  setTimeout(() => {
    data.isShowRemoteDesktop = true;
  }, 0);
};

const closeRemoteDesktop = async () => {
  const confirmed = await confirm('确认关闭？', '提示');
  if (confirmed) {
    appWindow.setFullscreen(false);
    data.isShowRemoteDesktop = false;
    appWindow.close();
    closeAllConnections();
  }
};

const closeAllConnections = () => {
  Object.values(pcs).forEach(pc => {
    if (pc.connection && pc.connection.close) {
      pc.connection.close();
    }
  });
  pcs = {};
};

// 鼠标事件处理改动，传递事件对象和视频元素
const mouseDown = (e, videoElement, id) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_DOWN, id);
};

const mouseUp = (e, videoElement, id) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_UP, id);
};

const mouseMove = (e, videoElement, id) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_MOVE, id);
};

const wheel = (e, videoElement, id) => {
  const type = e.deltaY > 0 ? WheelStatus.WHEEL_DOWN : WheelStatus.WHEEL_UP;
  sendMouseEvent(e, videoElement, type, id);
};

const rightClick = (e, videoElement, id) => {
  e.preventDefault();  // 阻止默认的右键菜单
  sendMouseEvent(e, videoElement, MouseStatus.RIGHT_CLICK, id);
};

// 更新后的 sendMouseEvent 函数
const sendMouseEvent = (e, videoElement, eventType, id) => {
  if (!videoElement || !pcs[id] || !pcs[id].dpi || typeof pcs[id].dpi.width === 'undefined' || typeof pcs[id].dpi.height === 'undefined') {
    console.error('无法发送鼠标事件，视频元素或 DPI 数据未准备好或不完整');
    console.log(videoElement);
    console.log(pcs[id]);
    console.log(pcs[id].dpi);
    console.log(pcs[id].dpi.width);
    console.log(pcs[id].dpi.height);
    return;
  }
  const x = e.clientX;
  const y = e.clientY;
  const widthRatio = pcs[id].dpi.width / videoElement.clientWidth;
  const heightRatio = pcs[id].dpi.height / videoElement.clientHeight;

  const data = {
    x: Math.round(x * widthRatio),
    y: Math.round(y * heightRatio),
    eventType: eventType,
  };

  sendToClient({
    type: InputEventType.MOUSE_EVENT,
    data: data,
    receiver: id, // 确保发送到正确的接收者
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
const close = () => {
  if (desktop.value!.srcObject) {
    const tracks = desktop.value!.srcObject as MediaStream;
    tracks.getTracks().forEach((track: MediaStreamTrack) => track.stop());
    desktop.value!.srcObject = null;
  } else {
    webcamStream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
  }
  // 关闭 Peer 连接
  pc.close();
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

const addVideo = (stream, id) => {
  const videoObj = {
    stream,
    id,
    name: `Video ${videos.length + 1}`
  };
  videos.push(videoObj);
};


function toggleFullScreen(index) {
  const videoElement = videos[index].stream;
  if (!document.fullscreenElement && videoElement) {
    videoElement.requestFullscreen().catch(err => {
      console.error(`Error attempting to enable full-screen mode: ${err.message} (${err.name})`);
    });
  } else {
    if (document.fullscreenElement === videoElement) {
      document.exitFullscreen();
    }
  }
}
onMounted(() => {
  remoteDesktop(); // 在组件挂载时调用 remoteDesktop 方法
});
</script>
<template>
  <div class="container">
    <div class="video-grid">
      <div v-for="(video, index) in videos" :key="video.id" class="video-container" @click="setActiveVideo(video.id)">
        <video :ref="'videoElement' + video.id" :srcObject="video.stream" controls autoplay
          @mousedown="e => mouseDown(e, $refs['videoElement' + video.id], video.id)"
          @mouseup="e => mouseUp(e, $refs['videoElement' + video.id], video.id)"
          @mousemove="e => mouseMove(e, $refs['videoElement' + video.id], video.id)"
          @wheel="e => wheel(e, $refs['videoElement' + video.id], video.id)"
          @contextmenu.prevent="e => rightClick(e, $refs['videoElement' + video.id], video.id)">
        </video>
      </div>
    </div>
  </div>
</template>


<style lang="less" scoped>
.container {
  display: flex;
  flex-direction: column;
}
.sidebar {
  width: 200px;
  background-color: #f4f4f4;
  padding: 10px;
}

.main-content {
  flex-grow: 1;
}

.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  grid-gap: 10px;
}

.video-container {
  position:relative;
  padding-top: 56.25%; /* 16:9 Aspect Ratio */
  background: #000;
}

video {
  position: absolute;
  width: 100%;
  height: 100%;
  top: 0;
  left: 0;
}
</style>

