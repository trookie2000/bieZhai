<script setup lang="ts">
import { ref, reactive, onBeforeMount } from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow, WebviewWindow } from "@tauri-apps/api/window";
// import TauriWebsocket from 'tauri-plugin-websocket-api';
// import WebSocket from "tauri-plugin-websocket-api";
import {
  MessageType,
} from "../common/Constans";

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

let ws: WebSocket;
let dc: RTCDataChannel;
onBeforeMount(async () => {
  data.account = await invoke("generate_account");
  initWebSocket();
});

/********************************* connect *************************************/
const initWebSocket = () => {
  ws = new WebSocket(`ws://10.134.180.11:8081/conn/${data.account.id}`);
  ws.onerror = (e: Event) => {
    console.log("WebSocket 连接错误:", e);
  };
};

const remoteDesktop = async () => {
  if (!data.receiverAccount.id || !data.receiverAccount.password) {
    alert("请输入ID和密码");
    return;
  }
  const uniqueLabel = `webview_${Date.now()}`;

  const webview = new WebviewWindow(uniqueLabel, {
    url: '#/screenOne',
  });

  webview.once('tauri://created', function (e) {
    console.error('Webview success:', e);
  });

  webview.once('tauri://error', function (e) {
    console.error('Webview error:', e);
  });

  sendToServer({
    msg_type: MessageType.REMOTE_DESKTOP,
    receiver: data.receiverAccount.id,
    msg: data.receiverAccount.password,
    sender: data.account.id,
  });
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
        address: <span>{{ data.account.id }}</span>
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


</style>
