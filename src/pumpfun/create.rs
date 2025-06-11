use std::{str::FromStr, time::Instant, sync::Arc};

use anyhow::anyhow;
use solana_hash::Hash;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, 
    instruction::Instruction, message::{v0, VersionedMessage}, 
    pubkey::Pubkey, 
    native_token::sol_to_lamports, 
    signature::Keypair, 
    signer::Signer, 
    system_instruction, 
    transaction::{Transaction, VersionedTransaction}
};
use spl_associated_token_account::instruction::create_associated_token_account;

use crate::{
    common::{PriorityFee, SolanaRpcClient}, constants, instruction, 
    ipfs::TokenMetadataIPFS,  swqos::{FeeClient, TradeType},
};

use crate::pumpfun::common::{
    create_priority_fee_instructions, 
    get_buy_amount_with_slippage, get_global_account
};

use crate::common::tip_cache::TipCache;

use super::common::{get_bonding_curve_account, get_buy_token_amount, get_creator_vault_pda};

/// Create a new token
pub async fn create(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Keypair,
    ipfs: TokenMetadataIPFS,
    priority_fee: PriorityFee,
) -> Result<(), anyhow::Error> {
    let mut instructions = create_priority_fee_instructions(priority_fee);

    instructions.push(instruction::create(
        payer.as_ref(),
        &mint,
        instruction::Create {
            _name: ipfs.metadata.name,
            _symbol: ipfs.metadata.symbol,
            _uri: ipfs.metadata_uri,
            _creator: payer.pubkey(),
        },
    ));

    let recent_blockhash = rpc.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[payer.as_ref(), &mint],
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}

/// Create and buy tokens in one transaction
pub async fn create_and_buy(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Keypair,
    ipfs: TokenMetadataIPFS,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if buy_sol_cost == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    let mint = Arc::new(mint);
    let transaction = build_create_and_buy_transaction(rpc.clone(), payer.clone(), mint.clone(), ipfs, buy_sol_cost, slippage_basis_points, priority_fee.clone(), recent_blockhash).await?;
    rpc.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}

pub async fn create_and_buy_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Keypair,
    ipfs: TokenMetadataIPFS,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint);
    let build_instructions = build_create_and_buy_instructions(rpc.clone(), payer.clone(), mint.clone(), ipfs.clone(), buy_sol_cost, slippage_basis_points).await?;
    let mut handles = vec![];
    for fee_client in fee_clients {
        let tip_account = fee_client.get_tip_account()?;
        let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
        let transaction = build_create_and_buy_transaction_with_tip(/*rpc.clone(),*/ tip_account, payer.clone(), priority_fee.clone(), build_instructions.clone(), recent_blockhash).await?;
        let handle = tokio::spawn(async move {    
            fee_client.send_transaction(TradeType::CreateAndBuy, &transaction).await.map_err(|e| anyhow!(e.to_string()))?;
            println!("Total Jito create and buy operation time: {:?}ms", start_time.elapsed().as_millis());
            Ok::<(), anyhow::Error>(())
        });

        handles.push(handle);
    }

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => println!("Error in task: {}", e),
            Err(e) => println!("Task join error: {}", e),
        }
    }

    Ok(())
}

pub async fn build_create_and_buy_transaction(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Arc<Keypair>,
    ipfs: TokenMetadataIPFS,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    recent_blockhash: Hash,
) -> Result<Transaction, anyhow::Error> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
    ];

    let build_instructions = build_create_and_buy_instructions(rpc.clone(), payer.clone(), mint.clone(), ipfs, buy_sol_cost, slippage_basis_points).await?;
    instructions.extend(build_instructions);

    // let recent_blockhash = rpc.get_latest_blockhash().await?;
    // let recent_blockhash = Hash::default();
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[payer.as_ref(), mint.as_ref()],
        recent_blockhash,
    );

    Ok(transaction)
}

pub async fn build_create_and_buy_transaction_with_tip(
    // rpc: Arc<SolanaRpcClient>,
    tip_account: Arc<Pubkey>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,  
    build_instructions: Vec<Instruction>,
    recent_blockhash: Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let tip_cache = TipCache::get_instance();
    let tip_amount = tip_cache.get_tip();

    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
        system_instruction::transfer(
            &payer.pubkey(),
            &tip_account,
            sol_to_lamports(tip_amount),
        ),
    ];
    instructions.extend(build_instructions);

    // let recent_blockhash = rpc.get_latest_blockhash().await?;
    // let recent_blockhash = Hash::default();
    let v0_message: v0::Message =
        v0::Message::try_compile(&payer.pubkey(), &instructions, &[], recent_blockhash)?;
    
    let versioned_message: VersionedMessage = VersionedMessage::V0(v0_message);
    let transaction = VersionedTransaction::try_new(versioned_message, &[&payer])?;

    Ok(transaction)
}

pub async fn build_create_and_buy_instructions(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Arc<Keypair>,
    ipfs: TokenMetadataIPFS,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
) -> Result<Vec<Instruction>, anyhow::Error> {
    if buy_sol_cost == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    let (bonding_curve_account, bonding_curve_pda) = get_bonding_curve_account(&rpc, &mint.pubkey()).await?;
    let creator_vault_pda = get_creator_vault_pda(&bonding_curve_account.creator).unwrap();
    let (buy_token_amount, max_sol_cost) = get_buy_token_amount(&bonding_curve_account, buy_sol_cost, slippage_basis_points)?;

    let mut instructions = vec![];

    instructions.push(instruction::create(
        payer.as_ref(),
        mint.as_ref(),
        instruction::Create {
            _name: ipfs.metadata.name.clone(),
            _symbol: ipfs.metadata.symbol.clone(),
            _uri: ipfs.metadata_uri.clone(),
            _creator: payer.pubkey(),
        },
    ));

    instructions.push(create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &mint.pubkey(),
        &constants::pumpfun::accounts::TOKEN_PROGRAM,
    ));

    instructions.push(instruction::buy(
        payer.as_ref(),
        &mint.pubkey(),
        &bonding_curve_pda,
        &creator_vault_pda,
        &constants::pumpfun::global_constants::FEE_RECIPIENT,
        instruction::Buy {
            _amount: buy_token_amount,
            _max_sol_cost: max_sol_cost,
        },
    ));

    Ok(instructions)
}
