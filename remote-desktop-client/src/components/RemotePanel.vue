<script setup lang="ts">
import { ref, reactive, onBeforeMount, onMounted, onUnmounted } from "vue";
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
  handleGetTopWindowInfo,
  handleKeyboardEvent,
  handleMouseEvent,
  handleWindowTop,
} from "../common/InputEvent";
import eventBus from "../common/eventBus"; // 引入事件总线

// 存储页面所需的响应式数据
const data = reactive({
  account: {
    id: "",
    password: "",
  },
  receiverAccount: {
    id: "",
    password: "",
  },
  screenChangesignal: 0, // 用于远控窗口数量
  isShowRemoteDesktop: false,
  isConnecting: false, // 连接状态
  clearWindowInfoInterval: null as (() => void) | null,
  deviceList: [] as { ip: string; password: string }[], // 设备列表
});

const isDeviceListOpen = ref(false);

const toggleDeviceList = () => {
  isDeviceListOpen.value = !isDeviceListOpen.value;
};

const removeDevice = (index: number) => {
  data.deviceList.splice(index, 1);
};

// 视频元素引用
const desktop = ref<HTMLVideoElement>();

// WebSocket
let ws: WebSocket;

// 为了同时远控多台机器，我们用一个 Map 来维护每个连接（pc/dc/流等）
type ConnectionInfo = {
  pc: RTCPeerConnection;
  dc: RTCDataChannel | null;
  webcamStreamArr: MediaStream[];
  remoteDesktopDpi: Record<string, any>;
};
const connections = new Map<string, ConnectionInfo>();

/** 获取或创建某个远程目标(remoteId)的连接信息 */
function getOrCreateConnection(remoteId: string): ConnectionInfo {
  let conn = connections.get(remoteId);
  if (conn) {
    return conn;
  }

  // 新建 PeerConnection
  const pc = new RTCPeerConnection({
    iceServers: [
      {
        urls: "stun:stun.l.google.com:19302",
      },
      {
        urls: "turn:numb.viagenie.ca",
        username: "webrtc@live.com",
        credential: "muazkh",
      },
    ],
  });

  // 初始化存储结构
  conn = {
    pc,
    dc: null,
    webcamStreamArr: [],
    remoteDesktopDpi: {},
  };

  // 绑定 PC 事件
  pc.onicecandidate = (event: RTCPeerConnectionIceEvent) => {
    if (event.candidate) {
      // 发送 ICE 给对端
      sendToServer({
        msg_type: MessageType.NEW_ICE_CANDIDATE,
        receiver: remoteId,
        msg: JSON.stringify(event.candidate),
        sender: data.account.id,
      });
    }
  };

  pc.oniceconnectionstatechange = (event: Event) => {
    console.log("*** ICE 连接状态变为 " + pc.iceConnectionState);
  };

  pc.onicegatheringstatechange = (event: Event) => {
    console.log("*** ICE 聚集状态变为 " + pc.iceGatheringState);
  };

  pc.onsignalingstatechange = (event: Event) => {
    console.log("*** WebRTC 信令状态变为: " + pc.signalingState);
  };

  // 收到远端的音视频 Track
  pc.ontrack = (event: RTCTrackEvent) => {
    // 这里只简单地把画面显示到一个固定的 <video> 上
    desktop.value!.srcObject = event.streams[0];

    // 键盘事件示例
    document.onkeydown = (e: KeyboardEvent) => {
      sendToClient(remoteId, {
        type: InputEventType.KEY_EVENT,
        data: {
          eventType: KeyboardStatus.MOUSE_DOWN,
          key: e.key,
        },
      });
    };

    document.onkeyup = (e: KeyboardEvent) => {
      sendToClient(remoteId, {
        type: InputEventType.KEY_EVENT,
        data: {
          eventType: KeyboardStatus.MOUSE_UP,
          key: e.key,
        },
      });
    };
  };

  // 对端创建 DataChannel
  pc.ondatachannel = (e: RTCDataChannelEvent) => {
    conn!.dc = e.channel;
    const dc = conn!.dc;

    dc.onopen = () => {
      console.log("数据通道已打开: remoteId=", remoteId);
    };

    dc.onmessage = (event: MessageEvent) => {
      // 这里简单存一下对端发来的 DPI 信息
      conn!.remoteDesktopDpi = JSON.parse(event.data);
    };

    dc.onclose = () => {
      console.log("数据通道已关闭: remoteId=", remoteId);
    };

    console.log("数据通道:", dc);
  };

  connections.set(remoteId, conn);
  return conn;
}

