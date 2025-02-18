<!-- screen.vue -->
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
import { appWindow } from "@tauri-apps/api/window";
import { handleGetTopWindowInfo } from "../common/InputEvent";
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
} from "../common/InputEvent";

// ====================== 1. 定义连接类型与全局变量 ======================
interface Connection {
  remoteId: string;
  pc: RTCPeerConnection;
  dc: RTCDataChannel | null;
  remoteDesktopDpi: {
    width?: number;
    height?: number;
    left?: number;
    right?: number;
    top?: number;
    bottom?: number;
  };
  clearWindowInfoInterval?: number;
  lastMouseX?: number;
  lastMouseY?: number;
  lastTimestamp?: number;
}

// 统一管理所有远程连接
const connections: Record<string, Connection> = reactive({});

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
  isConnecting: false,
});

// WebSocket
let ws: WebSocket;
let webcamStream: MediaStream;
let webcamStreamArr: MediaStream[] = [];

// 关闭窗口监听时的 unlisten
let unlisten: Function | null = null;

// 定义视频对象接口
interface Video {
  id: string;
  stream: MediaStream;
  receiverAccount: {
    id: string;
    password: string;
  };
  name: string;

  width?: number;
  height?: number;
  left?: number;
  right?: number;
  top?: number;
  bottom?: number;
}
// ====================== 2. 生命周期钩子 ======================
onBeforeMount(async () => {
  // 获取本地 ID、密码
  data.account = await invoke("generate_account");
  initWebSocket();
});

onMounted(() => {

  remoteDesktop();
  document.addEventListener("keydown", onKeyDown);
  document.addEventListener("keyup", onKeyUp);
  appWindow
    .onCloseRequested(async (event) => {
      event.preventDefault();
      closeRemoteDesktop();
    })
    .then((unlistenFn: Function) => {
      unlisten = unlistenFn;
    });
});

onBeforeUnmount(() => {
  document.removeEventListener("fullscreenchange", handleFullscreenChange);
});

onUnmounted(() => {
  if (unlisten) {
    unlisten();
    document.removeEventListener("keydown", onKeyDown);
    document.removeEventListener("keyup", onKeyUp);
  }
});

// ====================== 3. WebSocket 相关 ======================
const initWebSocket = () => {
  ws = new WebSocket(`ws://192.168.0.124:8081/conn/${data.account.id}`);

  ws.onopen = () => {
    // 发送心跳
    setInterval(() => {
      sendToServer({
        msg_type: "heartbeat",
        receiver: "",
        sender: "",
        msg: "",
      });
    }, 1000 * 60);

    // 每秒获取所有连接的统计信息
    setInterval(() => {
      for (const remoteId in connections) {
        const conn = connections[remoteId];
        conn.pc.getStats(null).then((stats) => {
          stats.forEach((report) => {
            if (report.type === "candidate-pair" && report.currentRoundTripTime) {
              console.log(`[${remoteId}] 当前RTT:`, report.currentRoundTripTime * 1000, "毫秒");
            }
            if (report.type === "outbound-rtp" && report.jitter) {
              console.log(`[${remoteId}] 网络抖动（Jitter）:`, report.jitter * 1000, "毫秒");
            }
          });
        });
      }
    }, 1000);
  };

  ws.onmessage = async (e: MessageEvent) => {
    const msg: Record<string, any> = JSON.parse(e.data);
    switch (msg.msg_type) {
      case MessageType.VIDEO_OFFER:
        handleVideoOfferMsg(msg);
        break;
      case MessageType.VIDEO_ANSWER:
        handleVideoAnswerMsg(msg);
        break;
      case MessageType.NEW_ICE_CANDIDATE:
        handleNewICECandidateMsg(msg);
        break;
      case MessageType.REMOTE_DESKTOP:
        handleRemoteDesktopRequest(msg);
        break;
      case MessageType.STOP_SHARING:
        close();
        break;
    }
  };

  ws.onerror = (e: Event) => {
    console.log("WebSocket 连接错误:", e);
  };
};

