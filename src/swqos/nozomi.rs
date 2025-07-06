
use crate::swqos::common::{poll_transaction_confirmation, serialize_transaction_and_encode};
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde_json::json;
use std::{sync::Arc, time::Instant};

use solana_sdk::{signature::Signature};

use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;

use anyhow::Result;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{ClientType, TradeType};
use crate::swqos::SwqosClientTrait;

use crate::{common::SolanaRpcClient, constants::pumpfun::accounts::NOZOMI_TIP_ACCOUNTS};


#[derive(Clone)]
pub struct NozomiClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub endpoint: String,
    pub auth_token: String,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl SwqosClientTrait for NozomiClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NOZOMI_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NOZOMI_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::Nozomi
    }
}

impl NozomiClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(64)
            .tcp_keepalive(Some(Duration::from_secs(1200)))
            .http2_keep_alive_interval(Duration::from_secs(15))
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self { rpc_client: Arc::new(rpc_client), endpoint, auth_token, http_client }
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        let start_time = Instant::now();
        let (content, signature) = serialize_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        println!(" 交易编码base64: {:?}", start_time.elapsed());

        // 按照 Nozomi 文档要求构建请求体
        let request_body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                content,
                { "encoding": "base64" }
            ]
        }))?;

        let mut url = String::with_capacity(self.endpoint.len() + self.auth_token.len() + 20);
        url.push_str(&self.endpoint);
        url.push_str("/?c=");
        url.push_str(&self.auth_token);

        let response_text = self.http_client.post(&url)
            .body(request_body)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if response_json.get("result").is_some() {
                println!(" nozomi{}提交: {:?}", trade_type, start_time.elapsed());
            } else if let Some(_error) = response_json.get("error") {
                // eprintln!("nozomi交易提交失败: {:?}", _error);
            }
        }

        let start_time: Instant = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(_) => (),
            Err(_) => (),
        }

        println!(" nozomi{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(signature)
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        let mut signatures = Vec::new();
        for transaction in transactions {
            let signature = self.send_transaction(trade_type, transaction).await?;
            signatures.push(signature);
        }
        Ok(signatures)
    }
}