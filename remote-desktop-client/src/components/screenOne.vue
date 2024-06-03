<script setup lang="ts">
import {
  ref,
  reactive,
  onBeforeMount,
  onMounted,
  nextTick,
  watch,
  onBeforeUnmount,
  onUnmounted,
  computed,
} from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { confirm } from "@tauri-apps/api/dialog";
import { appWindow, WebviewWindow } from "@tauri-apps/api/window";
import {
  handleGetTopWindowInfo,
} from "../common/InputEvent";
import {
  MouseStatus,
  WheelStatus,
  KeyboardStatus,
  MessageType,
  InputEventType,
} from "../common/Constans";
import {
  handleKeyboardEvent,
  handleMouseEvent,
  handleWindowTop,
} from "../common/InputEvent";

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
  clearWindowInfoInterval: null as (() => void) | null, //s
});

// WebSocket 连接和RTC其他变量
let ws: WebSocket;
let pc: RTCPeerConnection;
let dc: RTCDataChannel;
let webcamStream: MediaStream;
let webcamStreamArr: MediaStream[] = [];
//分辨率
let remoteDesktopDpi: Record<string, any>;
let unlisten: Function | null = null;
// 在组件挂载之前执行的异步操作
onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});
onMounted(() => {
  remoteDesktop(); // 在组件挂载时调用 remoteDesktop 方法
  appWindow
    .onCloseRequested(async (event) => {
      event.preventDefault();
      closeRemoteDesktop();
    })
    .then((unlistenFn: Function) => {
      unlisten = unlistenFn;
    });
});
interface Video {
  id: string;
  stream: any;
  receiverAccount: {
    id: string;
    password: string;
  };
  name: string;
}

onBeforeUnmount(() => {
  document.removeEventListener("fullscreenchange", handleFullscreenChange);
});
onUnmounted(() => {
  if (unlisten) {
    unlisten();
  }
});
const activeDeviceId = ref<string | null>(null);

const groupedVideos = computed((): Record<string, Video[]> => {
  const groups: Record<string, Video[]> = {};
  videos.forEach((video: Video) => {
    const deviceId = video.receiverAccount.id;
    if (!groups[deviceId]) {
      groups[deviceId] = [];
    }
    groups[deviceId].push(video);
  });
  return groups;
});


const toggleDevice = (deviceId: string) => {
  activeDeviceId.value = activeDeviceId.value === deviceId ? null : deviceId;
};
// 关闭远程桌面
const closeRemoteDesktop = async () => {
  const confirmed = await confirm("确认？", "提示");
  if (confirmed) {
    appWindow.setFullscreen(false);
    data.isShowRemoteDesktop = false;
    appWindow.close();

    // 停止并移除所有的视频流
    webcamStreamArr.forEach((stream) => {
      stream.getTracks().forEach((track) => {
        track.stop();
      });
    });
    webcamStreamArr = []; // 清空video数组
    close();
    sendToServer({
      msg_type: MessageType.STOP_SHARING,
      receiver: data.receiverAccount.id,
      msg: data.receiverAccount.password,
      sender: data.account.id,
    });
  }
};
/********************************* connect *************************************/

