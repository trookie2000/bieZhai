use actix::prelude::*;
use actix_web_actors::ws;

use crate::server;

/// WebSocket会话处理器，用于处理与客户端的WebSocket连接
#[derive(Debug)]
pub struct MyWs {
    pub id: usize,              // 会话ID
    pub uuid: String,           // 用户ID
    pub addr: Addr<server::ChatServer>, // ChatServer的地址
}

impl Handler<server::Message> for MyWs {
    type Result = ();

    /// 处理来自ChatServer的消息，将消息文本发送给WebSocket客户端
    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    /// Actor启动时调用的方法，将会话注册到ChatServer
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.addr
            .send(server::Connect {
                uuid: self.uuid.clone(),
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    _ => ctx.stop(), // 如果出错，停止会话
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    /// Actor停止时调用的方法，通知ChatServer会话已断开
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(server::Disconnect {
            uuid: self.uuid.to_string(),
        });
        Running::Stop
    }
}

/// 用于处理来自WebSocket客户端的消息流
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let text = text.to_string();

                // 解析JSON格式的消息文本
                let message: server::ClientMessage = serde_json::from_str(&text).unwrap();

                // 将消息发送给ChatServer处理
                self.addr.do_send(message);
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            _ => (),
        }
    }
}
