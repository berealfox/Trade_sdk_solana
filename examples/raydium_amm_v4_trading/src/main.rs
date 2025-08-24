use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use sol_trade_sdk::{constants::raydium_amm_v4::accounts, solana_streamer_sdk::{match_event, streaming::event_parser::protocols::raydium_amm_v4::RaydiumAmmV4SwapEvent}, trading::common::get_multi_token_balances};
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::common::EventType;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::raydium_amm_v4::parser::RAYDIUM_AMM_V4_PROGRAM_ID;
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::{Protocol, UnifiedEvent};
use sol_trade_sdk::solana_streamer_sdk::streaming::yellowstone_grpc::{
    AccountFilter, TransactionFilter,
};
use sol_trade_sdk::solana_streamer_sdk::streaming::YellowstoneGrpc;
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::SwqosConfig,
    trading::{core::params::RaydiumAmmV4Params, factory::DexType},
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
    let protocols = vec![Protocol::RaydiumAmmV4];
    // Filter accounts
    let account_include = vec![
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(), // Listen to raydium_amm_v4 program ID
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
    let event_type_filter = EventTypeFilter {
        include: vec![EventType::RaydiumAmmV4SwapBaseIn, EventType::RaydiumAmmV4SwapBaseOut],
    };

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
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                // Test code, only test one transaction
                if !ALREADY_EXECUTED.swap(true, Ordering::SeqCst) {
                    let event_clone = e.clone();
                    tokio::spawn(async move {
                        if let Err(err) = raydium_amm_v4_copy_trade_with_grpc(event_clone).await {
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
    priority_fee.rpc_unit_limit = 150000;

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

/// Raydium_amm_v4 sniper trade
/// This function demonstrates how to snipe a new token from a Raydium_amm_v4 trade event
async fn raydium_amm_v4_copy_trade_with_grpc(trade_info: RaydiumAmmV4SwapEvent) -> AnyResult<()> {
    println!("Testing Raydium_amm_v4 trading...");

    let client = create_solana_trade_client().await?;
    let slippage_basis_points = Some(100);
    let recent_blockhash = client.rpc.get_latest_blockhash().await?;

    let amm_info =
        sol_trade_sdk::trading::raydium_amm_v4::common::fetch_amm_info(&client.rpc, trade_info.amm)
            .await?;
    let (coin_reserve, pc_reserve) =
        get_multi_token_balances(&client.rpc, &amm_info.token_coin, &amm_info.token_pc).await?;
    let mint_pubkey = if amm_info.pc_mint == accounts::WSOL_TOKEN_ACCOUNT {
        amm_info.coin_mint
    } else {
        amm_info.pc_mint
    };
    let params = RaydiumAmmV4Params::from_amm_info_and_reserves(
        trade_info.amm,
        amm_info,
        coin_reserve,
        pc_reserve,
    );
    // Buy tokens
    println!("Buying tokens from Raydium_amm_v4...");
    let buy_sol_amount = 100_000;
    client
        .buy(
            DexType::RaydiumAmmV4,
            mint_pubkey,
            buy_sol_amount,
            slippage_basis_points,
            recent_blockhash,
            None,
            Box::new(params),
            None,
            true,
        )
        .await?;

    // Sell tokens
    println!("Selling tokens from Raydium_amm_v4...");

    let rpc = client.rpc.clone();
    let payer = client.payer.pubkey();
    let account = get_associated_token_address(&payer, &mint_pubkey);
    let balance = rpc.get_token_account_balance(&account).await?;
    println!("Balance: {:?}", balance);
    let amount_token = balance.amount.parse::<u64>().unwrap();

    println!("Selling {} tokens", amount_token);
    let params = RaydiumAmmV4Params::from_amm_address_by_rpc(&client.rpc, trade_info.amm).await?;
    client
        .sell(
            DexType::RaydiumAmmV4,
            mint_pubkey,
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Box::new(params),
            None,
            true,
        )
        .await?;

    // Exit program
    std::process::exit(0);
}
