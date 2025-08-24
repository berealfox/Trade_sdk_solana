use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use sol_trade_sdk::solana_streamer_sdk::match_event;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::EventType;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::parser::PUMPFUN_PROGRAM_ID;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::{Protocol, UnifiedEvent};
use sol_trade_sdk::solana_streamer_sdk::streaming::yellowstone_grpc::{
    AccountFilter, TransactionFilter,
};
use sol_trade_sdk::solana_streamer_sdk::streaming::YellowstoneGrpc;
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::SwqosConfig,
    trading::{core::params::PumpFunParams, factory::DexType},
    SolanaTrade,
};
use solana_sdk::signer::Signer;
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair};
use spl_associated_token_account::get_associated_token_address;

// Global static flag to ensure transaction is executed only once
static ALREADY_EXECUTED: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subscribing to GRPC events...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    let callback = create_event_callback();
    let protocols = vec![Protocol::PumpFun];
    // Filter accounts
    let account_include = vec![
        PUMPFUN_PROGRAM_ID.to_string(), // Listen to pumpfun program ID
    ];
    let account_exclude = vec![];
    let account_required = vec![];

    // Listen to transaction data
    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude,
        account_required,
    };

    // Listen to account data belonging to owner programs -> account event monitoring
    let account_filter = AccountFilter { account: vec![], owner: vec![] };

    // listen to specific event type
    let event_type_filter =
        EventTypeFilter { include: vec![EventType::PumpFunBuy, EventType::PumpFunSell] };

    grpc.subscribe_events_immediate(
        protocols,
        None,
        transaction_filter,
        account_filter,
        Some(event_type_filter),
        None,
        callback,
    )
    .await?;

    tokio::signal::ctrl_c().await?;

    Ok(())
}

/// Create an event callback function that handles different types of events
fn create_event_callback() -> impl Fn(Box<dyn UnifiedEvent>) {
    |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                // Test code, only test one transaction
                if !ALREADY_EXECUTED.swap(true, Ordering::SeqCst) {
                    let event_clone = e.clone();
                    tokio::spawn(async move {
                        if let Err(err) = pumpfun_copy_trade_with_grpc(event_clone).await {
                            eprintln!("Error in copy trade: {:?}", err);
                            std::process::exit(0);
                        }
                    });
                }
            },
        });
    }
}

/// Create SolanaTrade client
/// Initializes a new SolanaTrade client with configuration
async fn create_solana_trade_client() -> AnyResult<SolanaTrade> {
    println!("Creating SolanaTrade client...");

    let payer = Keypair::from_base58_string("use_your_payer_keypair_here");
    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();

    let swqos_configs = vec![SwqosConfig::Default(rpc_url.clone())];

    let mut priority_fee = PriorityFee::default();
    // Configure according to your needs
    priority_fee.rpc_unit_limit = 100000;

    let trade_config = TradeConfig {
        rpc_url,
        commitment: CommitmentConfig::confirmed(),
        priority_fee: priority_fee,
        swqos_configs,
        lookup_table_key: None,
    };

    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    println!("SolanaTrade client created successfully!");

    Ok(solana_trade_client)
}

/// PumpFun sniper trade
/// This function demonstrates how to snipe a new token from a PumpFun trade event
async fn pumpfun_copy_trade_with_grpc(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    let client = create_solana_trade_client().await?;
    let mint_pubkey = trade_info.mint;
    let slippage_basis_points = Some(100);
    let recent_blockhash = client.rpc.get_latest_blockhash().await?;

    // Buy tokens
    println!("Buying tokens from PumpFun...");
    let buy_sol_amount = 100_000;
    client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            buy_sol_amount,
            slippage_basis_points,
            recent_blockhash,
            None,
            Box::new(PumpFunParams::from_trade(&trade_info, None)),
            None,
            true,
        )
        .await?;

    // Sell tokens
    println!("Selling tokens from PumpFun...");

    let rpc = client.rpc.clone();
    let payer = client.payer.pubkey();
    let account = get_associated_token_address(&payer, &mint_pubkey);
    let balance = rpc.get_token_account_balance(&account).await?;
    println!("Balance: {:?}", balance);
    let amount_token = balance.amount.parse::<u64>().unwrap();

    println!("Selling {} tokens", amount_token);
    client
        .sell(
            DexType::PumpFun,
            mint_pubkey,
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Box::new(PumpFunParams::from_trade(&trade_info, Some(true))),
            None,
            true,
        )
        .await?;

    // PumpFunParams can also be set as PumpFunParams::immediate_sell(creator_vault, close_token_account_when_sell)
    // creator_vault can be obtained from the trade event

    // Exit program
    std::process::exit(0);
}