// 当共享方关闭按钮后，远控方关闭对应 video
function closeVideoByMacAddress(msg: Record<string, any>) {
  const id = JSON.parse(msg.msg).id;
  const video = videos.find((item: any) => {
    return item.stream.id == id;
  });
  if (video) {
    closeVideo(video);
  }
}
function onKeyDown(e: KeyboardEvent) {
  // 如果没有激活的视频，就不发事件
  if (activeVideoIndex.value === null) return;
  const video = videos[activeVideoIndex.value];
  if (!video) return;

  const remoteId = video.receiverAccount.id;
  sendToClient(remoteId, {
    type: InputEventType.KEY_EVENT,
    data: {
      // 也可以自己定义更具体的 eventType
      eventType: KeyboardStatus.MOUSE_DOWN,
      key: e.key,
    },
  });
}

function onKeyUp(e: KeyboardEvent) {
  if (activeVideoIndex.value === null) return;
  const video = videos[activeVideoIndex.value];
  if (!video) return;

  const remoteId = video.receiverAccount.id;
  sendToClient(remoteId, {
    type: InputEventType.KEY_EVENT,
    data: {
      eventType: KeyboardStatus.MOUSE_UP,
      key: e.key,
    },
  });
}
// ====================== 4. WebRTC 处理函数 ======================
// 处理视频邀请
const handleVideoOfferMsg = async (msg: Record<string, any>) => {
  // sender 作为远程 ID
  data.receiverAccount.id = msg.sender;
  // 如果不存在连接，则创建
  if (!connections[msg.sender]) {
    connections[msg.sender] = createConnection(msg.sender);
  }
  const connection = connections[msg.sender];

  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await connection.pc.setRemoteDescription(desc);

  const answer = await connection.pc.createAnswer();
  await connection.pc.setLocalDescription(answer);

  sendToServer({
    msg_type: MessageType.VIDEO_ANSWER,
    receiver: msg.sender,
    msg: JSON.stringify(connection.pc.localDescription),
    sender: data.account.id,
  });
};

// 处理对方的 ANSWER
const handleVideoAnswerMsg = async (msg: Record<string, any>) => {
  const connection = connections[msg.sender];
  if (!connection) {
    console.error("连接不存在，无法处理 VIDEO_ANSWER");
    return;
  }
  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await connection.pc.setRemoteDescription(desc).catch(reportError);
};

// 处理新的 ICE
const handleNewICECandidateMsg = async (msg: Record<string, any>) => {
  const connection = connections[msg.sender];
  if (!connection) {
    console.error("连接不存在，无法处理 ICE");
    return;
  }
  const candidate = new RTCIceCandidate(JSON.parse(msg.msg));
  try {
    await connection.pc.addIceCandidate(candidate);
  } catch (err) {
    reportError(err);
  }
};

