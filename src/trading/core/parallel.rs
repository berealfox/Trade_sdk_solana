use anyhow::{anyhow, Result};
use solana_hash::Hash;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair};
use std::{str::FromStr, sync::Arc};
use tokio::task::JoinHandle;

use crate::{
    common::PriorityFee,
    swqos::{ClientType, FeeClient, TradeType},
    trading::common::{
        build_rpc_transaction, build_sell_tip_transaction_with_priority_fee,
        build_sell_transaction, build_tip_transaction_with_priority_fee,
    },
};

/// 并行执行交易的通用函数
pub async fn parallel_execute_with_tips(
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    instructions: Vec<Instruction>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    data_size_limit: u32,
    trade_type: TradeType,
) -> Result<()> {
    let cores = core_affinity::get_core_ids().unwrap();
    let mut handles: Vec<JoinHandle<Result<()>>> = vec![];

    for i in 0..fee_clients.len() {
        let fee_client = fee_clients[i].clone();
        let payer = payer.clone();
        let instructions = instructions.clone();
        let mut priority_fee = priority_fee.clone();
        let core_id = cores[i % cores.len()];

        let handle = tokio::spawn(async move {
            core_affinity::set_for_current(core_id);
            let transaction = if matches!(trade_type, TradeType::Sell)
                && fee_client.get_client_type() == ClientType::Rpc
            {
                build_sell_transaction(
                    payer,
                    &priority_fee,
                    instructions,
                    lookup_table_key,
                    recent_blockhash,
                )
                .await?
            } else if matches!(trade_type, TradeType::Sell)
                && fee_client.get_client_type() != ClientType::Rpc
            {
                let tip_account = fee_client.get_tip_account()?;
                let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
                build_sell_tip_transaction_with_priority_fee(
                    payer,
                    &priority_fee,
                    instructions,
                    &tip_account,
                    lookup_table_key,
                    recent_blockhash,
                )
                .await?
            } else if fee_client.get_client_type() == ClientType::Rpc {
                build_rpc_transaction(
                    payer,
                    &priority_fee,
                    instructions,
                    lookup_table_key,
                    recent_blockhash,
                    data_size_limit,
                )
                .await?
            } else {
                let tip_account = fee_client.get_tip_account()?;
                let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
                priority_fee.buy_tip_fee = priority_fee.buy_tip_fees[i];

                build_tip_transaction_with_priority_fee(
                    payer,
                    &priority_fee,
                    instructions,
                    &tip_account,
                    lookup_table_key,
                    recent_blockhash,
                    data_size_limit,
                )
                .await?
            };

            fee_client
                .send_transaction(trade_type, &transaction)
                .await?;
            Ok::<(), anyhow::Error>(())
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    let mut errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => errors.push(format!("Task error: {}", e)),
            Err(e) => errors.push(format!("Join error: {}", e)),
        }
    }

    if !errors.is_empty() {
        for error in &errors {
            println!("{}", error);
        }
        return Err(anyhow!("Some tasks failed: {:?}", errors));
    }

    Ok(())
}