/** 主动创建 DataChannel（被控端也可 ondatachannel 收到） */
function createDataChannel(remoteId: string) {
  const conn = getOrCreateConnection(remoteId);
  if (conn.dc) {
    // 已有 dataChannel 就不重复创建
    return;
  }
  const pc = conn.pc;
  const dc = pc.createDataChannel("my channel", { ordered: true });
  conn.dc = dc;

  dc.onopen = () => {
    data.isConnecting = true;
    console.log("数据通道已打开 for remoteId=", remoteId);

    // 启动定时器，不断发送窗口信息
    const intervalId = setInterval(async () => {
      const windInfo: any = await handleGetTopWindowInfo();
      let w, h;
      if (windInfo.name.includes("正在共享你的屏幕")) {
        w = window.screen.width;
        h = window.screen.height;
      } else {
        w = windInfo.width;
        h = windInfo.height;
      }

      // 拿到当前最新的共享流
      const arr = conn.webcamStreamArr;
      if (arr.length === 0) return;

      // 默认用最后一次共享的流 ID
      const currentStreamId = arr[arr.length - 1].id;
      dc.send(
        JSON.stringify({
          id: currentStreamId,
          name: windInfo.name,
          width: w * window.devicePixelRatio,
          height: h * window.devicePixelRatio,
          left: windInfo.left,
          right: windInfo.right,
          top: windInfo.top,
          bottom: windInfo.bottom,
        })
      );
    }, 1000);

    const clearWindowInfoInterval = () => {
      clearInterval(intervalId);
    };
    data.clearWindowInfoInterval = clearWindowInfoInterval;
  };

  dc.onmessage = (event: MessageEvent) => {
    const msg: Record<string, any> = JSON.parse(event.data);
    switch (msg.type) {
      case InputEventType.MOUSE_EVENT:
        handleMouseEvent(msg.data);
        break;
      case InputEventType.KEY_EVENT:
        handleKeyboardEvent(msg.data);
        break;
      case InputEventType.WINDOW_EVENT:
        handleWindowTop(msg.data);
        break;
    }
  };

  dc.onclose = () => {
    data.isConnecting = false;
    console.log("数据通道已关闭 for remoteId=", remoteId);
  };
}

/** 发送 offer 给对端 */
async function sendOffer(remoteId: string) {
  const conn = getOrCreateConnection(remoteId);
  const pc = conn.pc;

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);

  sendToServer({
    msg_type: MessageType.VIDEO_OFFER,
    receiver: remoteId,
    msg: JSON.stringify(pc.localDescription),
    sender: data.account.id,
  });
}

/** 初始化 WebSocket 连接 */
const initWebSocket = () => {
  ws = new WebSocket(`ws://192.168.0.124:8081/conn/${data.account.id}`);
  ws.onopen = () => {
    // 定时心跳
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
      case MessageType.CLOSE_REMOTE_DESKTOP:
        close(msg);
        break;
    }
  };

  ws.onerror = (e: Event) => {
    console.log("WebSocket 连接错误:", e);
  };
};

/** 收到对方的 VIDEO_OFFER */
const handleVideoOfferMsg = async (msg: Record<string, any>) => {
  const remoteId = msg.sender;
  data.receiverAccount.id = remoteId; // 仅用于UI显示

  const conn = getOrCreateConnection(remoteId);
  const pc = conn.pc;

  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc);

  const answer = await pc.createAnswer();
  await pc.setLocalDescription(answer);

  sendToServer({
    msg_type: MessageType.VIDEO_ANSWER,
    receiver: remoteId,
    msg: JSON.stringify(answer),
    sender: data.account.id,
  });
};

