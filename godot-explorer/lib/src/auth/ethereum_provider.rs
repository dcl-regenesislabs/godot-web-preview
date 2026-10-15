#[cfg(not(target_family = "wasm"))]
use ethers_providers::{Provider, Ws};
#[cfg(not(target_family = "wasm"))]
use tokio::sync::Mutex;

use crate::urls;

#[cfg(not(target_family = "wasm"))]
pub struct EthereumProvider {
    provider: Mutex<Option<Provider<Ws>>>,
}

impl Default for EthereumProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_family = "wasm"))]
impl EthereumProvider {
    pub fn new() -> Self {
        Self {
            provider: Mutex::new(None),
        }
    }

    pub async fn send_async(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, anyhow::Error> {
        let mut this_provider = self.provider.lock().await;

        if this_provider.is_none() {
            let provider_url = urls::ethereum_rpc_with_project("kernel-local");
            let provider = Provider::<Ws>::connect(&provider_url).await?;
            this_provider.replace(provider);
        }

        // TODO: check if the connection is missing
        let provider = this_provider.as_ref().unwrap();
        let result = provider.request(method, params).await;

        match result {
            Err(e) => {
                this_provider.take();
                Err(anyhow::Error::new(e))
            }
            Ok(result) => Ok(result),
        }
    }
}

/// Browsers can't use the ethers WebSocket provider; the same JSON-RPC goes over HTTPS POST.
#[cfg(target_family = "wasm")]
pub struct EthereumProvider {
    client: reqwest::Client,
}

#[cfg(target_family = "wasm")]
impl EthereumProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn send_async(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, anyhow::Error> {
        let url = urls::ethereum_rpc_with_project("kernel-local").replacen("wss://", "https://", 1);
        let body =
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response: serde_json::Value = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        match response.get("error") {
            Some(error) => Err(anyhow::anyhow!("JSON-RPC error: {error}")),
            None => Ok(response
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null)),
        }
    }
}