// 处理远程桌面请求
const handleRemoteDesktopRequest = async (msg: Record<string, any>) => {
  try {
    if (msg.msg !== data.account.password) {
      console.log("密码错误！");
      return;
    }
    data.receiverAccount.id = msg.sender;

    // 若无连接则创建
    if (!connections[msg.sender]) {
      connections[msg.sender] = createConnection(msg.sender);
    }
    const connection = connections[msg.sender];

    // 主动创建数据通道
    initRTCDataChannel(connection);

    // 获取本地桌面流
    const newStream = await navigator.mediaDevices.getDisplayMedia({
      video: true,
      audio: false,
    });

    // 记录到一个数组，方便 close 时统一管理
    webcamStreamArr.push(newStream);

    // 把这个新流的所有轨道都加到 RTCPeerConnection
    newStream.getTracks().forEach((track) => {
      connection.pc.addTrack(track, newStream);
    });
    await renegotiate(connection);
    // 发起 Offer
    // sendOffer(connection);
  } catch (error) {
    console.error("处理远程桌面请求时出错:", error);
    data.isConnecting = false;
  }
};
// 做一次 Offer -> Answer 的再协商
async function renegotiate(connection: Connection) {
  const offer = await connection.pc.createOffer();
  await connection.pc.setLocalDescription(offer);

  // 发送到信令服务器
  sendToServer({
    msg_type: MessageType.VIDEO_OFFER,
    receiver: connection.remoteId,
    msg: JSON.stringify(connection.pc.localDescription),
    sender: data.account.id,
  });
}
// ====================== 5. 创建 RTCPeerConnection ======================
function createConnection(remoteId: string): Connection {
  const iceServer = {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      {
        urls: "turn:numb.viagenie.ca",
        username: "webrtc@live.com",
        credential: "muazkh",
      },
    ],
  };

  const pc = new RTCPeerConnection(iceServer);
  const connection: Connection = {
    remoteId,
    pc,
    dc: null,
    remoteDesktopDpi: {},
    lastMouseX: 0,
    lastMouseY: 0,
    lastTimestamp: performance.now(),
  };

  // ICE
  pc.onicecandidate = (event: RTCPeerConnectionIceEvent) => {
    if (event.candidate) {
      sendToServer({
        msg_type: MessageType.NEW_ICE_CANDIDATE,
        receiver: remoteId,
        msg: JSON.stringify(event.candidate),
        sender: data.account.id,
      });
    }
  };

  // ICE 状态变化
  pc.oniceconnectionstatechange = () => {
    console.log(`*** ICE(${remoteId}) state: ${pc.iceConnectionState}`);
  };

  // Track 事件
  pc.ontrack = (event: RTCTrackEvent) => {
    // 可能会有多条 stream
    event.streams.forEach((stream) => {
      // 如果本地 videos 里还没有，就添加
      if (!videos.some((v) => v.stream.id === stream.id)) {
        addVideo(stream, remoteId);
      }
    });
  };
  // 当对方创建数据通道
  pc.ondatachannel = (e: RTCDataChannelEvent) => {
    connection.dc = e.channel;
    handleDataChannel(e, connection);
  };

  return connection;
}

// ====================== 6. DataChannel 处理 ======================
function initRTCDataChannel(connection: Connection) {
  connection.dc = connection.pc.createDataChannel("my channel", { ordered: true });

  connection.dc.onopen = async () => {
    data.isConnecting = true;
    console.log(`数据通道(${connection.remoteId})已打开`);

    // 定义发送窗口信息的函数
    const sendWindowInfo = async () => {
      const windInfo: any = await handleGetTopWindowInfo();
      let w, h;
      if (windInfo.name.includes("正在共享你的屏幕")) {
        w = window.screen.width;
        h = window.screen.height;
      } else {
        w = windInfo.width;
        h = windInfo.height;
      }
      const lastStream = webcamStreamArr[webcamStreamArr.length - 1];
      const streamId = lastStream ? lastStream.id : "unknown";
      // 发送给对端
      connection.dc?.send(
        JSON.stringify({
          id: streamId,
          name: windInfo.name,
          width: w * window.devicePixelRatio,
          height: h * window.devicePixelRatio,
          left: windInfo.left,
          right: windInfo.right,
          top: windInfo.top,
          bottom: windInfo.bottom,
        })
      );
    };

    // 初次发送
    await sendWindowInfo();
    // 每隔1秒发送
    const intervalId = setInterval(sendWindowInfo, 1000);

  };

  connection.dc.onmessage = (event: MessageEvent) => {
    const msg = JSON.parse(event.data);
    switch (msg.type) {
      case InputEventType.MOUSE_EVENT:
        handleMouseEvent(msg.data);
        break;
      case InputEventType.KEY_EVENT:
        handleKeyboardEvent(msg.data);
        break;
    }
  };

  connection.dc.onclose = () => {
    console.log(`数据通道(${connection.remoteId})已关闭`);
    if (connection.clearWindowInfoInterval) {
      clearInterval(connection.clearWindowInfoInterval);
    }
  };
}

// 当对方创建数据通道
function handleDataChannel(e: RTCDataChannelEvent, connection: Connection) {
  data.isConnecting = false;
  const dc = e.channel;
  connection.dc = dc;

  dc.onopen = () => {
    console.log(`对方创建的数据通道(${connection.remoteId})已打开`);
  };

  dc.onmessage = (event: MessageEvent) => {
    // 这里处理对端发送过来的窗口信息、分辨率更新等
    const { id, name, width, height, left, right, top, bottom } = JSON.parse(event.data);

    // 找到对应的视频对象更新
    const video = videos.find((v: any) => v.stream.id === id);
    if (video) {
      if (!video.name) {
        video.name = name;
      }
      video.width = width;
      video.height = height;
      video.left = left;
      video.right = right;
      video.top = top;
      video.bottom = bottom;
    }
    // 同步更新到连接对象里
    connection.remoteDesktopDpi = { width, height, left, right, top, bottom };
  };

  dc.onclose = () => {
    console.log(`对方创建的数据通道(${connection.remoteId})已关闭`);
  };
}

