<script setup lang="ts">
import { ref, reactive, onBeforeMount, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { confirm } from "@tauri-apps/api/dialog";
import { appWindow, WebviewWindow } from "@tauri-apps/api/window";
// import TauriWebsocket from 'tauri-plugin-websocket-api';
// import WebSocket from "tauri-plugin-websocket-api";
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
  screenChangesignal: 0, //用于远控窗口数量
  isShowRemoteDesktop: false,
  isConnecting: false, //连接状态
  clearWindowInfoInterval: null as (() => void) | null, 
});

// 对象用于引用视频元素，DOM对象s
const desktop = ref<HTMLVideoElement>();

// WebSocket 连接和RTC其他变量
let ws: WebSocket;
let pc: RTCPeerConnection;
let dc: RTCDataChannel;
let webcamStreamArr: MediaStream[] = [];
//分辨率
let remoteDesktopDpi: Record<string, any>;
let unlisten: Function | null = null;
onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});
onMounted(() => {
  appWindow
    .onCloseRequested(async (event) => {
      event.preventDefault();
      closeRemoteDesktop();
    })
    .then((unlistenFn: Function) => {
      unlisten = unlistenFn;
    });
});

// 在组件卸载时取消监听器
onUnmounted(() => {
  if (unlisten) {
    unlisten();
  }
});

//   data.account = await invoke("generate_account");

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
        msg: "",
      });
    }, 1000 * 60);
  };

  ws.onmessage = async (e: MessageEvent) => {
    const msg: Record<string, any> = JSON.parse(e.data);
    switch (msg.msg_type) {
      case MessageType.VIDEO_OFFER: // 视频通话邀请s
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
        close(msg);
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
//此标记用于判定remotePanel页面为正在被共享还是初始状态，若screenChangesignal为0，则表示共享窗口全部关闭
let screenChangesignal = 0;
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
  const webcamStream: any = await navigator.mediaDevices.getDisplayMedia({
    video: true,
    audio: false,
  });
  data.screenChangesignal++;
  webcamStreamArr.push(webcamStream);

  // 点击漂浮栏中的【停止共享】按钮，MediaStream 触发 oninactive 事件，同时 MediaStreamTrack 触发 onended 事件
  webcamStream.oninactive = (e: any) => {
    console.log("mediaStream oninactive");

    console.log(webcamStream);
    data.screenChangesignal--;
    sendToServer({
      msg_type: MessageType.STOP_SHARING,
      receiver: data.receiverAccount.id,
      msg: JSON.stringify({
        id: e.currentTarget.id,
      }),
      sender: data.account.id,
    });
    if (data.screenChangesignal == 0) {
      console.log("共享窗口已全部关闭，界面状态更新");
      data.isConnecting = false;
    }
  };

  webcamStream.getTracks().forEach((track: MediaStreamTrack) => {
    pc.addTrack(track, webcamStream);
  });

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

  // 将清除定时器的方法暴露出去以便在需要时调用
  data.clearWindowInfoInterval = clearWindowInfoInterval;
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
      case InputEventType.WINDOW_EVENT:
        handleWindowTop(msg.data);
        break;
    }
  };

  dc.onclose = (e: Event) => {
    data.isConnecting = false;
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

  // Create a unique label for each webview
  const uniqueLabel = `webview_${Date.now()}`;

  const webview = new WebviewWindow("1", {
    url: "#/screenOne",
  });

  webview.once("tauri://created", function () {
    // Webview created successfully
  });

  webview.once("tauri://error", function (e) {
    // An error occurred during webview window creation
    console.error("Webview error:", e);
  });

  sendToServer({
    msg_type: MessageType.REMOTE_DESKTOP,
    receiver: data.receiverAccount.id,
    msg: data.receiverAccount.password,
    sender: data.account.id,
  });
};

// 关闭远程桌面
const closeRemoteDesktop = async () => {
  const confirmed = await confirm("确认结束被控？", "提示");
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

// const unlisten = await appWindow.onCloseRequested((event) => {
//   const confirmed = confirm('Are you sure?');
//   if (!confirmed) {
//     // user did not confirm closing the window; let's prevent it
//     event.preventDefault();
//   }
// });
// 关闭远程桌面
const close = (msg?: Record<string, any>) => {
  const id = JSON.parse(msg?.msg).id;
  console.log(id);

  if (msg) {
    const stream = webcamStreamArr.find((item) => item.id == id);
    stream?.getTracks().forEach((track: MediaStreamTrack) => track.stop());
  } else {
    webcamStreamArr.forEach((stream) => {
      stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
    });
  }
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

// onMounted(async () => {
//   const unlisten = await appWindow.onCloseRequested(async (event) => {
//   const confirmed = await confirm('Are you sure?');
//   if (!confirmed) {
//     // user did not confirm closing the window; let's prevent it
//     event.preventDefault();
//   }
// });

// you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
</script>

<template>
  <div
    v-if="data.isConnecting"
    class="connecting-message sidebarr"
    style="position: fixed; top: 0; left: 0; right: 0; bottom: 0"
  >
    正在被远控{{ data.screenChangesignal }}个窗口...
  </div>
  <button
    v-if="data.isConnecting"
    class="close-btn"
    @click="closeRemoteDesktop()"
  >
    结束所有被控
  </button>
  <div v-if="!data.isConnecting" class="sidebar">
    <div>
      <p>
        address: <span>{{ data.account.id }}</span>
      </p>
      <p>
        password: <span>{{ data.account.password }}</span>
      </p>
    </div>
  </div>
  <div v-if="!data.isConnecting" class="form">
    <input
      v-model="data.receiverAccount.id"
      type="text"
      placeholder="请输入对方id"
    />
    <input
      v-model="data.receiverAccount.password"
      type="text"
      placeholder="请输入对方密码"
    />
    <button @click="remoteDesktop()">发起远程</button>
  </div>
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

  > div {
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
  z-index: 9999;
}

.close-btn {
  width: 60px;
  height: 24px;
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 1;
  background: #d71526;
  font-size: 12px;
}
</style>
