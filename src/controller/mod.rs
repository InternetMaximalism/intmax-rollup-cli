use std::{
    fs::{create_dir, File},
    io::{Read, Write},
    str::FromStr,
};

use intmax_zkp_core::{
    rollup::gadgets::deposit_block::DepositInfo,
    sparse_merkle_tree::goldilocks_poseidon::{
        GoldilocksHashOut, LayeredLayeredPoseidonSparseMerkleTree, NodeDataMemory,
    },
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
    utils::key_management::{
        memory::WalletOnMemory,
        types::{Asset, TokenKind, Wallet},
    },
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
        #[structopt(long)]
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
    SetDefault {
        #[structopt(long)]
        user_address: Option<String>,
    },
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
        #[structopt(long)]
        variable_index: String,
        #[structopt(long)]
        amount: u64,
    },
    /// Merge your own assets.
    #[structopt(name = "merge")]
    Merge {
        #[structopt(long)]
        user_address: Option<String>,
    },
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

            println!("{}", serde_json::to_string(&user_state.assets).unwrap());
            println!("{:?}", user_state.assets);
        }
        SubCommand::Transaction { tx_command } => match tx_command {
            TransactionCommand::Merge { user_address } => {
                let user_address =
                    parse_address(&wallet, user_address).expect("user address was not given");
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                let old_user_asset_root = user_state.asset_tree.get_root();
                // dbg!(old_user_asset_root.to_string());

                let (blocks, latest_block_number) =
                    service.get_blocks(user_address, Some(user_state.last_seen_block_number), None);
                // dbg!(&blocks.len());

                let merge_witnesses = service.merge_deposits(blocks, user_address, user_state);
                let _new_user_asset_root = user_state.asset_tree.get_root();
                // dbg!(new_user_asset_root.to_string());

                let transaction = service.send_assets(
                    user_state.account,
                    &merge_witnesses,
                    &[],
                    &[],
                    old_user_asset_root,
                );

                user_state.insert_pending_transactions(&[transaction]);
                user_state.last_seen_block_number = latest_block_number;
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
                let output_asset = Asset {
                    kind: TokenKind {
                        contract_address: Address::from_str(&contract_address[2..]).unwrap(),
                        variable_index: GoldilocksHashOut::from_str(&variable_index[2..]).unwrap(),
                    },
                    amount,
                };

                let old_user_asset_root = user_state.asset_tree.get_root();
                // dbg!(&old_user_asset_root);

                let (blocks, latest_block_number) =
                    service.get_blocks(user_address, Some(user_state.last_seen_block_number), None);
                // dbg!(&blocks.len());

                let merge_witnesses = service.merge_deposits(blocks, user_address, user_state);
                let _new_user_asset_root = user_state.asset_tree.get_root();
                // dbg!(&new_user_asset_root);

                let input_assets = user_state.assets.filter(output_asset.kind);

                let mut input_amount = 0;
                for asset in input_assets.0.iter() {
                    input_amount += asset.1;
                }

                if output_asset.amount > input_amount {
                    panic!("output asset amount is too much");
                }

                let rest_asset = Asset {
                    kind: output_asset.kind,
                    amount: input_amount - output_asset.amount,
                };

                let mut tx_diff_tree: LayeredLayeredPoseidonSparseMerkleTree<NodeDataMemory> =
                    LayeredLayeredPoseidonSparseMerkleTree::new(
                        Default::default(),
                        Default::default(),
                    );

                let output_witness = tx_diff_tree
                    .set(
                        receiver_address.0.into(),
                        output_asset.kind.contract_address.0.into(),
                        output_asset.kind.variable_index,
                        HashOut::from_partial(&[F::from_canonical_u64(output_asset.amount)]).into(),
                    )
                    .unwrap();

                let rest_witness = tx_diff_tree
                    .set(
                        user_address.0.into(),
                        rest_asset.kind.contract_address.0.into(),
                        rest_asset.kind.variable_index,
                        HashOut::from_partial(&[F::from_canonical_u64(rest_asset.amount)]).into(),
                    )
                    .unwrap();

                let mut purge_input_witness = vec![];
                for input_asset in input_assets.0.iter() {
                    let input_witness = user_state
                        .asset_tree
                        .set(
                            input_asset.2, // tx_hash
                            input_asset.0.contract_address.0.into(),
                            input_asset.0.variable_index,
                            HashOut::from_partial(&[F::from_canonical_u64(input_asset.1)]).into(),
                        )
                        .unwrap();
                    purge_input_witness.push(input_witness);
                }

                let purge_output_witness = vec![output_witness, rest_witness];

                let transaction = service.send_assets(
                    user_state.account,
                    &merge_witnesses,
                    &purge_input_witness,
                    &purge_output_witness,
                    old_user_asset_root,
                );

                user_state.insert_pending_transactions(&[transaction]);
                user_state.assets.remove(output_asset.kind);
                user_state.last_seen_block_number = latest_block_number;
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
                let pending_transactions = wallet.get_pending_transaction_hashes(user_address);
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                for tx_hash in pending_transactions {
                    let tx_inclusion_proof =
                        service.get_transaction_inclusion_witness(user_address, tx_hash);
                    let block_hash = tx_inclusion_proof.root;
                    let received_signature =
                        service.sign_to_message(user_state.account, *block_hash);
                    service.send_received_signature(received_signature, tx_hash);
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

    Ok(())
}
