use std::{
    fs::{create_dir, File},
    io::{Read, Write},
};

use intmax_zkp_core::{
    rollup::gadgets::deposit_block::{DepositInfo, VariableIndex},
    sparse_merkle_tree::goldilocks_poseidon::WrappedHashOut,
    transaction::asset::{Asset, TokenKind},
    zkdsa::account::{Account, Address},
};
use plonky2::{
    field::types::Field,
    plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
};
use structopt::StructOpt;

use crate::{
    service::*,
    utils::key_management::{memory::WalletOnMemory, types::Wallet},
};

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

const DEFAULT_AGGREGATOR_URL: &str = "http://localhost:8080";

#[derive(Debug, StructOpt)]
#[structopt(name = "intmax")]
struct Cli {
    #[structopt(subcommand)]
    pub sub_command: SubCommand,
}

#[derive(Debug, StructOpt)]
enum SubCommand {
    /// Config
    #[structopt(name = "config")]
    Config {
        #[structopt(subcommand)]
        config_command: ConfigCommand,
    },
    /// Account
    #[structopt(name = "account")]
    Account {
        #[structopt(subcommand)]
        account_command: AccountCommand,
    },
    /// Deposit
    #[structopt(name = "deposit")]
    Deposit {
        #[structopt(long)]
        user_address: Option<Address<F>>,
        // #[structopt(long)]
        // contract_address: Address<F>,
        /// `token-id` can be selected 0x00 - 0xff.
        #[structopt(long = "token-id", short = "i")]
        token_id: VariableIndex<F>,
        /// `amount` must be a positive integer less than 2^56.
        #[structopt(long)]
        amount: u64,
    },
    /// Assets
    #[structopt(name = "assets")]
    Assets {
        #[structopt(long)]
        user_address: Option<Address<F>>,
        #[structopt(long)]
        verbose: bool,
    },
    /// Transaction
    #[structopt(name = "tx")]
    Transaction {
        #[structopt(subcommand)]
        tx_command: TransactionCommand,
    },
    /// Block
    #[structopt(name = "block")]
    Block {
        #[structopt(subcommand)]
        block_command: BlockCommand,
    },
}

#[derive(Debug, StructOpt)]
enum ConfigCommand {
    /// Set aggregator URL
    #[structopt(name = "aggregator-url")]
    AggregatorUrl { aggregator_url: Option<String> },
}

#[derive(Debug, StructOpt)]
enum AccountCommand {
    /// Initializing your wallet and delete your all accounts
    #[structopt(name = "reset")]
    Reset {},
    /// Add your account
    #[structopt(name = "add")]
    Add {
        #[structopt(long)]
        private_key: Option<WrappedHashOut<F>>,
        #[structopt(long = "default")]
        is_default: bool,
    },
    /// List your addresses
    #[structopt(name = "list")]
    List {},
    /// Set default account
    #[structopt(name = "set-default")]
    SetDefault { user_address: Option<Address<F>> },
}

#[derive(Debug, StructOpt)]
enum TransactionCommand {
    /// Send a transaction and merge your own assets.
    #[structopt(name = "send")]
    Send {
        #[structopt(long)]
        user_address: Option<Address<F>>,
        #[structopt(long)]
        receiver_address: Address<F>,
        #[structopt(long)]
        contract_address: Option<Address<F>>,
        /// the token id can be selected 0x00 - 0xff
        #[structopt(long = "token-id", short = "i")]
        token_id: VariableIndex<F>,
        /// `amount` must be a positive integer less than 2^56.
        #[structopt(long)]
        amount: u64,
        // #[structopt(long)]
        // broadcast: bool,
    },
    /// Merge your own assets.
    #[structopt(name = "merge")]
    Merge {
        #[structopt(long)]
        user_address: Option<Address<F>>,
    },
    // #[structopt(name = "multi-send")]
    // MultiSend {}
}

#[derive(Debug, StructOpt)]
enum BlockCommand {
    /// Trigger to propose a block.
    #[structopt(name = "propose")]
    Propose {},
    /// Sign the diff.
    #[structopt(name = "sign")]
    Sign {
        #[structopt(long)]
        user_address: Option<Address<F>>,
    },
    /// Trigger to approve a block.
    #[structopt(name = "approve")]
    Approve {},
    /// Verify a approved block.
    #[structopt(name = "verify")]
    Verify {
        #[structopt(long, short = "n")]
        block_number: Option<u32>,
    },
}