/** 收到对方的 VIDEO_ANSWER */
const handleVideoAnswerMsg = async (msg: Record<string, any>) => {
  const remoteId = msg.sender;
  const conn = getOrCreateConnection(remoteId);
  const pc = conn.pc;

  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc).catch(reportError);
};

/** 收到对方的 ICE_CANDIDATE */
const handleNewICECandidateMsg = async (msg: Record<string, any>) => {
  const remoteId = msg.sender;
  const conn = getOrCreateConnection(remoteId);
  const pc = conn.pc;

  const candidate = new RTCIceCandidate(JSON.parse(msg.msg));
  try {
    await pc.addIceCandidate(candidate);
  } catch (err) {
    reportError(err);
  }
};

/** 收到对方请求 REMOTE_DESKTOP */
const handleRemoteDesktopRequest = async (msg: Record<string, any>) => {
  const remoteId = msg.sender;
  // if (msg.msg != data.account.password) { ... }

  data.receiverAccount.id = remoteId; // 用于UI显示

  // 获取 / 创建 RTCPeerConnection
  const conn = getOrCreateConnection(remoteId);
  createDataChannel(remoteId);

  // 采集本地桌面流
  const webcamStream: MediaStream = await navigator.mediaDevices.getDisplayMedia({
    video: true,
    audio: false,
  });

  data.screenChangesignal++;
  conn.webcamStreamArr.push(webcamStream);

  // 监听屏幕流结束
// 给流中的每条 track 添加 onended 监听
webcamStream.getTracks().forEach((track) => {
  track.addEventListener("ended", (e) => {
    // 在这里处理停止共享逻辑
    data.screenChangesignal--;
    sendToServer({
      msg_type: MessageType.STOP_SHARING,
      receiver: remoteId,
      msg: JSON.stringify({
        id: webcamStream.id, // 或者 track.id
      }),
      sender: data.account.id,
    });
    if (data.screenChangesignal === 0) {
      data.isConnecting = false;
    }
  });
});

  // 把每条 track 加到 PeerConnection
  webcamStream.getTracks().forEach((track) => {
    conn.pc.addTrack(track, webcamStream);
  });

  // 发送 offer
  await sendOffer(remoteId);
};

/** 请求远程桌面：主动方 */
const remoteDesktop = async () => {
  if (!data.receiverAccount.id) {
    alert("请输入IP地址");
    return;
  }
  eventBus.emit("event");

  // 打开一个新的 webview（如果你有此需求）
  const webview = new WebviewWindow("1", {
    url: "#/screenOne",
  });
  webview.once("tauri://created", () => {});
  webview.once("tauri://error", (e) => {
    console.error("Webview error:", e);
  });

  // 通知对方，想要发起远程桌面
  sendToServer({
    msg_type: MessageType.REMOTE_DESKTOP,
    receiver: data.receiverAccount.id,
    msg: data.receiverAccount.password,
    sender: data.account.id,
  });
};

/** 关闭远程桌面 */
const closeRemoteDesktop = async () => {
  const confirmed = await confirm("确认结束被控？", "提示");
  if (confirmed) {
    appWindow.setFullscreen(false);
    data.isShowRemoteDesktop = false;

    // 清空视频元素
    if (desktop.value) {
      desktop.value.srcObject = null;
    }

    // 遍历所有连接，停止流并关闭 PeerConnection
    connections.forEach((conn, remoteId) => {
      // 关闭所有共享流
      conn.webcamStreamArr.forEach((stream) => {
        stream.getTracks().forEach((track) => track.stop());
      });
      // 关闭 PeerConnection
      conn.pc.close();
    });
    // 清空 connections Map
    connections.clear();

    // 清除定时器（如果有用到 data.clearWindowInfoInterval）
    if (data.clearWindowInfoInterval) {
      data.clearWindowInfoInterval();
      data.clearWindowInfoInterval = null;
    }

    // 通知对端停止共享
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

/** 关闭共享流/连接 */
const close = (msg?: Record<string, any>) => {
  if (msg) {
    // 只关闭特定流
    const id = JSON.parse(msg.msg).id;
    // 找到对应连接
    const remoteId = msg.sender;
    const conn = connections.get(remoteId);
    if (!conn) return;

    const targetStream = conn.webcamStreamArr.find((s) => s.id === id);
    targetStream?.getTracks().forEach((track) => track.stop());
  } else {
    // 全部关闭
    connections.forEach((conn) => {
      conn.webcamStreamArr.forEach((stream) => {
        stream.getTracks().forEach((track) => track.stop());
      });
      conn.webcamStreamArr = [];
    });
  }
};

/** 发送消息给服务器 */
const sendToServer = (msg: Record<string, any>) => {
  if (!ws) return;
  const msgJSON = JSON.stringify(msg);
  ws.send(msgJSON);
};

/** 发送消息给指定 remoteId 的对端 DataChannel */
function sendToClient(remoteId: string, msg: Record<string, any>) {
  const conn = connections.get(remoteId);
  if (!conn || !conn.dc) return;
  if (conn.dc.readyState === "open") {
    conn.dc.send(JSON.stringify(msg));
  }
}

/** 错误处理 */
function reportError(err: any) {
  console.error("WebRTC Error:", err);
}

/** 组件生命周期 */
onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});

