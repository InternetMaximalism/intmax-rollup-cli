use std::{
    fs::{create_dir, File},
    io::{Read, Write},
    str::FromStr,
};

use intmax_zkp_core::{
    rollup::gadgets::deposit_block::DepositInfo,
    sparse_merkle_tree::goldilocks_poseidon::GoldilocksHashOut,
    transaction::asset::{Asset, TokenKind},
    zkdsa::account::{Account, Address},
};
use plonky2::{
    field::types::{Field, Sample},
    hash::hash_types::HashOut,
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
        user_address: Option<String>,
        #[structopt(long)]
        contract_address: String,
        #[structopt(long = "token-id", short = "i")]
        variable_index: String,
        #[structopt(long)]
        amount: u64,
    },
    /// Assets
    #[structopt(name = "assets")]
    Assets {
        #[structopt(long)]
        user_address: Option<String>,
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
    #[structopt(name = "aggregator-url")]
    AggregatorUrl { aggregator_url: Option<String> },
}

#[derive(Debug, StructOpt)]
enum AccountCommand {
    #[structopt(name = "reset")]
    Reset {},
    #[structopt(name = "add")]
    Add {
        #[structopt(long)]
        private_key: Option<String>,
        #[structopt(long = "default")]
        is_default: bool,
    },
    #[structopt(name = "list")]
    List {},
    #[structopt(name = "set-default")]
    SetDefault { user_address: Option<String> },
}

#[derive(Debug, StructOpt)]
enum TransactionCommand {
    /// Send a transaction and merge your own assets.
    #[structopt(name = "send")]
    Send {
        #[structopt(long)]
        user_address: Option<String>,
        #[structopt(long)]
        receiver_address: String,
        #[structopt(long)]
        contract_address: String,
        #[structopt(long = "token-id", short = "i")]
        variable_index: String,
        #[structopt(long)]
        amount: u64,
        // #[structopt(long)]
        // broadcast: bool,
    },
    /// Merge your own assets.
    #[structopt(name = "merge")]
    Merge {
        #[structopt(long)]
        user_address: Option<String>,
    },
    // #[structopt(name = "multi-send")]
    // MultiSend {}
}

#[derive(Debug, StructOpt)]
enum BlockCommand {
    /// Reset ALL server state.
    #[structopt(name = "reset")]
    Reset {},
    /// Trigger to propose a block.
    #[structopt(name = "propose")]
    Propose {},
    /// Sign the diff.
    #[structopt(name = "sign")]
    Sign {
        #[structopt(long)]
        user_address: Option<String>,
    },
    /// Trigger to approve a block.
    #[structopt(name = "approve")]
    Approve {},
}