// ====================== 7. 发起 Offer ======================
// const sendOffer = async (connection: Connection) => {
//   const offer = await connection.pc.createOffer();
//   await connection.pc.setLocalDescription(offer);

//   sendToServer({
//     msg_type: MessageType.VIDEO_OFFER,
//     receiver: connection.remoteId,
//     msg: JSON.stringify(connection.pc.localDescription),
//     sender: data.account.id,
//   });
// };

// ====================== 8. 关闭所有连接 ======================
function closeAllConnections() {
  for (const remoteId in connections) {
    const conn = connections[remoteId];
    // 关闭 RTCPeerConnection
    conn.pc.close();
    // 清理定时器
    if (conn.clearWindowInfoInterval) {
      clearInterval(conn.clearWindowInfoInterval);
    }
  }
}

// ====================== 9. 用户交互逻辑 ======================
const remoteDesktop = async () => {
  appWindow.setFullscreen(false);
  data.isConnecting = true;
  setTimeout(() => {
    data.isShowRemoteDesktop = true;
  }, 0);
};

// ====================== 10. 视频管理 ======================
const videos: Video[] = reactive([]);
const videoElements = ref<HTMLVideoElement[]>([]);

function addVideo(stream: MediaStream, remoteId: string) {
  // 每条新流都对应一个 Video 对象
  const videoObj: Video = {
    id: Date.now().toString(),
    stream,
    receiverAccount: {
      id: remoteId,
      password: "",
    },
    name: "",
  };
  videos.push(videoObj);
}
const closeRemoteDesktop = async () => {
  const confirmed = await confirm("确认结束远程控制？", "提示");
  if (confirmed) {
    appWindow.setFullscreen(false);
    data.isShowRemoteDesktop = false;
    // 关闭所有连接
    closeAllConnections();
    closeAllVideos();
    // 停止并移除所有的视频流
    webcamStreamArr.forEach((stream) => {
      stream.getTracks().forEach((track) => track.stop());
    });
    webcamStreamArr = [];

    // 发送 STOP_SHARING
    sendToServer({
      msg_type: MessageType.STOP_SHARING,
      receiver: data.receiverAccount.id,
      msg: data.receiverAccount.password,
      sender: data.account.id,
    });

    // 关闭当前窗口
    appWindow.close();
  }
};
function closeAllVideos() {
  // 方法1：倒序遍历，避免 splice 导致下标错乱
  for (let i = videos.length - 1; i >= 0; i--) {
    closeVideo(videos[i]);
  }
}

const closeVideo = (video: Video) => {
  console.log("Closing video with ID:", video.id);

  const index = videos.findIndex((v) => v.id === video.id);
  if (index !== -1) {
    // 通知对端关闭
    sendToServer({
      msg_type: MessageType.CLOSE_REMOTE_DESKTOP,
      receiver: video.receiverAccount.id,
      msg: JSON.stringify({
        password: data.receiverAccount.password,
        id: video.stream.id,
      }),
      sender: data.account.id,
    });
    videos.splice(index, 1);
    if (activeVideoIndex.value === index) {
      activeVideoIndex.value = null;
    }
  }
};

// ====================== 11. 鼠标事件处理 ======================
const mouseDown = (e: MouseEvent, videoElement: HTMLVideoElement, remoteId: string) => {
  sendMouseEvent(e, videoElement, mouseType(MouseStatus.MOUSE_DOWN, e.button), remoteId);
};
const mouseUp = (e: MouseEvent, videoElement: HTMLVideoElement, remoteId: string) => {
  sendMouseEvent(e, videoElement, mouseType(MouseStatus.MOUSE_UP, e.button), remoteId);
};
const mouseMove = (e: MouseEvent, videoElement: HTMLVideoElement, remoteId: string) => {
  sendMouseEvent(e, videoElement, MouseStatus.MOUSE_MOVE, remoteId);
};
const wheel = (e: WheelEvent, videoElement: HTMLVideoElement, remoteId: string) => {
  const type = e.deltaY > 0 ? WheelStatus.WHEEL_DOWN : WheelStatus.WHEEL_UP;
  sendMouseEvent(e, videoElement, type, remoteId);
};
const rightClick = (e: MouseEvent, videoElement: HTMLVideoElement, remoteId: string) => {
  sendMouseEvent(e, videoElement, MouseStatus.RIGHT_CLICK, remoteId);
};

