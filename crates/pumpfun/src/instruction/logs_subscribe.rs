use anchor_client::solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter}
};

use anchor_client::solana_sdk::commitment_config::CommitmentConfig;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use futures::StreamExt;
use crate::instruction::{
    logs_events::DexEvent,
    logs_data::DexInstruction,
    logs_filters::LogFilter
};

/// Subscription handle containing task and unsubscribe logic
pub struct SubscriptionHandle {
    pub task: JoinHandle<()>,
    pub unsub_fn: Box<dyn Fn() + Send>,
}

impl SubscriptionHandle {
    pub async fn shutdown(self) {
        (self.unsub_fn)();
        self.task.abort();
    }
}

pub async fn create_pubsub_client(ws_url: &str) -> PubsubClient {
    PubsubClient::new(ws_url).await.unwrap()
}

/// 启动订阅
pub async fn tokens_subscription<F>(
    ws_url: &str,
    program_address: &str,
    commitment: CommitmentConfig,
    callback: F,
) -> Result<SubscriptionHandle, Box<dyn std::error::Error>>
where
    F: Fn(DexEvent) + Send + Sync + 'static,
{
    let logs_filter = RpcTransactionLogsFilter::Mentions(vec![program_address.to_string()]);

    let logs_config = RpcTransactionLogsConfig {
        commitment: Some(commitment),
    };

    // Create PubsubClient
    let sub_client = Arc::new(PubsubClient::new(ws_url).await.unwrap());

    let sub_client_clone = Arc::clone(&sub_client);

    // Create channel for unsubscribe
    let (unsub_tx, _) = mpsc::channel(1);

    // Start subscription task
    let task = tokio::spawn(async move {
        let (mut stream, _) = sub_client_clone.logs_subscribe(logs_filter, logs_config).await.unwrap();

        loop {
            let msg = stream.next().await;
            match msg {
                Some(msg) => {
                    if let Some(_err) = msg.value.err {
                        continue;
                    }

                    let instructions = LogFilter::parse_instruction(&msg.value.logs).unwrap();
                    for instruction in instructions {
                        match instruction {
                            DexInstruction::CreateToken(token_info) => {
                                callback(DexEvent::NewToken(token_info));
                            }
                            DexInstruction::Trade(trade_info) => {
                                callback(DexEvent::NewTrade(trade_info));
                            }
                            _ => {}
                        }
                    }
                }
                None => {
                    println!("Token subscription stream ended");
                }
            }   
        }
    });

    // Return subscription handle and unsubscribe logic
    Ok(SubscriptionHandle {
        task,
        unsub_fn: Box::new(move || {
            let _ = unsub_tx.try_send(());
        }),
    })
}

pub async fn stop_subscription(handle: SubscriptionHandle) {
    (handle.unsub_fn)();
    handle.task.abort();
}