pub fn parse_address<W: Wallet>(
    wallet: &W,
    user_address: Option<String>,
) -> anyhow::Result<Address<F>> {
    if let Some(user_address) = user_address {
        Ok(Address::from_str(&user_address[2..]).expect("fail to parse user address"))
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

    let mut wallet_file_path = intmax_dir.clone();
    wallet_file_path.push("wallet");

    let mut config_file_path = intmax_dir.clone();
    config_file_path.push("config");

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

    let service = if let Ok(mut file) = File::open(config_file_path.clone()) {
        let mut encoded_service = String::new();
        file.read_to_string(&mut encoded_service)?;
        serde_json::from_str(&encoded_service).unwrap()
    } else {
        Config::new(DEFAULT_AGGREGATOR_URL)
    };

    match sub_command {
        SubCommand::Config { config_command } => match config_command {
            ConfigCommand::AggregatorUrl { aggregator_url } => {
                service.set_aggregator_url(aggregator_url);
            }
        },
        SubCommand::Account { account_command } => {
            match account_command {
                AccountCommand::Reset {} => {}
                AccountCommand::Add {
                    private_key,
                    is_default,
                } => {
                    let private_key = if let Some(private_key) = private_key {
                        *GoldilocksHashOut::from_str(&private_key[2..])
                            .expect("fail to parse user address")
                    } else {
                        HashOut::rand()
                    };
                    let account = Account::new(private_key);
                    wallet.add_account(account);
                    println!("new account added: 0x{}", account.address);

                    if is_default {
                        wallet.set_default_account(Some(account.address));
                        println!("set above account as default");
                    }
                }
                AccountCommand::List {} => {
                    let account_list = wallet.data.keys();

                    let mut is_empty = true;
                    for address in account_list {
                        is_empty = false;

                        if Some(*address) == wallet.get_default_account() {
                            println!("0x{} (default)", address);
                        } else {
                            println!("0x{}", address);
                        }
                    }

                    if is_empty {
                        println!("No accounts is in your wallet. Please execute `account add --default`.");
                    }
                }
                AccountCommand::SetDefault { user_address } => {
                    if let Some(user_address) = user_address {
                        let user_address = Address::from_str(&user_address[2..])
                            .expect("fail to parse user address");
                        wallet.set_default_account(Some(user_address));
                        println!("set default account: 0x{}", user_address);
                    } else {
                        wallet.set_default_account(None);
                        println!("set default account: null");
                    }
                }
            }
        }
        SubCommand::Deposit {
            user_address,
            contract_address,
            variable_index,
            amount,
        } => {
            let user_address =
                parse_address(&wallet, user_address).expect("user address was not given");
            let _user_state = wallet
                .data
                .get(&user_address)
                .expect("user address was not found in wallet");

            // let mut decoded_contract_address = hex::decode(contract_address).unwrap();
            // decoded_contract_address.reverse();
            // decoded_contract_address.resize(32, 0);
            // decoded_contract_address.reverse();
            let deposit_info = DepositInfo {
                receiver_address: user_address,
                contract_address: Address::from_str(&contract_address[2..]).unwrap(),
                variable_index: GoldilocksHashOut::from_str(&variable_index[2..]).unwrap().0,
                amount: F::from_canonical_u64(amount),
            };
            service.deposit_assets(vec![deposit_info]);
        }
        SubCommand::Assets { user_address } => {
            let user_address =
                parse_address(&wallet, user_address).expect("user address was not given");
            let user_state = wallet
                .data
                .get(&user_address)
                .expect("user address was not found in wallet");

            let total_amount_map = user_state.assets.calc_total_amount();

            let separator = "-----------------------------------------------------------------------------------------";
            println!("{}", separator);
            if total_amount_map.is_empty() {
                println!("  No assets held");
                println!("{}", separator);
            } else {
                for (kind, total_amount) in total_amount_map {
                    println!("  Contract Address | 0x{}", kind.contract_address);
                    println!("  Token ID         | 0x{}", kind.variable_index);
                    println!("  Amount           | {}", total_amount);
                    println!("{}", separator);
                }
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
            }
            TransactionCommand::Send {
                user_address,
                receiver_address,
                contract_address,
                variable_index,
                amount,
            } => {
                let user_address =
                    parse_address(&wallet, user_address).expect("user address was not given");
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                let receiver_address = Address::from_str(&receiver_address[2..]).unwrap();
                if user_address == receiver_address {
                    anyhow::bail!("cannot send asset to myself");
                }

                let output_asset = Asset {
                    kind: TokenKind {
                        contract_address: Address::from_str(&contract_address[2..]).unwrap(),
                        variable_index: GoldilocksHashOut::from_str(&variable_index[2..]).unwrap(),
                    },
                    amount,
                };

                service.merge_and_purge_asset(
                    user_state,
                    user_address,
                    &[(receiver_address, output_asset)],
                    true,
                );
            }
        },
        SubCommand::Block { block_command } => match block_command {
            BlockCommand::Reset {} => {
                service.reset_server_state();
            }
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
                let pending_transactions = user_state.get_pending_transaction_hashes();

                for tx_hash in pending_transactions {
                    let tx_inclusion_proof =
                        service.get_transaction_inclusion_witness(user_address, tx_hash);
                    let block_hash = tx_inclusion_proof.root;
                    let received_signature =
                        service.sign_to_message(user_state.account, *block_hash);
                    service.send_received_signature(received_signature, tx_hash);
                    user_state.remove_pending_transactions(tx_hash);
                }
            }
            BlockCommand::Approve {} => {
                service.trigger_approve_block();
            }
        },
    }

    let encoded_wallet = serde_json::to_string(&wallet).unwrap();
    let mut file = File::create(wallet_file_path)?;
    write!(file, "{}", encoded_wallet)?;
    file.flush()?;

    let encoded_service = serde_json::to_string(&service).unwrap();
    let mut file = File::create(config_file_path)?;
    write!(file, "{}", encoded_service)?;
    file.flush()?;

    Ok(())
}
