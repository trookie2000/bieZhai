//! `ChatServer` 是一个 Actor，它维护连接的客户端会话列表，并管理可用的房间。同一个房间中的对等方可以通过 `ChatServer` 互相发送消息。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use actix::prelude::*;

/// Chat server sends this messages to session
/// Chat 服务器向会话发送这些消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

/// 表示聊天服务器，它维护连接的会话列表并管理消息广播。
pub struct ChatServer {
    sessions: HashMap<String, Recipient<Message>>,
}

impl ChatServer {
    /// 创建 ChatServer 的新实例。
    pub fn new() -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
        }
    }
}

impl ChatServer {
    /// 向房间中的所有用户发送消息。
    fn send_message(&self, msg_type: String, receiver: String, message: String) {
        match msg_type.as_str() {
            "heartbeat" => {
                // println!("");
            }
            _ => {
                if let Some(addr) = self.sessions.get(&receiver) {
                    addr.do_send(Message(message));
                }
            }
        }
    }
}

/// 从 `ChatServer` 创建一个 Actor
impl Actor for ChatServer {
    /// 我们将使用简单的 Context，我们只需要与其他 Actor 进行通信的能力。
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub uuid: String,
    pub addr: Recipient<Message>,
}

/// 处理 Connect 消息的处理程序。
///
/// 注册新会话并为该会话分配唯一 ID
impl Handler<Connect> for ChatServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        let uuid = msg.uuid;
        self.sessions.insert(uuid, msg.addr);
        0
    }
}

/// 表示会话已断开
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub uuid: String,
}

/// 处理 Disconnect 消息的处理程序。
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        // 移除地址
        self.sessions.remove(&msg.uuid);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientMessage {
    pub msg_type: String,
    pub receiver: String,
    pub sender: String,
    pub msg: String,
}

/// 处理 ClientMessage 消息的处理程序。
impl Handler<ClientMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        let msg_type = msg.msg_type.clone();
        match msg_type.as_str() {
            "heartbeat" => {
                println!("WebSocket连接正常")
            }
            _ => {
                let receiver = msg.receiver.clone();
                let msg_type = msg.msg_type.clone();
                let json = serde_json::to_string(&msg).unwrap();
                self.send_message(msg_type, receiver, json);
            }
        }
    }
}
