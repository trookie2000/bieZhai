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
import  eventBus from '../common/eventBus';  // 引入事件总线
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
  deviceList: [] as { ip: string, password: string }[], // List to store devices
});

const isDeviceListOpen = ref(false);

const toggleDeviceList = () => {
  isDeviceListOpen.value = !isDeviceListOpen.value;
};

const removeDevice = (index: number) => {
  data.deviceList.splice(index, 1);
};


// 对象用于引用视频元素，DOM对象s
const desktop = ref<HTMLVideoElement>();

// WebSocket 连接和RTC其他变量
let ws: WebSocket;
let pc: RTCPeerConnection;
let dc: RTCDataChannel;
let webcamStreamArr: MediaStream[] = [];
let remoteDesktopDpi: Record<string, any>;
let unlisten: Function | null = null;

onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});

onMounted(() => {
  eventBus.on('addDevice', (deviceIp: any) => {
  console.log("接收到事件 addDevice设备IP:", deviceIp);
  if (!data.deviceList.some(device => device.ip === deviceIp)) {
    data.deviceList.push({
      ip: deviceIp,
      password: '',  // 或者填充其他所需的值
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
      unlisten = unlistenFn;
    });
});

onUnmounted(() => {
  if (unlisten) {
    unlisten();
  }
});

// 初始化 WebSocket 连接
const initWebSocket = () => {
  ws = new WebSocket(`ws://192.168.1.10:8081/conn/${data.account.id}`);

  ws.onopen = (e: Event) => {
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

const handleVideoAnswerMsg = async (msg: Record<string, any>) => {
  const desc = new RTCSessionDescription(JSON.parse(msg.msg));
  await pc.setRemoteDescription(desc).catch(reportError);
};

const handleNewICECandidateMsg = async (msg: Record<string, any>) => {
  const candidate = new RTCIceCandidate(JSON.parse(msg.msg));
  try {
    await pc.addIceCandidate(candidate);
  } catch (err) {
    reportError(err);
  }
};

const handleRemoteDesktopRequest = async (msg: Record<string, any>) => {
  // if (msg.msg != data.account.password) {
  //   console.log("密码错误！");
  //   return;
  // }

  data.receiverAccount.id = msg.sender;

  await initRTCPeerConnection();
  initRTCDataChannel();

  const webcamStream: any = await navigator.mediaDevices.getDisplayMedia({
    video: true,
    audio: false,
  });
  data.screenChangesignal++;
  webcamStreamArr.push(webcamStream);

  webcamStream.oninactive = (e: any) => {
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
      data.isConnecting = false;
    }
  };

  webcamStream.getTracks().forEach((track: MediaStreamTrack) => {
    pc.addTrack(track, webcamStream);
  });

  sendOffer();
};

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

const handleICEConnectionStateChangeEvent = (event: Event) => {
  console.log("*** ICE 连接状态变为" + pc.iceConnectionState);
};

const handleICEGatheringStateChangeEvent = (event: Event) => {
  console.log("*** ICE 聚集状态变为" + pc.iceGatheringState);
};

const handleSignalingStateChangeEvent = (event: Event) => {
  console.log("*** WebRTC 信令状态变为: " + pc.signalingState);
};

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

const initRTCDataChannel = () => {
  dc = pc.createDataChannel("my channel", {
    ordered: true,
  });

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
    };

    await sendWindowInfo();

    const intervalId = setInterval(sendWindowInfo, 1000);

    const clearWindowInfoInterval = () => {
      clearInterval(intervalId);
    };

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


// 请求远程桌面
const remoteDesktop = async () => {
  if (!data.receiverAccount.id) {
    alert("请输入IP地址");
    return;
  }
  eventBus.emit('event');
  // 判断是否已存在相同的IP
  // const exists = data.deviceList.some(device => device.ip === data.receiverAccount.id);
  // if (!exists) {
  //   data.deviceList.push({
  //     ip: data.receiverAccount.id,
  //     password: data.receiverAccount.password,
  //   });
  // }
  const webview = new WebviewWindow("1", {
    url: "#/screenOne",
  });

  webview.once("tauri://created", function () { });

  webview.once("tauri://error", function (e) {
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

    webcamStreamArr.forEach((stream) => {
      stream.getTracks().forEach((track) => {
        track.stop();
      });
    });
    webcamStreamArr = [];
    close();
    sendToServer({
      msg_type: MessageType.STOP_SHARING,
      receiver: data.receiverAccount.id,
      msg: data.receiverAccount.password,
      sender: data.account.id,
    });
  }
};

const close = (msg?: Record<string, any>) => {
  const id = JSON.parse(msg?.msg).id;
  if (msg) {
    const stream = webcamStreamArr.find((item) => item.id == id);
    stream?.getTracks().forEach((track: MediaStreamTrack) => track.stop());
  } else {
    webcamStreamArr.forEach((stream) => {
      stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
    });
  }
};

const sendToServer = (msg: Record<string, any>) => {
  let msgJSON = JSON.stringify(msg);
  ws.send(msgJSON);
};

const sendToClient = (msg: Record<string, any>) => {
  let msgJSON = JSON.stringify(msg);
  dc.readyState == "open" && dc.send(msgJSON);
};

const selectDevice = (device:{ip:string}) => {
  console.log(`Selected device IP: ${device.ip}`);
  data.receiverAccount.id = device.ip;
  // data.receiverAccount.password = device.password;
};
</script>
<template>
  <div v-if="data.isConnecting" class="connecting-message" style="position: fixed; top: 0; left: 0; right: 0; bottom: 0">
    正在被远控{{ data.screenChangesignal }}个窗口...
  </div>
  <button v-if="data.isConnecting" class="close-btn" @click="closeRemoteDesktop()">
    结束被控
  </button>
  <div class="container">
    <div class="sidebar">
      <div class="device-list-container">
        <div @click="toggleDeviceList" class="device-list-title">
          <i class="icon fas fa-desktop"></i>设备列表
          <i :class="['icon', 'fas', isDeviceListOpen ? 'fa-chevron-down' : 'fa-chevron-right']"></i>
        </div>
        <ul v-show="isDeviceListOpen" class="device-list">
          <li v-for="(device, index) in data.deviceList" :key="index" class="device-item">
            <div class="device-item-content" @click="selectDevice(device)">
              <span>
                {{ device.ip }}
              </span>
              <span class="close-btn" @click.stop="removeDevice(index)">
                &times;
              </span>
            </div>
          </li>
        </ul>
      </div>
    </div>
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
