# Sol Trade SDK

一个全面的 Rust SDK，用于与 Solana DEX 交易程序进行无缝交互。此 SDK 提供强大的工具和接口集，将 PumpFun、PumpSwap 和 Bonk 功能集成到您的应用程序中。

## 项目特性

1. **PumpFun 交易**: 支持`购买`、`卖出`功能
2. **PumpSwap 交易**: 支持 PumpSwap 池的交易操作
3. **Bonk 交易**: 支持 Bonk 的交易操作
4. **事件订阅**: 订阅 PumpFun、PumpSwap 和 Bonk 程序的交易事件
5. **Yellowstone gRPC**: 使用 Yellowstone gRPC 订阅程序事件
6. **ShredStream 支持**: 使用 ShredStream 订阅程序事件
7. **多种 MEV 保护**: 支持 Jito、Nextblock、ZeroSlot、Temporal、Bloxroute 等服务
8. **并发交易**: 同时使用多个 MEV 服务发送交易，最快的成功，其他失败
9. **统一交易接口**: 使用统一的交易协议枚举进行交易操作

## 安装

将此项目克隆到您的项目目录：

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/sol-trade-sdk
```

在您的`Cargo.toml`中添加依赖：

```toml
# 添加到您的 Cargo.toml
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.1.0" }
```

## 使用示例

### 1. 事件订阅 - 监听代币交易

#### 1.1 使用 Yellowstone gRPC 订阅事件

```rust
use sol_trade_sdk::{
    streaming::{
        event_parser::{
            protocols::{
                bonk::{BonkPoolCreateEvent, BonkTradeEvent},
                pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
                pumpswap::{
                    PumpSwapBuyEvent, PumpSwapCreatePoolEvent, PumpSwapDepositEvent,
                    PumpSwapSellEvent, PumpSwapWithdrawEvent,
                },
            },
            Protocol, UnifiedEvent,
        },
        YellowstoneGrpc,
    },
    match_event,
};

async fn test_grpc() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 GRPC 客户端订阅事件
    println!("正在订阅 GRPC 事件...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // 定义回调函数处理事件
    let callback = |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                println!("BonkPoolCreateEvent: {:?}", e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                println!("BonkTradeEvent: {:?}", e);
            },
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                println!("PumpFunTradeEvent: {:?}", e);
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                println!("PumpFunCreateTokenEvent: {:?}", e);
            },
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                println!("Buy event: {:?}", e);
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                println!("Sell event: {:?}", e);
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                println!("CreatePool event: {:?}", e);
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                println!("Deposit event: {:?}", e);
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                println!("Withdraw event: {:?}", e);
            },
        });
    };

    // 订阅多个协议的事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk];
    grpc.subscribe_events(protocols, None, None, None, callback)
        .await?;

    Ok(())
}
```

#### 1.2 使用 ShredStream 订阅事件

```rust
use sol_trade_sdk::streaming::ShredStreamGrpc;

async fn test_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ShredStream 客户端订阅事件
    println!("正在订阅 ShredStream 事件...");

    let shred_stream = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // 定义回调函数处理事件（与上面相同）
    let callback = |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                println!("BonkPoolCreateEvent: {:?}", e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                println!("BonkTradeEvent: {:?}", e);
            },
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                println!("PumpFunTradeEvent: {:?}", e);
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                println!("PumpFunCreateTokenEvent: {:?}", e);
            },
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                println!("Buy event: {:?}", e);
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                println!("Sell event: {:?}", e);
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                println!("CreatePool event: {:?}", e);
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                println!("Deposit event: {:?}", e);
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                println!("Withdraw event: {:?}", e);
            },
        });
    };

    // 订阅事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk];
    shred_stream
        .shredstream_subscribe(protocols, None, callback)
        .await?;

    Ok(())
}
```

### 2. 初始化 SolanaTrade 实例

```rust
use std::{str::FromStr, sync::Arc};
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::{SwqosConfig, SwqosRegion},
    SolanaTrade
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

/// 创建 SolanaTrade 客户端的示例
async fn test_create_solana_trade_client() -> AnyResult<SolanaTrade> {
    println!("Creating SolanaTrade client...");

    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

    // 配置各种 SWQOS 服务
    let swqos_configs = vec![
        SwqosConfig::Jito(SwqosRegion::Frankfurt),
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Default(rpc_url.clone()),
    ];

    // 定义交易配置
    let trade_config = TradeConfig {
        rpc_url: rpc_url.clone(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        swqos_configs,
        lookup_table_key: None,
    };

    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    println!("SolanaTrade client created successfully!");

    Ok(solana_trade_client)
}
```

### 3. PumpFun 交易操作

```rust
use sol_trade_sdk::{
    common::bonding_curve::BondingCurveAccount,
    constants::pumpfun::global_constants::TOKEN_TOTAL_SUPPLY,
    trading::{
        core::params::PumpFunParams,
        factory::TradingProtocol,
    },
};

async fn test_pumpfun_sniper_trade_width_shreds(trade_info: PumpFunTradeEvent) -> AnyResult<()> {

    println!("Testing PumpFun trading...");

    // 如果不是开发者购买，则返回
    if !trade_info.is_dev_create_token_trade {
        return Ok(());
    }

    let solana_trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let dev_sol_amount = trade_info.max_sol_cost;
    let dev_token_amount = trade_info.token_amount;
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    
    println!("Buying tokens from PumpFun...");
    
    // 不使用rpc调用获取bonding_curve，可以节约交易时间
    let bonding_curve = BondingCurveAccount::from_dev_trade(
        &mint_pubkey,
        dev_token_amount,
        dev_sol_amount,
        creator,
    );

    // 我本次交易所花的的sol金额
    let buy_sol_amount = 100_000;
 
    solana_trade_client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            buy_sol_amount,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Some(Box::new(PumpFunParams {
                bonding_curve: Some(Arc::new(bonding_curve.clone())),
            })),
        )
        .await?;

    Ok(())
}

