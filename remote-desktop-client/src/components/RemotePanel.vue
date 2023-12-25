<script setup lang="ts">
import { ref, reactive, onBeforeMount } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
// import TauriWebsocket from 'tauri-plugin-websocket-api';
// import WebSocket from "tauri-plugin-websocket-api";
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
});

// 对象用于引用视频元素，DOM对象
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
  ws = new WebSocket(`ws://172.20.64.1:8081/conn/${data.account.id}`);
 
  ws.onopen = (e: Event) => {
    // 向服务器发送心跳消息
    setInterval(() => {
      sendToServer({
        msg_type: "heartbeat",
        receiver: "",
        sender: "",
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
  if (msg.msg != data.account.password) {
    console.log("密码错误！");
    return;
  }

  data.receiverAccount.id = msg.sender;

  await initRTCPeerConnection();

  initRTCDataChannel();

  // 获取本地桌面流
  webcamStream = await navigator.mediaDevices.getDisplayMedia({
    video: true,
    audio: false,
  });

  webcamStream.getTracks().forEach((track: MediaStreamTrack) =>
    pc.addTrack(track, webcamStream)
  );

  sendOffer();
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
const handleTrackEvent = (event: RTCTrackEvent) => {
  desktop.value!.srcObject = event.streams[0];

  document.onkeydown = (e: KeyboardEvent) => {
    sendToClient({
      type: InputEventType.KEY_EVENT,
      data: {
        eventType: KeyboardStatus.MOUSE_DOWN,
        key: e.key,
      },
    });
  };

  document.onkeyup = (e: KeyboardEvent) => {
    sendToClient({
      type: InputEventType.KEY_EVENT,
      data: {
        eventType: KeyboardStatus.MOUSE_UP,
        key: e.key,
      },
    });
  };
};

// 数据通道事件处理
const handleDataChannel = (e: RTCDataChannelEvent) => {
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

// 发送视频通话邀请
const sendOffer = async () => {
  const offer = await pc.createOffer();

  await pc.setLocalDescription(offer);

  sendToServer({
    msg_type: MessageType.VIDEO_OFFER,
    receiver: data.receiverAccount.id,
    msg: JSON.stringify(pc.localDescription),
    sender: data.account.id,
  });
};

/********************************* user event *************************************/

// 请求远程桌面
const remoteDesktop = async () => {
  if (!data.receiverAccount.id || !data.receiverAccount.password) {
    alert("请输入ID和密码");
    return;
  }

  appWindow.setFullscreen(true);

  // 显示远程桌面面板
  data.isShowRemoteDesktop = true;

  //将下下哦i发给被远程者1
  sendToServer({
    msg_type: MessageType.REMOTE_DESKTOP,
    receiver: data.receiverAccount.id,
    msg: data.receiverAccount.password,
    sender: data.account.id,
  });
};

// 关闭远程桌面
const closeRemoteDesktop = async () => {
  appWindow.setFullscreen(false);
  data.isShowRemoteDesktop = false;

  close();

  sendToServer({
    msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
    receiver: data.receiverAccount.id,
    msg: data.receiverAccount.password,
    sender: data.account.id,
  });
};

// 鼠标按下事件处理
const mouseDown = (e: MouseEvent) => {
  sendMouseEvent(e.x, e.y, mouseType(MouseStatus.MOUSE_DOWN, e.button));
};

// 鼠标松开事件处理
const mouseUp = (e: MouseEvent) => {
  sendMouseEvent(e.x, e.y, mouseType(MouseStatus.MOUSE_UP, e.button));
};

// 滚轮事件处理
const wheel = (e: WheelEvent) => {
  let type = e.deltaY > 0 ? WheelStatus.WHEEL_UP : WheelStatus.WHEEL_DOWN;
  sendMouseEvent(e.x, e.y, type);
};

// 鼠标移动事件处理
const mouseMove = (e: MouseEvent) => {
  sendMouseEvent(e.x, e.y, MouseStatus.MOUSE_MOVE);
};

// 鼠标右键单击事件处理
const rightClick = (e: MouseEvent) => {
  sendMouseEvent(e.x, e.y, MouseStatus.RIGHT_CLICK);
};

/********************************* common *************************************/

// 发送鼠标事件
const sendMouseEvent = (x: number, y: number, eventType: string) => {
  if (remoteDesktopDpi) {
    let widthRatio = remoteDesktopDpi.width / desktop.value!.clientWidth;
    let heightRatio = remoteDesktopDpi.height / desktop.value!.clientHeight;

    let data = {
      x: parseInt((x * widthRatio).toFixed(0)),
      y: parseInt((y * heightRatio).toFixed(0)),
      eventType: eventType,
    };
    sendToClient({
      type: InputEventType.MOUSE_EVENT,
      data: data,
    });
  }
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

</script>

<template>
  <div class="sidebar">
    <div>
      <p>
        id: <span>{{ data.account.id }}</span>
      </p>
      <p>
        password: <span>{{ data.account.password }}</span>
      </p>
    </div>
  </div>
  <div class="form">
    <input v-model="data.receiverAccount.id" type="text" placeholder="请输入对方id" />
    <input v-model="data.receiverAccount.password" type="text" placeholder="请输入对方密码" />
    <button @click="remoteDesktop()">发起远程</button>
  </div>
  <video v-show="data.isShowRemoteDesktop" @mousedown="mouseDown($event)" @mouseup="mouseUp($event)"
    @mousemove="mouseMove($event)" @wheel="wheel($event)" @contextmenu.prevent="rightClick($event)" class="desktop"
    ref="desktop" autoplay></video>
  <button v-if="data.isShowRemoteDesktop" class="close-btn" @click="closeRemoteDesktop()">
    关闭
  </button>
</template>

<style lang="less" scoped>
.sidebar {
  width: 100%;
  height: 160px;
  background: #1b1b1c;
  color: #fafafa;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  border-bottom: 1px solid #252525;
  box-sizing: border-box;

  >div {
    background: #242425;
    padding: 10px 20px;
    border-radius: 10px;

    p {
      line-height: 28px;
      font-size: 16px;

      span {
        font-size: 18px;
        font-weight: 600;
      }
    }
  }
}

.form {
  height: calc(100% - 160px);
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background: #1b1b1c;

  button {
    width: 280px;
    height: 34px;
    background: #00c1cd;
  }
}

input {
  width: 280px;
  outline: none;
  border: 1px solid #252525;
  padding: 0 10px;
  height: 34px;
  box-sizing: border-box;
  border-radius: 5px;
  margin-bottom: 30px;
}

button {
  outline: none;
  border: none;
  color: #fff;
  border-radius: 5px;
}

.desktop {
  width: 100%;
  height: 100%;
  position: fixed;
  top: 0;
  left: 0;
  background: #121212;
  cursor: none;
}

.close-btn {
  width: 40px;
  height: 24px;
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 1;
  background: #d71526;
  font-size: 12px;
}
</style>
