use actix_web::{HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt;

use crate::startup::AppState;

pub fn build_internal_scope() -> actix_web::Scope {
    web::scope("/_live")
        .route("/health", web::get().to(|| async { "OK" }))
        .route("/script.js", web::get().to(script))
        .route("/ws", web::get().to(ws_handler))
}

async fn script() -> HttpResponse {
    HttpResponse::Ok()
        .append_header(("Cache-Control", "no-store, max-age=0"))
        .content_type("application/javascript")
        .body(include_str!("./js/script.js"))
}

async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;
    let mut rx = state.broadcaster.subscribe();

    let mut session_for_messages = session.clone();

    actix_web::rt::spawn(async move {
        while let Some(Ok(message)) = msg_stream.next().await {
            match message {
                Message::Ping(bytes) => {
                    if session_for_messages.pong(&bytes).await.is_err() {
                        break;
                    }
                }
                Message::Close(reason) => {
                    let _ = session_for_messages.close(reason).await;
                    break;
                }
                Message::Text(_)
                | Message::Binary(_)
                | Message::Continuation(_)
                | Message::Pong(_) => {}
                Message::Nop => {}
            }
        }
    });

    actix_web::rt::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(payload) => {
                    if session.text(payload).await.is_err() {
                        break;
                    }
                }
                Err(error) => {
                    eprintln!("[web-dev-server] failed to serialize live message: {error}")
                }
            }
        }
    });

    Ok(response)
}