// 如果想每个连接单独计算鼠标速度，可把这几个全局变量也放进 Connection 里
let lastMouseX = 0;
let lastMouseY = 0;
let lastTimestamp = performance.now();

function sendMouseEvent(
  e: MouseEvent,
  videoElement: HTMLVideoElement,
  eventType: string,
  remoteId: string
) {
  const connection = connections[remoteId];
  if (!connection || !connection.remoteDesktopDpi) return;

  const { width = 0, height = 0, left = 0, top = 0 } = connection.remoteDesktopDpi;
  if (!videoElement.offsetWidth || !videoElement.offsetHeight) return;

  const xRatio = width / videoElement.offsetWidth;
  const yRatio = height / videoElement.offsetHeight;

  const x = Math.round(e.offsetX * xRatio) + left;
  const y = Math.round(e.offsetY * yRatio) + top;

  const now = performance.now();
  const deltaTime = now - lastTimestamp;
  const deltaX = x - lastMouseX;
  const deltaY = y - lastMouseY;
  const speedX = deltaTime ? deltaX / deltaTime : 0;
  const speedY = deltaTime ? deltaY / deltaTime : 0;

  sendToClient(remoteId, {
    type: InputEventType.MOUSE_EVENT,
    data: { x, y, speedX, speedY, eventType },
  });

  lastMouseX = x;
  lastMouseY = y;
  lastTimestamp = now;
}

function mouseType(mouseStatus: MouseStatus, button: number) {
  switch (button) {
    case 0:
      return "left-" + mouseStatus;
    case 2:
      return "right-" + mouseStatus;
    default:
      return "unknown-" + mouseStatus;
  }
}

// ====================== 12. 发送消息 ======================
function sendToServer(msg: Record<string, any>) {
  ws.send(JSON.stringify(msg));
}

function sendToClient(remoteId: string, msg: Record<string, any>) {
  const connection = connections[remoteId];
  if (connection && connection.dc && connection.dc.readyState === "open") {
    connection.dc.send(JSON.stringify(msg));
  }
}

// ====================== 13. 界面辅助逻辑 ======================
let activeVideoIndex = ref<number | null>(null);

const setActiveVideo = (index: number) => {
  activeVideoIndex.value = index;
};

const showVideo = (video: Video) => {
  activeVideoIndex.value = videos.findIndex((v) => v.id === video.id);
};

const activeDeviceId = ref<string | null>(null);