async fn test_pumpfun_copy_trade_width_grpc(trade_info: PumpFunTradeEvent) -> AnyResult<()> {

    println!("Testing PumpFun trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from PumpFun...");

    // 我本次交易所花的的sol金额
    let buy_sol_amount = 100_000;

    // 不使用rpc调用获取bonding_curve，可以节约交易时间
    let bonding_curve = BondingCurveAccount::from_trade(&trade_info);

    solana_trade_client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            buy_sol_amount,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Some(Box::new(PumpFunParams {
                bonding_curve: Some(Arc::new(bonding_curve.clone())),
            })),
        )
        .await?;

    Ok(())
}

async fn test_pumpfun_sell(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    let amount_token = 100_000_000; 
    solana_trade_client
        .sell(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
}
```

### 4. PumpSwap 交易操作

```rust
async fn test_pumpswap() -> AnyResult<()> {
    println!("Testing PumpSwap trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;
    let creator = Pubkey::from_str("11111111111111111111111111111111")?; // dev account
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // token mint

    println!("Buying tokens from PumpSwap...");
    // buy
    solana_trade_client
        .buy(
            DexType::PumpSwap,
            mint_pubkey,
            Some(creator),
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    
    // sell
    println!("Selling tokens from PumpSwap...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::PumpSwap,
            mint_pubkey,
            Some(creator),
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}
```

### 5. Bonk 交易操作

```rust
async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Bonk trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100); // 1%
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;

    println!("Buying tokens from letsbonk.fun...");
    // buy
    solana_trade_client
        .buy(
            DexType::Bonk,
            mint_pubkey,
            None,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    
    // sell
    println!("Selling tokens from letsbonk.fun...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::Bonk,
            mint_pubkey,
            None,
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}
```

### 6. 自定义优先费用配置

```rust
use sol_trade_sdk::common::PriorityFee;

// 自定义优先费用配置
let priority_fee = PriorityFee {
    unit_limit: 190000,
    unit_price: 1000000,
    rpc_unit_limit: 500000,
    rpc_unit_price: 500000,
    buy_tip_fee: 0.001,
    buy_tip_fees: vec![0.001, 0.002],
    sell_tip_fee: 0.0001,
};

// 在TradeConfig中使用自定义优先费用
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee, // 使用自定义优先费用
    swqos_configs,
    lookup_table_key: None,
};
```

## 支持的交易平台

- **PumpFun**: 主要的 meme 币交易平台
- **PumpSwap**: PumpFun 的交换协议
- **Bonk**: 代币发行平台（letsbonk.fun）

## MEV 保护服务

- **Jito**: 高性能区块空间
- **NextBlock**: 快速交易执行
- **ZeroSlot**: 零延迟交易
- **Temporal**: 时间敏感交易
- **Bloxroute**: 区块链网络加速

## 新架构特性

### 统一交易接口

- **TradingProtocol 枚举**: 使用统一的协议枚举（PumpFun、PumpSwap、Bonk）
- **统一的 buy/sell 方法**: 所有协议都使用相同的交易方法签名
- **协议特定参数**: 每个协议都有自己的参数结构（PumpFunParams 等）

### 事件解析系统

- **统一事件接口**: 所有协议事件都实现 UnifiedEvent 特征
- **协议特定事件**: 每个协议都有自己的事件类型
- **事件工厂**: 自动识别和解析不同协议的事件

### 交易引擎

- **统一交易接口**: 所有交易操作都使用相同的方法
- **协议抽象**: 支持多个协议的交易操作
- **并发执行**: 支持同时向多个 MEV 服务发送交易

## 项目结构

```
src/
├── common/           # 通用功能和工具
├── constants/        # 常量定义
├── instruction/      # 指令构建
├── streaming/        # 事件流处理
│   ├── event_parser/ # 事件解析系统
│   │   ├── common/   # 通用事件解析工具
│   │   ├── core/     # 核心解析特征和接口
│   │   ├── protocols/# 协议特定解析器
│   │   │   ├── bonk/ # Bonk事件解析
│   │   │   ├── pumpfun/ # PumpFun事件解析
│   │   │   └── pumpswap/ # PumpSwap事件解析
│   │   └── factory.rs # 解析器工厂
│   ├── shred_stream.rs # ShredStream客户端
│   └── yellowstone_grpc.rs # Yellowstone gRPC客户端
├── swqos/            # MEV服务客户端
├── trading/          # 统一交易引擎
│   ├── common/       # 通用交易工具
│   ├── core/         # 核心交易引擎
│   ├── bonk/         # Bonk交易实现
│   ├── pumpfun/      # PumpFun交易实现
│   ├── pumpswap/     # PumpSwap交易实现
│   └── factory.rs    # 交易工厂
├── lib.rs            # 主库文件
└── main.rs           # 示例程序
```

## 许可证

MIT 许可证

## 联系方式

- 项目仓库: https://github.com/0xfnzero/sol-trade-sdk
- Telegram 群组: https://t.me/fnzero_group

## 重要注意事项

1. 在主网使用前请充分测试
2. 正确设置私钥和 API 令牌
3. 注意滑点设置避免交易失败
4. 监控余额和交易费用
5. 遵循相关法律法规

## 语言版本

- [English](README.md)
- [中文](README_CN.md)