pub fn parse_address<W: Wallet>(
    wallet: &W,
    user_address: Option<Address<F>>,
) -> anyhow::Result<Address<F>> {
    if let Some(user_address) = user_address {
        Ok(user_address)
    } else if let Some(user_address) = wallet.get_default_account() {
        Ok(user_address)
    } else {
        Err(anyhow::anyhow!("user address was not given"))
    }
}

pub fn invoke_command() -> anyhow::Result<()> {
    let mut intmax_dir = dirs::home_dir().expect("fail to get home directory");
    intmax_dir.push(".intmax");

    if File::open(intmax_dir.clone()).is_err() {
        create_dir(intmax_dir.clone()).unwrap();
        println!("make directory: {}", intmax_dir.to_string_lossy());
    }

    let mut config_file_path = intmax_dir.clone();
    config_file_path.push("config");

    let service = if let Ok(mut file) = File::open(config_file_path.clone()) {
        let mut encoded_service = String::new();
        file.read_to_string(&mut encoded_service)?;
        serde_json::from_str(&encoded_service).unwrap()
    } else {
        Config::new(DEFAULT_AGGREGATOR_URL)
    };

    let mut wallet_dir_path = intmax_dir.clone();
    let aggregator_url = service
        .aggregator_api_url("")
        .split("://")
        .last()
        .unwrap()
        .to_string();
    assert!(!aggregator_url.is_empty());
    wallet_dir_path.push(aggregator_url);
    let mut wallet_file_path = wallet_dir_path.clone();
    wallet_file_path.push("wallet");

    let Cli { sub_command } = Cli::from_args();

    let password = "password";
    let mut wallet = if let SubCommand::Account {
        account_command: AccountCommand::Reset {},
    } = sub_command
    {
        WalletOnMemory::new(password.to_string())
    } else if let Ok(mut file) = File::open(wallet_file_path.clone()) {
        let mut encoded_wallet = String::new();
        file.read_to_string(&mut encoded_wallet)?;
        serde_json::from_str(&encoded_wallet).unwrap()
    } else {
        WalletOnMemory::new(password.to_string())
    };

    match sub_command {
        SubCommand::Config { config_command } => match config_command {
            ConfigCommand::AggregatorUrl { aggregator_url } => {
                service.set_aggregator_url(aggregator_url);
            }
        },
        SubCommand::Account { account_command } => match account_command {
            AccountCommand::Reset {} => {}
            AccountCommand::Add {
                private_key,
                is_default,
            } => {
                let private_key = private_key
                    // .map(|v| WrappedHashOut::from_str(&v).expect("fail to parse user address"))
                    .unwrap_or_else(WrappedHashOut::rand);
                let account = Account::new(*private_key);
                service.register_account(account.public_key);
                wallet.add_account(account);
                let user_state = wallet
                    .data
                    .get_mut(&account.address)
                    .expect("user address was not found in wallet");

                let latest_block = service
                    .get_latest_block()
                    .expect("fail to fetch latest block");
                // dbg!(latest_block.header.block_number);
                let last_seen_block_number = latest_block.header.block_number;
                user_state.last_seen_block_number = last_seen_block_number;

                println!("new account added: {}", account.address);

                if is_default {
                    wallet.set_default_account(Some(account.address));
                    println!("set above account as default");
                }

                service.trigger_propose_block();
                service.trigger_approve_block();
            }
            AccountCommand::List {} => {
                let account_list = wallet.data.keys();

                let mut is_empty = true;
                for address in account_list {
                    is_empty = false;

                    if Some(*address) == wallet.get_default_account() {
                        println!("{} (default)", address);
                    } else {
                        println!("{}", address);
                    }
                }

                if is_empty {
                    println!(
                        "No accounts is in your wallet. Please execute `account add --default`."
                    );
                }
            }
            AccountCommand::SetDefault { user_address } => {
                let account_list = wallet.data.keys().cloned().collect::<Vec<_>>();
                if let Some(user_address) = user_address && !account_list.iter().any(|v| v == &user_address) {
                    println!("given account does not exist in your wallet");
                } else {
                    wallet.set_default_account(user_address);
                    if let Some(user_address) = user_address {
                        println!("set default account: {}", user_address);
                    } else {
                        println!("set default account: null");
                    }
                }
            }
        },
        SubCommand::Deposit {
            user_address,
            token_id: variable_index,
            amount,
        } => {
            let user_address =
                parse_address(&wallet, user_address).expect("user address was not given");
            let _user_state = wallet
                .data
                .get(&user_address)
                .expect("user address was not found in wallet");

            // receiver_address と同じ contract_address をもつトークンしか mint できない
            let contract_address = user_address; // serde_json::from_str(&contract_address).unwrap()

            if amount == 0 || amount >= 1u64 << 56 {
                anyhow::bail!("`amount` must be a positive integer less than 2^56");
            }

            // let variable_index = VariableIndex::from_str(&variable_index).unwrap();
            let deposit_info = DepositInfo {
                receiver_address: user_address,
                contract_address,
                variable_index,
                amount: F::from_canonical_u64(amount),
            };
            service.deposit_assets(vec![deposit_info]);

            service.trigger_propose_block();
            service.trigger_approve_block();
        }
        SubCommand::Assets {
            user_address,
            verbose,
        } => {
            let user_address =
                parse_address(&wallet, user_address).expect("user address was not given");
            let user_state = wallet
                .data
                .get(&user_address)
                .expect("user address was not found in wallet");

            let total_amount_map = user_state.assets.calc_total_amount();

            let separator = "-----------------------------------------------------------------------------------------";
            println!("User: {}", user_address);
            println!("{}", separator);
            if total_amount_map.is_empty() {
                println!("  No assets held");
                println!("{}", separator);
            } else {
                for (kind, total_amount) in total_amount_map {
                    println!("  Contract Address | {}", kind.contract_address);
                    println!("  Token ID         | {}", kind.variable_index);
                    println!("  Amount           | {}", total_amount);
                    println!("{}", separator);
                }
            }

            if verbose {
                println!(
                    "raw data: {}",
                    serde_json::to_string(&user_state.assets).unwrap()
                );
            }
        }
        SubCommand::Transaction { tx_command } => match tx_command {
            TransactionCommand::Merge { user_address } => {
                let user_address =
                    parse_address(&wallet, user_address).expect("user address was not given");
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                service.merge_and_purge_asset(user_state, user_address, &[], false);

                service.trigger_propose_block();
                // service.sign_proposed_block(user_state, user_address);
                service.trigger_approve_block();
            }
            TransactionCommand::Send {
                user_address,
                receiver_address,
                contract_address,
                token_id: variable_index,
                amount,
            } => {
                let user_address =
                    parse_address(&wallet, user_address).expect("user address was not given");
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                // let receiver_address = Address::from_str(&receiver_address).unwrap();
                if user_address == receiver_address {
                    anyhow::bail!("cannot send asset to myself");
                }

                if amount == 0 || amount >= 1u64 << 56 {
                    anyhow::bail!("`amount` must be a positive integer less than 2^56");
                }

                // let variable_index = VariableIndex::from_str(&variable_index).unwrap();
                let output_asset = Asset {
                    kind: TokenKind {
                        contract_address: contract_address
                            // .map(|v| Address::from_str(&v).unwrap())
                            .unwrap_or(user_address),
                        variable_index,
                    },
                    amount,
                };

                service.merge_and_purge_asset(
                    user_state,
                    user_address,
                    &[(receiver_address, output_asset)],
                    true,
                );

                service.trigger_propose_block();
                service.sign_proposed_block(user_state, user_address);
                service.trigger_approve_block();
            }
        },
        SubCommand::Block { block_command } => match block_command {
            BlockCommand::Propose {} => {
                service.trigger_propose_block();
            }
            BlockCommand::Sign { user_address } => {
                println!("block sign");
                let user_address =
                    parse_address(&wallet, user_address).expect("user address was not given");
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");
                service.sign_proposed_block(user_state, user_address);
            }
            BlockCommand::Approve {} => {
                service.trigger_approve_block();
            }
            BlockCommand::Verify { block_number } => {
                service.verify_block(block_number).unwrap();
            }
        },
    }

    let encoded_wallet = serde_json::to_string(&wallet).unwrap();
    std::fs::create_dir(wallet_dir_path).unwrap_or(());
    let mut file = File::create(wallet_file_path)?;
    write!(file, "{}", encoded_wallet)?;
    file.flush()?;

    let encoded_service = serde_json::to_string(&service).unwrap();
    let mut file = File::create(config_file_path)?;
    write!(file, "{}", encoded_service)?;
    file.flush()?;

    Ok(())
}
