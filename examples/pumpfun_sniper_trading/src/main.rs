use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::EventType;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::{Protocol, UnifiedEvent};
use sol_trade_sdk::solana_streamer_sdk::{match_event, streaming::ShredStreamGrpc};
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::SwqosConfig,
    trading::{core::params::PumpFunParams, factory::DexType},
    SolanaTrade,
};
use solana_sdk::signer::Signer;
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair};
use spl_associated_token_account::get_associated_token_address;
use std::mem::take;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Atomic flag to ensure the sniper trade is executed only once
static ALREADY_EXECUTED: AtomicBool = AtomicBool::new(false);

/// Main entry point - subscribes to PumpFun events and executes sniper trades on token creation
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subscribing to ShredStream events...");
    let shred_stream = ShredStreamGrpc::new("use_your_shred_stream_url_here".to_string()).await?;
    let callback = create_event_callback();
    let protocols = vec![Protocol::PumpFun];
    let event_type_filter = EventTypeFilter {
        include: vec![EventType::PumpFunBuy, EventType::PumpFunSell, EventType::PumpFunCreateToken],
    };
    println!("Starting to listen for events, press Ctrl+C to stop...");
    shred_stream.shredstream_subscribe(protocols, None, Some(event_type_filter), callback).await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// Create an event callback function that handles different types of events
fn create_event_callback() -> impl Fn(Box<dyn UnifiedEvent>) {
    |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                // Only process developer token creation events
                if !e.is_dev_create_token_trade {
                    return;
                }
                // Ensure we only execute the trade once using atomic compare-and-swap
                if !ALREADY_EXECUTED.swap(true, Ordering::SeqCst) {
                    let event_clone = e.clone();
                    // Spawn a new task to handle the trading operation
                    tokio::spawn(async move {
                        if let Err(err) = pumpfun_sniper_trade_with_shreds(event_clone).await {
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
    // Set RPC unit limit based on your requirements
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

/// Execute PumpFun sniper trading strategy based on received token creation event
/// This function buys tokens immediately after creation and then sells all tokens
async fn pumpfun_sniper_trade_with_shreds(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    let client = create_solana_trade_client().await?;
    let mint_pubkey = trade_info.mint;
    let slippage_basis_points = Some(300);
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
            Box::new(PumpFunParams::from_dev_trade(&trade_info, None)),
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
            Box::new(PumpFunParams::immediate_sell(trade_info.creator_vault, true)),
            None,
            true,
        )
        .await?;

    // PumpFunParams can also be set as PumpFunParams::immediate_sell(creator_vault, close_token_account_when_sell)
    // creator_vault can be obtained from the trade event

    // Exit program after completing the trade
    std::process::exit(0);
}