// 初始化 WebSocket 连接
const initWebSocket = () => {
  ws = new WebSocket(`ws://192.168.1.101:8081/conn/${data.account.id}`);

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
      case MessageType.STOP_SHARING:
        closeVideoByMacAddress(msg);
        break;
    }
  };

  ws.onerror = (e: Event) => {
    console.log("WebSocket 连接错误:", e);
  };

};
//当共享方关闭漂浮栏中的按钮后通知远控方video关闭
function closeVideoByMacAddress(msg: Record<string, any>) {
  const id = JSON.parse(msg.msg).id;
  const video = videos.find((item: any) => {
    return item.stream.id == id;
  });

  console.log(videos);

  closeVideo(video);
}

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

    await initRTCPeerConnection();

    initRTCDataChannel();

    // 获取本地桌面流
    webcamStream = await navigator.mediaDevices.getDisplayMedia({
      video: true,
      audio: false,
    });

    webcamStream
      .getTracks()
      .forEach((track: MediaStreamTrack) => pc.addTrack(track, webcamStream));

    sendOffer();
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
const handleTrackEvent = (event: any) => {
  console.log(event.streams);

  const stream = event.streams[0];
  addVideo(stream);

  // 使用 nextTick 来确保 DOM 更新
  nextTick(() => {
    const elems = videoElements.value; // 确保你有正确的 ref 指向视频元素数组
    const elem: any = elems[elems.length - 1];
    if (elem) {
      elem.srcObject = stream;
    } else {
      console.error("Video element not found");
    }

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
    const { id, name, width, height, left, right, top, bottom } = JSON.parse(
      event.data
    );


    //窗体位置发生变化的应对
    const video = videos.find((v: any) => v.stream.id == id);
    video.name = name;
    video.streamId = id;
    video.width = width;
    video.height = height;
    video.left = left;
    video.right = right;
    video.top = top;
    video.bottom = bottom;
    remoteDesktopDpi = {
      width,
      height,
      left,
      right,
      top,
      bottom,
    };
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
  dc.onopen = async (e: Event) => {
  data.isConnecting = true;
  console.log("数据通道已打开");

  const sendWindowInfo = async () => {
    const windInfo: any = await handleGetTopWindowInfo();

    let w;
    let h;
    if (windInfo.name.includes("正在共享你的屏幕")) {
      w = window.screen.width;
      h = window.screen.height;
    } else {
      w = windInfo.width;
      h = windInfo.height;
    }

    console.log(webcamStreamArr[webcamStreamArr.length - 1]);

    dc.send(
      JSON.stringify({
        id: webcamStreamArr[webcamStreamArr.length - 1].id,
        name: windInfo.name,
        width: w * window.devicePixelRatio,
        height: h * window.devicePixelRatio,
        left: windInfo.left,
        right: windInfo.right,
        top: windInfo.top,
        bottom: windInfo.bottom,
      })
    );

    // 更新 videos 数组中的名称
    const video = videos.find((v: any) => v.id === webcamStreamArr[webcamStreamArr.length - 1].id);
    if (video) {
      video.name = windInfo.name;
    }

    console.log("数据通道:", dc);
  };

  // 初次发送窗口信息
  await sendWindowInfo();

  // 设置定时器定期发送窗口信息
  const intervalId = setInterval(sendWindowInfo, 1000); // 每隔一秒发送一次窗口信息

  // 清除定时器的方法（可在需要时调用，例如断开连接时）
  const clearWindowInfoInterval = () => {
    clearInterval(intervalId);
  };

  // 将清除定时器的方法存储到 data 对象中
  data.clearWindowInfoInterval = clearWindowInfoInterval;
};

// 示例：在连接断开时清除定时器
dc.onclose = (e: Event) => {
  console.log("数据通道已关闭");
  if (data.clearWindowInfoInterval) {
    data.clearWindowInfoInterval();
  }
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
  appWindow.setFullscreen(false);
  // 显示远程桌面面板
  data.isConnecting = true;
  setTimeout(() => {
    data.isShowRemoteDesktop = true;
  }, 0);
};

const closeVideo = (video: any) => {
  console.log("Closing video with ID:", videos);

  // 从数组中移除该视频对象
  const index = videos.findIndex((v: any) => v.id === video.id);
  if (index !== -1) {
    sendToServer({
      msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
      receiver: data.receiverAccount.id,
      msg: JSON.stringify({
        password: data.receiverAccount.password,
        id: video.stream.id,
      }),
      sender: data.account.id,
    });

    videos.splice(index, 1);

    // 如果关闭的是当前显示的视频，重置activeVideoIndex
    if (activeVideoIndex.value === index) {
      activeVideoIndex.value = null;
    }
  }
};


// 鼠标事件处理改动，传递事件对象和视频元素
const mouseDown = (e: any, videoElement: any) => {
  sendMouseEvent(e, videoElement, mouseType(MouseStatus.MOUSE_DOWN, e.button));
};

const mouseUp = (e: any, videoElement: any) => {
  sendMouseEvent(e, videoElement, mouseType(MouseStatus.MOUSE_UP, e.button));
};

const mouseMove = (e: any, videoElement: any) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_MOVE);
};

const wheel = (e: any, videoElement: any) => {
  const type = e.deltaY > 0 ? WheelStatus.WHEEL_DOWN : WheelStatus.WHEEL_UP;
  sendMouseEvent(e, videoElement, type);
};

const rightClick = (e: any, videoElement: any) => {
  sendMouseEvent(e, videoElement, MouseStatus.RIGHT_CLICK);
};

// 更新后的 sendMouseEvent 函数
let lastMouseX = 0;
let lastMouseY = 0;
let lastTimestamp = 0;
const sendMouseEvent = (
  e: MouseEvent,
  videoElement: HTMLVideoElement,
  eventType: string
) => {
  if (!videoElement) return;

  // 获取远程桌面的实际尺寸
  const desktopWidth = remoteDesktopDpi.width;
  const desktopHeight = remoteDesktopDpi.height;

  console.log(remoteDesktopDpi);

  // 计算鼠标相对于视频元素的位置
  const xRatio = desktopWidth / videoElement.offsetWidth;
  const yRatio = desktopHeight / videoElement.offsetHeight;
  const x = Math.round(e.offsetX * xRatio) + remoteDesktopDpi.left;
  const y = Math.round(e.offsetY * yRatio) + remoteDesktopDpi.top;

  console.log(x);
  console.log(y);

  // 计算鼠标移动的速度
  const now = performance.now();
  const deltaTime = now - lastTimestamp;
  const deltaX = x - lastMouseX;
  const deltaY = y - lastMouseY;
  const speedX = deltaX / deltaTime;
  const speedY = deltaY / deltaTime;

  // 发送鼠标事件给客户端
  sendToClient({
    type: InputEventType.MOUSE_EVENT,
    data: {
      x: x,
      y: y,
      speedX: speedX,
      speedY: speedY,
      eventType: eventType,
    },
  });

  // 更新上一次鼠标位置和时间戳
  lastMouseX = x;
  lastMouseY = y;
  lastTimestamp = now;
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

const videoElements = ref<any[]>([]);

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
const videos:any = reactive<Video[]>([]);
//
watch(
  videos,
  (value) => {
    const videos = document.querySelectorAll("video");
    videos.forEach((video: any) => {
      const clickHandler = (event: any) => {
        event.preventDefault();
      };
      const keydownHandler = (event: any) => {
        if (event.keyCode === 32) {
          video.pause();
          event.preventDefault();
        }
        if (event.keyCode === 13) {
          event.preventDefault();
        }
      };
      video.addEventListener("click", clickHandler);
      video.addEventListener("keydown", keydownHandler);

      // 保存事件处理函数，以便稍后移除
      video.__clickHandler = clickHandler;
      video.__keydownHandler = keydownHandler;
    });
  },
  {
    deep: true,
  }
);
videos.forEach((video: any) => {
  video.removeEventListener("click", video.__clickHandler);
  video.removeEventListener("keydown", video.__keydownHandler);
});

let activeVideoIndex = ref<number | null>(null);
const setActiveVideo = (index: number) => {
  activeVideoIndex.value = index;
};

const showVideo = (video: Video) => {
  activeVideoIndex.value = videos.findIndex((v: Video) => v.id === video.id);
};


const addVideo = (stream: any) => {
  const videoObj = {
    id: Date.now().toString(),
    stream,
    receiverAccount: { id: data.receiverAccount.id }, // 确保包含 receiverAccount.id
    name: `Video ${videos.length + 1}`,
  };
  videos.push(videoObj);
};

const isVideoFullscreen = ref(true);
const setWindowTop = (video: any) => {
  // 保留原来的 video.name
  const windowTitle = video.name;
  console.log("Setting window top with title:", windowTitle);

  // 只更新需要的属性，不修改 video.name
  remoteDesktopDpi = {
    width: video.width,
    height: video.height,
    left: video.left,
    right: video.right,
    top: video.top,
    bottom: video.bottom,
  };

  sendToClient({
    type: InputEventType.WINDOW_EVENT,
    data: {
      windowTitle: windowTitle + "\0",
    },
  });
};



// 监听视频全屏状态变化
document.addEventListener("fullscreenchange", handleFullscreenChange);

// 处理视频全屏状态变化
function handleFullscreenChange() {
  isVideoFullscreen.value = document.fullscreenElement !== null;

  // 更新按钮位置和样式
  const buttons = document.querySelectorAll(".close-btn");
  buttons.forEach((button: any) => {
    if (isVideoFullscreen.value) {
      button.style.bottom = "20px";
      button.style.right = "20px";
    } else {
      button.style.bottom = "5%";
      button.style.right = "5%";
    }
  });
}

document.addEventListener("fullscreenchange", handleFullscreenChange);
</script>
<template>
  <div class="container">
    <!-- 新增列表部分 -->
    <div class="video-list">
      <ul>
        <li v-for="(videos, deviceId) in groupedVideos" :key="deviceId" class="device-item">
          <div @click="toggleDevice(deviceId)" class="device-name">
            <i class="icon fas fa-desktop"></i>{{ deviceId }}
          </div>
          <ul v-show="activeDeviceId === deviceId" class="sub-list">
            <li v-for="video in videos" :key="video.id" class="video-item">
              <div class="video-item-content">
                <span @click="() => { showVideo(video); setWindowTop(video); }" class="video-name">
                  <i class="icon fas fa-video"></i>{{ video.name }}
                </span>
                <span @click="closeVideo(video)" class="close-btn">
                  <i class="icon fas fa-times"></i>
                </span>
              </div>
              
            </li>
          </ul>
        </li>
      </ul>
    </div>

    <div class="video-grid">
      <div v-for="(video, index) in videos" :key="video.id" class="video-wrapper" :class="{ isTop: video.isTop }"
        v-show="activeVideoIndex === index">
        <div class="video-container">
          <video class="video" ref="videoElements" :srcObject="video.stream"
            @mousedown="(e) => mouseDown(e, ($refs.videoElements as any[])[index])"
            @mouseup="(e) => mouseUp(e, ($refs.videoElements as any[])[index])"
            @mousemove="(e) => mouseMove(e, ($refs.videoElements as any[])[index])"
            @wheel="(e) => wheel(e, ($refs.videoElements as any[])[index])" @contextmenu.prevent="(e) => rightClick(e, ($refs.videoElements as any[])[index])
        " x5-video-player-type="h5-page" autoplay controls></video>
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="less" scoped>
@import url('https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.0.0-beta3/css/all.min.css');

.container {
  display: flex;
  height: 100vh;
}
.video-item-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}
.video-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr; /* 每行一个视频 */
  grid-template-rows: 1fr; /* 每列一个视频 */
  grid-gap: 10px;
  background-color: #f0f4f7; /* 背景颜色 */
}

