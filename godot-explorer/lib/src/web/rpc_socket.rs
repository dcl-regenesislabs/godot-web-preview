//! dcl-rpc `WebSocket` over the browser WebSocket, mirroring the tungstenite client's API.

use std::sync::Arc;

use async_trait::async_trait;
use dcl_rpc::transports::web_sockets::{Error, Message, WebSocket};
use dcl_web_websocket as browser;

pub struct BrowserRpcSocket(browser::WebSocket);

pub struct WebSocketClient;

impl WebSocketClient {
    pub async fn connect(url: &str) -> Result<Arc<BrowserRpcSocket>, Error> {
        browser::WebSocket::connect(url, None)
            .await
            .map(|ws| Arc::new(BrowserRpcSocket(ws)))
            .map_err(|e| Error::Other(Box::new(e)))
    }
}

#[async_trait]
impl WebSocket for BrowserRpcSocket {
    async fn send(&self, message: Message) -> Result<(), Error> {
        let message = match message {
            Message::Text(t) => browser::Message::Text(t),
            Message::Binary(b) => browser::Message::Binary(b),
            // The browser answers protocol-level pings itself and exposes neither to scripts.
            Message::Ping | Message::Pong => return Ok(()),
            Message::Close => {
                self.0.close();
                return Ok(());
            }
        };
        self.0.send(message).map_err(|_| Error::ConnectionClosed)
    }

    async fn receive(&self) -> Option<Result<Message, Error>> {
        match self.0.receive().await? {
            Ok(browser::Message::Text(t)) => Some(Ok(Message::Text(t))),
            Ok(browser::Message::Binary(b)) => Some(Ok(Message::Binary(b))),
            Err(e) => Some(Err(Error::Other(Box::new(e)))),
        }
    }

    async fn close(&self) -> Result<(), Error> {
        self.0.close();
        Ok(())
    }
}