const groupedVideos = computed((): Record<string, Video[]> => {
  const groups: Record<string, Video[]> = {};
  videos.forEach((video) => {
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

// 设置窗口置顶（示例：更新 connection.remoteDesktopDpi 并通知对端）
function setWindowTop(video: Video) {
  const remoteId = video.receiverAccount.id;
  const connection = connections[remoteId];
  if (!connection) return;

  // 更新 remoteDesktopDpi
  connection.remoteDesktopDpi = {
    width: (video as any).width,
    height: (video as any).height,
    left: (video as any).left,
    right: (video as any).right,
    top: (video as any).top,
    bottom: (video as any).bottom,
  };

  // 发送给客户端
  sendToClient(remoteId, {
    type: InputEventType.WINDOW_EVENT,
    data: {
      windowTitle: (video.name || "") + "\0",
    },
  });
}

// 全屏切换监测
const isVideoFullscreen = ref(true);
document.addEventListener("fullscreenchange", handleFullscreenChange);

function handleFullscreenChange() {
  isVideoFullscreen.value = document.fullscreenElement !== null;
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

// 监听 videos 数组变动，给每个 video 标签加事件（可根据需要保留/修改）
watch(
  videos,
  () => {
    const allVideos = document.querySelectorAll("video");
    allVideos.forEach((video: any) => {
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
      // 保存以便移除
      video.__clickHandler = clickHandler;
      video.__keydownHandler = keydownHandler;
    });
  },
  { deep: true }
);

// ====================== 14. 通用报错处理 ======================
function reportError(err: any) {
  console.error("WebRTC Error:", err);
}
</script>

<template>
  <div class="container">
    <!-- 列表 -->
    <div class="video-list">
      <ul>
        <li v-for="(videosOfDevice, deviceId) in groupedVideos" :key="deviceId" class="device-item">
          <div @click="toggleDevice(deviceId)" class="device-name">
            <i class="icon fas fa-desktop"></i>{{ deviceId }}
            <i :class="[
              'icon',
              'fas',
              activeDeviceId === deviceId ? 'fa-chevron-down' : 'fa-chevron-right',
            ]"></i>
          </div>
          <ul v-show="activeDeviceId === deviceId" class="sub-list">
            <li v-for="video in videosOfDevice" :key="video.id" class="video-item">
              <div class="video-item-content">
                <span @click="() => {
                    showVideo(video);
                    setWindowTop(video);
                  }
                  " class="video-name">
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

    <!-- 视频区域 -->
    <div class="video-grid">
      <div v-for="(video, index) in videos" :key="video.id" class="video-wrapper" v-show="activeVideoIndex === index">
        <div class="video-container">
          <video class="video" ref="videoElements" :srcObject="video.stream"
            @mousedown="(e) => mouseDown(e, (videoElements as HTMLVideoElement[])[index], video.receiverAccount.id)"
            @mouseup="(e) => mouseUp(e, (videoElements as HTMLVideoElement[])[index], video.receiverAccount.id)"
            @mousemove="(e) => mouseMove(e, (videoElements as HTMLVideoElement[])[index], video.receiverAccount.id)"
            @wheel="(e) => wheel(e, (videoElements as HTMLVideoElement[])[index], video.receiverAccount.id)"
            @contextmenu.prevent="(e) => rightClick(e, (videoElements as HTMLVideoElement[])[index], video.receiverAccount.id)"
            x5-video-player-type="h5-page" autoplay controls></video>
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
  grid-template-columns: 1fr;
  grid-template-rows: 1fr;
  grid-gap: 10px;
  background-color: #000000;
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
  color: #000000;
  font-size: 20px;
  font-weight: bold;
  cursor: pointer;
  transition: color 0.3s;
}


.close-btn:hover {
  color: #ff5a5a;
}

.video-container {
  width: 100%;
  height: 100%;
  cursor: none;
}

.video-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: #f0f4f7;
  overflow: hidden;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
}

video {
  position: relative;
  width: 100%;
  height: 100%;
  border-radius: 10px;
}

.video-list {
  width: 220px;
  background-color: #1e88e5;
  border-right: 2px solid #1e88e5;
  overflow-y: auto;
  color: #ecf0f1;
  padding: 15px 0;
}

.video-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.video-list li {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  align-items: flex-start;
  padding: 12px 15px;
  cursor: pointer;
  border-bottom: 1px solid #1e88e5;
  transition: background-color 0.3s, color 0.3s;
}


.device-item {
  margin-bottom: 10px;
}

.device-name {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: px 0px;
  cursor: pointer;
  background-color: #3498db;
  color: #ecf0f1;
  font-weight: bold;
  transition: background-color 0.3s, color 0.3s;
}

.device-name .icon {
  margin-left: 10px;
  font-size: 14px;
  transition: transform 0.3s;
}

.device-name:hover {
  background-color: #2980b9;
}

.sub-list {
  padding-left: 15px;
  border-left: 2px solid #2980b9;
  margin-top: 5px;
}

.video-item {
  display: flex;
  align-items: center;
  padding: 8px 15px;
  cursor: pointer;
  border-bottom: 1px solid #2980b9;
  transition: background-color 0.3s, color 0.3s;
  color: #ecf0f1;
}

.video-item:hover {
  background-color: #2980b9;
  color: #ecf0f1;
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
}

.icon {
  margin-right: 10px;
}
</style>