.video-wrapper.isTop {
  z-index: 9999;
}

video::-webkit-media-controls-enclosure {
  display: none !important;
}

.video-name {
  flex-grow: 1;
  padding-right: 10px;
  color: #2c3e50;
  font-weight: bold;
}

.videoElements {
  width: 100%;
  height: 100%;
  position: fixed;
  top: 0;
  left: 0;
  background: #121212;
  cursor: none;
}

.close-btn {
  color: #000000; /* 关闭按钮颜色 */
  font-size: 20px;
  font-weight: bold;
  cursor: pointer;
  transition: color 0.3s;
}

.close-btn:hover {
  color: #c0392b;
}

.close-btn:hover {
  color: #ff5a5a;
}

.video-container {
  width: 100%;
  height: 100%;
  cursor: none; /* 隐藏鼠标光标 */
}

.video-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: #f0f4f7; /* 视频容器背景颜色 */
  border-radius: 10px; /* 圆角边框 */
  overflow: hidden;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3); /* 添加阴影 */
}

video {
  position: relative;
  width: 100%;
  height: 100%;
  border-radius: 10px; /* 圆角边框 */
}

.video-list {
  width: 220px; /* 调整列表宽度 */
  background-color: #3498db; /* 更亮的蓝色背景 */
  border-right: 2px solid #2980b9; /* 边框颜色 */
  overflow-y: auto;
  color: #ecf0f1; /* 字体颜色 */
  padding: 15px 0;
}

