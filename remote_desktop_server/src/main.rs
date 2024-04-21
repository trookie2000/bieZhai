use actix::prelude::*;
use actix_web::{
    web::{self},
    Error, HttpRequest, HttpResponse, Result,
};
use actix_web_actors::ws;

mod server;
mod session;

// 处理 WebSocket 连接的路由函数
async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<server::ChatServer>>,
) -> Result<HttpResponse, Error> {
    let path = req.path();

    
    let uuid = &path[6..path.len()];

    // 启动 WebSocket 连接，使用 session::MyWs 实现
    let resp = ws::start(
        session::MyWs {
            id: 0,
            uuid: uuid.to_string(),
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )?;

    Ok(resp)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use actix_web::{App, HttpServer};

    let server = server::ChatServer::new().start();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server.clone()))
            .route("/conn/{account}", web::get().to(chat_route))
    })
    .bind(("0.0.0.0", 8081))? 
    .run() 
    .await 
}
