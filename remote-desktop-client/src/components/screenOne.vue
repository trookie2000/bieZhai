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
} from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { confirm } from "@tauri-apps/api/dialog";
import { appWindow, WebviewWindow } from "@tauri-apps/api/window";

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
onBeforeUnmount(() => {
  document.removeEventListener("fullscreenchange", handleFullscreenChange);
});
onUnmounted(() => {
  if (unlisten) {
    unlisten();
  }
});
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
  ws = new WebSocket(`ws://192.168.1.2:8081/conn/${data.account.id}`);

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

    console.log(event.data);
    

    const video = videos.find((v: any) => v.stream.id == id);
    video.name = name;
    video.streamId = id;

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
const videos: any = reactive([]);
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

let activeVideoIndex = ref(null);
const setActiveVideo = (index: any) => {
  activeVideoIndex.value = index;
};

const addVideo = (stream: any) => {
  const videoObj = {
    id: Date.now().toString(), // 使用时间戳生成唯一ID
    stream,
    name: `Video ${videos.length + 1}`,
  };
  videos.push(videoObj);
};
const toggleFullScreen = (videoElement: any, vide: any, index: any) => {
  if (!document.fullscreenElement) {
    videoElement
      .requestFullscreen()
      .then(() => {
        videos[index].isFullscreen = true;
        handleFullscreenChange();
        setTimeout(() => {
          videoElement.controls = false; // 延迟隐藏控制栏
        }, 10); // 1秒后隐藏控制栏
        console.log("控制栏隐藏");
      })
      .catch((err: any) => {
        console.error(
          `Error attempting to enable full-screen mode: ${err.message} (${err.name})`
        );
      });
  } else {
    document
      .exitFullscreen()
      .then(() => {
        videos.forEach((v: any) => (v.isFullscreen = false));
        handleFullscreenChange();
        videoElement.controls = true; // 退出全屏后显示控制栏
      })
      .catch((err) => {
        console.error(
          `Error attempting to disable full-screen mode: ${err.message} (${err.name})`
        );
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
const setWindowTop = (video: any) => {
  sendToClient({
    type: InputEventType.WINDOW_EVENT,
    data: {
      windowTitle: video.name + "\0",
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
    <div class="video-grid">
      <div
        v-for="(video, index) in videos"
        :key="video.id"
        class="video-wrapper"
        :class="{ isTop: video.isTop }"
        @click="setActiveVideo(index)"
      >
        <div class="video-container">
          <video
            v-show="data.isShowRemoteDesktop"
            class="video"
            ref="videoElements"
            :srcObject="video.stream"
            @mousedown="(e) => mouseDown(e, ($refs.videoElements as any[])[index])"
            @mouseup="(e) => mouseUp(e, ($refs.videoElements as any[])[index])"
            @mousemove="(e) => mouseMove(e, ($refs.videoElements as any[])[index])"
            @wheel="(e) => wheel(e, ($refs.videoElements as any[])[index])"
            @contextmenu.prevent="(e) => rightClick(e, ($refs.videoElements as any[])[index])
        "
            @dblclick="(e) => {
        toggleFullScreen(($refs.videoElements as any[])[index], video, index);
        setWindowTop(video);
      }
        "
            x5-video-player-type="h5-page"
            autoplay
            controls
          ></video>

          <button
            v-if="data.isShowRemoteDesktop"
            class="close-btn"
            @click="closeVideo(video)"
          >
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

.video-wrapper.isTop {
  z-index: 9999;
}

video::-webkit-media-controls-enclosure {
  display: none !important;
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
</style>