.video-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.video-list li {
  display: flex;
  flex-direction: column; /* 垂直排列子列表 */
  justify-content: space-between;
  align-items: flex-start;
  padding: 12px 15px; /* 调整内边距 */
  cursor: pointer;
  border-bottom: 1px solid #2980b9; /* 边框颜色 */
  transition: background-color 0.3s, color 0.3s;
}

.video-list li:hover {
  background-color: #2980b9;
}

.device-item {
  margin-bottom: 10px; /* 调整父项间距 */
}

.device-name {
  display: flex;
  align-items: center;
  padding: 12px 15px; /* 调整内边距 */
  cursor: pointer;
  background-color: #3498db; /* 背景颜色 */
  color: #ecf0f1; /* 字体颜色 */
  font-weight: bold;
  transition: background-color 0.3s, color 0.3s;
}

.device-name:hover {
  background-color: #2980b9;
}

.sub-list {
  padding-left: 15px; /* 子列表缩进 */
  border-left: 2px solid #2980b9; /* 子列表边框颜色 */
  margin-top: 5px; /* 调整子列表上间距 */
}

.video-item {
  display: flex;
  align-items: center;
  padding: 8px 15px; /* 调整内边距 */
  cursor: pointer;
  border-bottom: 1px solid #2980b9; /* 边框颜色 */
  transition: background-color 0.3s, color 0.3s;
  color: #ecf0f1; /* 子列表字体颜色 */
}

.video-item:hover {
  background-color: #2980b9;
  color: #ecf0f1; /* 鼠标悬停时子列表字体颜色 */
}

.icon {
  margin-right: 10px;
}
</style>