onMounted(() => {
  eventBus.on("addDevice", (deviceIp: any) => {
    console.log("接收到事件 addDevice设备IP:", deviceIp);
    if (!data.deviceList.some((device) => device.ip === deviceIp)) {
      data.deviceList.push({
        ip: deviceIp,
        password: "",
      });
    }
    console.log("更新后的设备列表:", data.deviceList);
  });

  appWindow
    .onCloseRequested(async (event) => {
      event.preventDefault();
      closeRemoteDesktop();
    })
    .then((unlistenFn: Function) => {
      // 监听函数
    });
});

onUnmounted(() => {
  // 清理
});
</script>

<template>
  <div v-if="data.isConnecting" class="connecting-message" style="position: fixed; top: 0; left: 0; right: 0; bottom: 0">
    正在被远控{{ data.screenChangesignal }}个窗口...
  </div>
  <button v-if="data.isConnecting" class="close-btn" @click="closeRemoteDesktop()">
    结束被控
  </button>

  <div class="container">
    <div class="main">
      <div class="ip-display">
        ip: <span>{{ data.account.id }}</span>
      </div>
      <div v-if="!data.isConnecting" class="form">
        <input v-model="data.receiverAccount.id" type="text" placeholder="请输入对方ip" />
        <button @click="remoteDesktop()">发起远程</button>
      </div>
    </div>
  </div>
</template>
<style lang="less" scoped>
.container {
  display: flex;
  height: 100%;
}

.sidebar {
  width: 200px;
  height: 100%;
  background: #2196f3;
  color: #fafafa;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  align-items: center;
  border-right: 1px solid #1e88e5;
  box-sizing: border-box;
  padding: 20px;

  .device-list-container {
    width: 100%;
  }

  .device-list-title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px;
    background: #42a5f5;
    margin-bottom: 10px;
    border-radius: 5px;
    cursor: pointer;

    .icon {
      margin-left: 10px;
    }

    &:hover {
      background: #90caf9;
    }
  }

  .device-list {
    list-style: none;
    padding: 0;
    margin: 0;
    width: 100%;

    .device-item {
      display: flex;
      align-items: center;
      padding: 10px;
      background: #ffffff;
      margin-bottom: 10px;
      border-radius: 5px;
      cursor: pointer;

      &:hover {
        background: #90caf9;
      }

      .device-item-content {
        display: flex;
        justify-content: space-between;
        width: 100%;
      }

      .close-btn {
        color: red;
        cursor: pointer;
      }
    }
  }
}

.main {
  flex-grow: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background: #1b1b1c;
}

.ip-display {
  background: #424242;
  padding: 10px 20px;
  border-radius: 10px;
  color: #fafafa;
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 230px;
}

.connecting-message {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: #1b1b1c;
  color: #fff;
  display: flex;
  justify-content: center;
  align-items: center;
  font-size: 24px;
}

.form {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;

  button {
    width: 280px;
    height: 34px;
    background: #00c1cd;
    cursor: pointer;
  }
}

.form button:hover {
  background-color: #2980b9;
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

.close-btn {
  color: red;
  cursor: pointer;
}
</style>
