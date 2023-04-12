use std::{
    fs::{create_dir, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};

use intmax_interoperability_plugin::ethers::{
    prelude::k256::ecdsa::SigningKey,
    types::{H160, U256},
    utils::secret_key_to_address,
};
use intmax_rollup_interface::intmax_zkp_core::{
    plonky2::{
        field::{goldilocks_field::GoldilocksField, types::Field},
        plonk::config::{GenericConfig, GenericHashOut, PoseidonGoldilocksConfig},
    },
    rollup::gadgets::deposit_block::VariableIndex,
    sparse_merkle_tree::goldilocks_poseidon::WrappedHashOut,
    transaction::asset::{ContributedAsset, TokenKind},
    zkdsa::account::{Account, Address},
};
use num_bigint::BigUint;
use structopt::StructOpt;

use crate::{
    service::{
        builder::*,
        ethereum::gwei_to_wei,
        functions::{bulk_mint, merge, parse_address, transfer},
        interoperability::{
            activate_offer, get_network_config, get_offer, lock_offer, register_transfer,
            unlock_offer, MakerTransferInfo, NetworkName, TakerTransferInfo,
        },
        read_distribution_from_csv,
    },
    utils::{
        key_management::{memory::WalletOnMemory, types::Wallet},
        nickname::NicknameTable,
    },
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
    /// configuration commands
    #[structopt(name = "config")]
    Config {
        #[structopt(subcommand)]
        config_command: ConfigCommand,
    },
    /// commands for accounts on intmax
    #[structopt(name = "account")]
    Account {
        #[structopt(subcommand)]
        account_command: AccountCommand,
    },
    /// commands for transactions on intmax
    #[structopt(name = "tx")]
    Transaction {
        #[structopt(subcommand)]
        tx_command: TransactionCommand,
    },
    /// commands for blocks on intmax
    #[structopt(name = "block")]
    Block {
        #[structopt(subcommand)]
        block_command: BlockCommand,
    },
    /// commands for interoperability
    #[cfg(feature = "interoperability")]
    #[structopt(name = "io")]
    Interoperability {
        #[structopt(subcommand)]
        io_command: InteroperabilityCommand,
    },
    /// commands for bridge
    #[cfg(feature = "bridge")]
    #[structopt(name = "bridge")]
    Bridge {
        #[structopt(subcommand)]
        bridge_command: BridgeCommand,
    },
}

#[derive(Debug, StructOpt)]
enum ConfigCommand {
    /// Set aggregator to the specified URL. If omitted, the currently set URL are displayed.
    #[structopt(name = "aggregator-url")]
    AggregatorUrl {
        /// aggregator URL
        aggregator_url: Option<String>,
    },
}

#[derive(Debug, StructOpt)]
enum AccountCommand {
    /// [danger operation] Initializing your wallet and delete your all accounts and nicknames.
    #[structopt(name = "reset")]
    Reset {
        #[structopt(short = "y", long = "yes")]
        assume_yes: bool,
    },
    /// Add your account.
    #[structopt(name = "add")]
    Add {
        /// Specify private key. If not specified, it is chosen at random.
        #[structopt(long)]
        private_key: Option<WrappedHashOut<F>>,

        /// Add nickname
        #[structopt(long)]
        nickname: Option<String>,

        /// Set as default account.
        #[structopt(long = "default")]
        is_default: bool,
    },
    /// List your addresses.
    #[structopt(name = "list")]
    List {},
    /// Sets the default user account used when --user-address attribute is omitted in other commands.
    #[structopt(name = "set-default")]
    SetDefault {
        /// default user address
        user_address: Option<String>,
    },
    /// Display your assets.
    #[structopt(name = "assets")]
    Assets {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
    },
    /// commands for account nicknames.
    #[structopt(name = "nickname")]
    Nickname {
        #[structopt(subcommand)]
        nickname_command: NicknameCommand,
    },
    /// [upcoming features] Output the possession proof of your assets.
    #[structopt(name = "possession-proof")]
    PossessionProof {},
}

#[derive(Debug, StructOpt)]
enum NicknameCommand {
    /// Give your account a nickname.
    #[structopt(name = "set")]
    Set { address: String, nickname: String },
    /// Remove specified nicknames. The assets held in the account are not lost.
    #[structopt(name = "remove")]
    Remove { nicknames: Vec<String> },
    /// Display nicknames.
    #[structopt(name = "list")]
    List {},
}

#[derive(Debug, StructOpt)]
enum TransactionCommand {
    /// Mint your token with the same token address as your user address.
    #[structopt(name = "mint")]
    Mint {
        #[structopt(long, short = "u")]
        user_address: Option<String>,

        /// `token-id` can be selected from 0x00 to 0xff. [default: 0x00]
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,

        /// `amount` must be a positive integer less than 2^56.
        #[structopt(long, short = "q")]
        amount: Option<u64>,

        /// Mint NFT (an alias of `--amount 1`).
        #[structopt(long = "nft")]
        is_nft: bool,
    },
    /// Send your owned token to others.
    #[structopt(name = "send")]
    Send {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
        /// destination of a token
        #[structopt(long, short = "r")]
        receiver_address: String,
        /// token address
        #[structopt(long = "token-address", short = "a")]
        contract_address: Option<String>,
        /// the token id can be selected from 0x00 to 0xff
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,
        /// amount must be a positive integer less than 2^56
        #[structopt(long, short = "q")]
        amount: Option<u64>,
        /// send NFT (an alias of `--amount 1`)
        #[structopt(long = "nft")]
        is_nft: bool,
    },
    /// [advanced command] Merge received your token.
    /// This is usually performed automatically before you send the transaction.
    /// Tokens sent by others cannot be moved until this operation is performed.
    #[structopt(name = "merge")]
    Merge {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
    },
    /// You can issue new token according to the contents of the file.
    /// Up to 16 tokens can be sent together.
    ///
    /// For more information, see https://github.com/InternetMaximalism/intmax-rollup-cli/blob/main/tests/airdrop/README.md .
    #[structopt(name = "bulk-mint")]
    BulkMint {
        #[structopt(long, short = "u")]
        user_address: Option<String>,

        /// CSV file path
        #[structopt(long = "file", short = "f")]
        csv_path: PathBuf,
        // #[structopt(long)]
        // json: Vec<ContributedAsset<F>>,
    },
    /// You can transfer owned tokens according to the contents of the file.
    /// Up to 8 tokens can be sent together.
    ///
    /// For more information, see https://github.com/InternetMaximalism/intmax-rollup-cli/blob/main/tests/airdrop/README.md .
    #[structopt(name = "bulk-transfer")]
    BulkTransfer {
        #[structopt(long, short = "u")]
        user_address: Option<String>,

        /// CSV file path
        #[structopt(long = "file", short = "f")]
        csv_path: PathBuf,
        // #[structopt(long)]
        // json: Vec<ContributedAsset<F>>,
    },
    /// [upcoming features] Exchange tokens with a specified user.
    #[structopt(name = "swap")]
    Swap {},
}

#[derive(Debug, StructOpt)]
enum BlockCommand {
    // /// Trigger to propose a block.
    // #[structopt(name = "propose")]
    // Propose {},
    /// [advanced command] Sign to the proposal block.
    /// It is usually performed automatically after the transaction has been executed.
    /// If you do not sign the proposal block containing your transaction by the deadline,
    /// the transaction is reverted.
    #[structopt(name = "sign")]
    Sign {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
    },
    // /// Trigger to approve a block.
    // #[structopt(name = "approve")]
    // Approve {},
    /// [advanced command] Verify a approved block.
    #[structopt(name = "verify")]
    Verify {
        #[structopt(long, short = "n")]
        block_number: Option<u32>,
    },
}

#[cfg(feature = "interoperability")]
#[derive(Debug, StructOpt)]
enum InteroperabilityCommand {
    #[structopt(name = "register")]
    Register {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
        /// destination of a token
        #[structopt(long, short = "r")]
        receiver_address: String,
        /// token address
        #[structopt(long = "token-address", short = "a")]
        contract_address: Option<String>,
        /// the token id can be selected from 0x00 to 0xff
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,
        /// maker amount must be a positive integer less than 2^56
        #[structopt(long)]
        maker_amount: Option<u64>,
        /// taker amount must be a positive integer less than 2^256 (example: --taker-amount 100)
        #[structopt(long)]
        taker_amount: String,
        /// send NFT (an alias of `--amount 1`)
        #[structopt(long = "nft")]
        is_nft: bool,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
        /// Upper limit of acceptable gas price in Gwei
        #[structopt(long)]
        max_gas_price: Option<f64>,
    },
    #[structopt(name = "activate")]
    Activate {
        // #[structopt(long, short = "u")]
        // user_address: Option<String>,
        #[structopt()]
        offer_id: usize,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
    },
    #[structopt(name = "lock")]
    Lock {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
        /// destination of a token
        #[structopt(long, short = "r")]
        receiver_address: String,
        /// token address [Default: receiver_address]
        #[structopt(long = "token-address", short = "a")]
        contract_address: Option<String>,
        /// the token id can be selected from 0x00 to 0xff
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,
        /// the receiver address on another chain
        #[structopt(long)]
        receiver: String,
        /// maker amount must be a positive integer less than 2^56
        #[structopt(long)]
        maker_amount: Option<u64>,
        /// taker amount must be a positive integer less than 2^256 (example: --taker-amount 100)
        #[structopt(long)]
        taker_amount: String,
        /// send NFT (an alias of `--amount 1`)
        #[structopt(long = "nft")]
        is_nft: bool,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
    },
    #[structopt(name = "unlock")]
    Unlock {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
        #[structopt()]
        offer_id: usize,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
    },
    #[structopt(name = "view")]
    View {
        // #[structopt(long, short = "u")]
        // user_address: Option<String>,
        #[structopt()]
        offer_id: usize,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
        #[structopt(long = "reverse-offer", short = "r")]
        is_reverse_offer: bool,
    },
}

#[cfg(feature = "bridge")]
#[derive(Debug, StructOpt)]
enum BridgeCommand {
    /// [upcoming features] Mint your token with the same token address as your user address.
    #[structopt(name = "deposit")]
    Deposit {
        #[structopt(long, short = "u")]
        user_address: Option<String>,

        /// `token-id` can be selected from 0x00 to 0xff. [default: 0x00]
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,

        /// `amount` must be a positive integer less than 2^56.
        #[structopt(long, short = "q")]
        amount: Option<u64>,

        /// Mint NFT (an alias of `--amount 1`).
        #[structopt(long = "nft")]
        is_nft: bool,

        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
    },
    /// Send your owned token to others.
    #[structopt(name = "exit")]
    Burn {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
        /// token address
        #[structopt(long = "token-address", short = "a")]
        contract_address: Option<String>,
        /// the token id can be selected from 0x00 to 0xff
        #[structopt(long = "token-id", short = "i")]
        token_id: Option<VariableIndex<F>>,
        /// amount must be a positive integer less than 2^56
        #[structopt(long, short = "q")]
        amount: Option<u64>,
        /// send NFT (an alias of `--amount 1`)
        #[structopt(long = "nft")]
        is_nft: bool,
        /// choose "scroll" (Scroll Alpha)
        #[structopt(long = "network", short = "n")]
        network_name: String,
    },
}

pub fn get_input(prompt: &str) -> String {
    println!("{}", prompt);
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {}
        Err(_no_updates_is_fine) => {}
    }
    input.trim().to_string()
}

pub async fn invoke_command() -> anyhow::Result<()> {
    let mut intmax_dir = dirs::home_dir().expect("fail to get home directory");
    intmax_dir.push(".intmax");

    if File::open(intmax_dir.clone()).is_err() {
        create_dir(intmax_dir.clone()).unwrap();
        println!("make directory: {}", intmax_dir.to_string_lossy());
    }

    let mut config_file_path = intmax_dir.clone();
    config_file_path.push("config");

    let mut service = if let Ok(mut file) = File::open(config_file_path.clone()) {
        let mut encoded_service = String::new();
        file.read_to_string(&mut encoded_service)?;
        serde_json::from_str(&encoded_service).unwrap()
    } else {
        ServiceBuilder::new(DEFAULT_AGGREGATOR_URL)
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

    let mut nickname_file_path = wallet_dir_path.clone();
    nickname_file_path.push("nickname");

    let mut nickname_table = if let Ok(mut file) = File::open(nickname_file_path.clone()) {
        let mut encoded_nickname_table = String::new();
        file.read_to_string(&mut encoded_nickname_table)?;
        serde_json::from_str(&encoded_nickname_table).unwrap()
    } else {
        NicknameTable::default()
    };

    let mut wallet_file_path = wallet_dir_path.clone();
    wallet_file_path.push("wallet");

    let Cli { sub_command } = Cli::from_args();

    let password = "password"; // unused
    if let SubCommand::Account {
        account_command: AccountCommand::Reset { assume_yes },
    } = sub_command
    {
        if !assume_yes {
            let response = get_input(
                "This operation cannot be undone. Do you really want to reset the wallet? [y/N]",
            );
            if response.to_lowercase() != "y" {
                println!("Wallet was not reset");

                return Ok(());
            }
        }

        let wallet = WalletOnMemory::new(wallet_file_path, password.to_string());

        wallet.backup()?;

        let nickname_table = NicknameTable::default();
        let encoded_nickname_table = serde_json::to_string(&nickname_table).unwrap();
        std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
        let mut file = File::create(nickname_file_path.clone())?;
        write!(file, "{}", encoded_nickname_table)?;
        file.flush()?;

        println!("Wallet initialized");

        return Ok(());
    }

    let mut wallet = {
        let result = WalletOnMemory::read_from_file(wallet_file_path.clone());
        if let Ok(wallet) = result {
            wallet
        } else {
            let wallet = WalletOnMemory::new(wallet_file_path, password.to_string());

            wallet.backup()?;

            println!("Wallet initialized");

            wallet
        }
    };

    if let SubCommand::Config { config_command: _ } = sub_command {
        // nothing to do
    } else {
        check_compatibility_with_server(&service).await?;
    }

    let set_nickname = |nickname_table: &mut NicknameTable,
                        address: Address<F>,
                        nickname: String|
     -> anyhow::Result<()> {
        if nickname.starts_with("0x") {
            anyhow::bail!("nickname must not start with 0x");
        }

        if nickname.len() > 12 {
            anyhow::bail!("choose a nickname that is less than or equal to 12 characters");
        }

        nickname_table.insert(address, nickname)?;

        let encoded_nickname_table = serde_json::to_string(&nickname_table).unwrap();
        std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
        let mut file = File::create(nickname_file_path.clone())?;
        write!(file, "{}", encoded_nickname_table)?;
        file.flush()?;

        Ok(())
    };

    match sub_command {
        SubCommand::Config { config_command } => match config_command {
            ConfigCommand::AggregatorUrl { aggregator_url } => {
                service.set_aggregator_url(aggregator_url).await?;

                let encoded_service = serde_json::to_string(&service).unwrap();
                let mut file = File::create(config_file_path)?;
                write!(file, "{}", encoded_service)?;
                file.flush()?;
            }
        },
        SubCommand::Account { account_command } => match account_command {
            AccountCommand::Reset { .. } => {}
            AccountCommand::Add {
                private_key,
                nickname,
                is_default,
            } => {
                let private_key = private_key
                    // .map(|v| WrappedHashOut::from_str(&v).expect("fail to parse user address"))
                    .unwrap_or_else(WrappedHashOut::rand);
                let account = Account::new(*private_key);
                service.register_account(account.public_key).await;
                wallet.add_account(account)?;

                println!("new account added: {}", account.address);

                if is_default {
                    wallet.set_default_account(Some(account.address));
                    println!("set the above account as default");
                }

                wallet.backup()?;

                if let Some(nickname) = nickname {
                    set_nickname(&mut nickname_table, account.address, nickname.clone())?;
                    println!("the above account appears replaced by {nickname}");
                }

                service.trigger_propose_block().await;
                service.trigger_approve_block().await;
            }
            AccountCommand::List {} => {
                let mut account_list = wallet.data.keys().collect::<Vec<_>>();
                account_list.sort_by_key(|v| v.to_string());

                let mut is_empty = true;
                for address in account_list {
                    is_empty = false;

                    if Some(*address) == wallet.get_default_account() {
                        if let Some(nickname) = nickname_table.address_to_nickname.get(address) {
                            println!("{address} [{nickname}] (default)",);
                        } else {
                            println!("{address} (default)");
                        }
                    } else if let Some(nickname) = nickname_table.address_to_nickname.get(address) {
                        println!("{address} [{nickname}]",);
                    } else {
                        println!("{address}");
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
                if let Some(user_address) = user_address {
                    let user_address = if user_address.is_empty() {
                        anyhow::bail!("empty user address");
                    } else if user_address.starts_with("0x") {
                        Address::from_str(&user_address)?
                    } else if let Some(user_address) =
                        nickname_table.nickname_to_address.get(&user_address)
                    {
                        *user_address
                    } else {
                        anyhow::bail!("unregistered nickname");
                    };

                    if account_list.iter().any(|v| v == &user_address) {
                        wallet.set_default_account(Some(user_address));
                        println!("set default account: {}", user_address);
                    } else {
                        anyhow::bail!("given account does not exist in your wallet");
                    }
                } else {
                    wallet.set_default_account(None);
                    println!("set default account: null");
                }

                wallet.backup()?;
            }

            AccountCommand::Assets { user_address } => {
                let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                {
                    let user_state = wallet
                        .data
                        .get_mut(&user_address)
                        .expect("user address was not found in wallet");

                    service
                        .sync_sent_transaction(user_state, user_address)
                        .await;

                    wallet.backup()?;
                }

                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                // NOTICE: Changes to `user_state` here are not saved to file.
                calc_merge_witnesses(user_state, user_state.rest_received_assets.clone()).await;

                let total_amount_map = user_state.assets.calc_total_amount();

                let separator = "--------------------------------------------------------------------------------------";
                {
                    if let Some(user_nickname) =
                        nickname_table.address_to_nickname.get(&user_address)
                    {
                        println!("User: {} ({})", user_nickname, user_address);
                    } else {
                        println!("User: {}", user_address);
                    }
                }
                println!("{}", separator);
                if total_amount_map.is_empty() {
                    println!("  No assets held");
                    println!("{}", separator);
                } else {
                    for ((contract_address, variable_index), total_amount) in total_amount_map {
                        let decoded_contract_address =
                            Address::from_str(&contract_address).unwrap();
                        if let Some(contract_nickname) = nickname_table
                            .address_to_nickname
                            .get(&decoded_contract_address)
                        {
                            println!(
                                "  Token Address | {} [{}]",
                                decoded_contract_address, contract_nickname
                            );
                        } else {
                            println!("  Token Address | {}", decoded_contract_address);
                        }
                        println!("  Token ID      | {}", variable_index);
                        println!("  Amount        | {}", total_amount);
                        println!("{}", separator);
                    }
                }

                #[cfg(feature = "verbose")]
                println!(
                    "raw data: {}",
                    serde_json::to_string(&user_state.assets).unwrap()
                );
            }
            AccountCommand::Nickname { nickname_command } => match nickname_command {
                NicknameCommand::Set { address, nickname } => {
                    if address.len() != 18 {
                        anyhow::bail!("address must be 8 bytes hex string with 0x-prefix");
                    }
                    let address = Address::from_str(&address)?;

                    set_nickname(&mut nickname_table, address, nickname)?;

                    println!("Done!");
                }
                NicknameCommand::Remove { nicknames } => {
                    for nickname in nicknames {
                        nickname_table.remove(nickname)?;
                    }

                    let encoded_nickname_table = serde_json::to_string(&nickname_table).unwrap();
                    std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
                    let mut file = File::create(nickname_file_path)?;
                    write!(file, "{}", encoded_nickname_table)?;
                    file.flush()?;

                    println!("Done!");
                }
                NicknameCommand::List {} => {
                    for (nickname, address) in nickname_table.nickname_to_address {
                        println!("{nickname} = {address}");
                    }
                }
            },
            AccountCommand::PossessionProof { .. } => {
                anyhow::bail!("This is a upcoming feature.");
            }
        },
        SubCommand::Transaction { tx_command } => {
            match tx_command {
                TransactionCommand::Mint {
                    user_address,
                    token_id: variable_index,
                    amount,
                    is_nft,
                } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                    let _user_state = wallet
                        .data
                        .get(&user_address)
                        .expect("user address was not found in wallet");

                    // Only tokens with the same contract_address as receiver_address can be minted.
                    let contract_address = user_address; // serde_json::from_str(&contract_address).unwrap()
                    let variable_index = if let Some(variable_index) = variable_index {
                        if is_nft && variable_index == 0u8.into() {
                            anyhow::bail!(
                                "it is recommended that the NFT token ID be something other than 0x00"
                            );
                        }

                        variable_index
                    } else {
                        if is_nft {
                            anyhow::bail!("you cannot omit --token-id attribute with --nft flag");
                        }

                        0u8.into()
                    };
                    let amount = if let Some(amount) = amount {
                        if is_nft {
                            println!("--nft flag was ignored because of --amount attribute");
                        }

                        amount
                    } else if is_nft {
                        1
                    } else {
                        anyhow::bail!("you cannot omit --amount attribute without --nft flag");
                    };

                    // let variable_index = VariableIndex::from_str(&variable_index).unwrap();
                    let deposit_info = ContributedAsset {
                        receiver_address: user_address,
                        kind: TokenKind {
                            contract_address,
                            variable_index,
                        },
                        amount,
                    };
                    service
                        .deposit_assets(user_address, vec![deposit_info])
                        .await?;

                    service.trigger_propose_block().await;
                    service.trigger_approve_block().await;
                }
                TransactionCommand::Merge { user_address } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;

                    {
                        let user_state = wallet
                            .data
                            .get_mut(&user_address)
                            .expect("user address was not found in wallet");

                        service
                            .sync_sent_transaction(user_state, user_address)
                            .await;

                        wallet.backup()?;
                    }

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    merge(&service, &mut wallet, user_address, 0).await?;
                }
                TransactionCommand::Send {
                    user_address,
                    receiver_address,
                    contract_address,
                    token_id: variable_index,
                    amount,
                    is_nft,
                } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;

                    let receiver_address = if receiver_address.is_empty() {
                        anyhow::bail!("empty recipient");
                    } else if receiver_address.starts_with("0x") {
                        if receiver_address.len() != 18 {
                            anyhow::bail!("recipient must be 8 bytes hex string with 0x-prefix");
                        }

                        Address::from_str(&receiver_address)?
                    } else if let Some(receiver_address) =
                        nickname_table.nickname_to_address.get(&receiver_address)
                    {
                        *receiver_address
                    } else {
                        anyhow::bail!("unregistered nickname: recipient");
                    };

                    if user_address == receiver_address {
                        anyhow::bail!("cannot send asset to myself");
                    }

                    let contract_address = if let Some(contract_address) = contract_address {
                        if contract_address.is_empty() {
                            anyhow::bail!("empty token address");
                        } else if contract_address.starts_with("0x") {
                            Address::from_str(&contract_address)?
                        } else if let Some(contract_address) =
                            nickname_table.nickname_to_address.get(&contract_address)
                        {
                            *contract_address
                        } else {
                            anyhow::bail!("unregistered nickname: token address");
                        }
                    } else {
                        user_address
                    };

                    if user_address == receiver_address {
                        anyhow::bail!("cannot send asset to myself");
                    }

                    let variable_index = if let Some(variable_index) = variable_index {
                        if is_nft && variable_index == 0u8.into() {
                            anyhow::bail!("it is recommended that the NFT token ID be something other than 0x00");
                        }

                        variable_index
                    } else {
                        if is_nft {
                            anyhow::bail!("you cannot omit --token-id attribute with --nft flag");
                        }

                        0u8.into()
                    };
                    let amount = if let Some(amount) = amount {
                        if is_nft {
                            println!("--nft flag was ignored because of --amount attribute");
                        }

                        amount
                    } else if is_nft {
                        1
                    } else {
                        anyhow::bail!("you cannot omit --amount attribute without --nft flag");
                    };

                    if amount == 0 || amount >= 1u64 << 56 {
                        anyhow::bail!("`amount` must be a positive integer less than 2^56");
                    }

                    // let variable_index = VariableIndex::from_str(&variable_index).unwrap();
                    let output_asset = ContributedAsset {
                        receiver_address,
                        kind: TokenKind {
                            contract_address,
                            variable_index,
                        },
                        amount,
                    };
                    #[cfg(feature = "verbose")]
                    dbg!(serde_json::to_string(&output_asset).unwrap());

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    transfer(&service, &mut wallet, user_address, &[output_asset]).await?;
                }
                TransactionCommand::BulkMint {
                    user_address,
                    csv_path,
                    // json
                } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;

                    let file =
                        File::open(csv_path).map_err(|_| anyhow::anyhow!("file was not found"))?;
                    let json = read_distribution_from_csv(user_address, file)?;

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    bulk_mint(&service, &mut wallet, user_address, json, true).await?;
                }
                TransactionCommand::BulkTransfer {
                    user_address,
                    csv_path,
                    // json
                } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;

                    let file =
                        File::open(csv_path).map_err(|_| anyhow::anyhow!("file was not found"))?;
                    let json = read_distribution_from_csv(user_address, file)?;

                    bulk_mint(&service, &mut wallet, user_address, json, false).await?;
                }
                TransactionCommand::Swap { .. } => {
                    anyhow::bail!("This is a upcoming feature.");
                }
            }
        }
        SubCommand::Block { block_command } => match block_command {
            // BlockCommand::Propose {} => {
            //     service.trigger_propose_block();
            // }
            BlockCommand::Sign { user_address } => {
                println!("block sign");
                let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                service.sign_proposed_block(user_state, user_address).await;

                wallet.backup()?;
            }
            // BlockCommand::Approve {} => {
            //     service.trigger_approve_block();
            // }
            BlockCommand::Verify { block_number } => {
                service.verify_block(block_number).await.unwrap();
            }
        },
        #[cfg(feature = "interoperability")]
        SubCommand::Interoperability { io_command } => match io_command {
            InteroperabilityCommand::Register {
                user_address,
                receiver_address,
                contract_address,
                token_id: variable_index,
                maker_amount,
                taker_amount,
                is_nft,
                network_name,
                max_gas_price,
            } => {
                let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                {
                    let user_state = wallet
                        .data
                        .get_mut(&user_address)
                        .expect("user address was not found in wallet");

                    service
                        .sync_sent_transaction(user_state, user_address)
                        .await;

                    wallet.backup()?;
                }

                {
                    let network_name: NetworkName = network_name.parse()?;
                    if network_name == NetworkName::PolygonZkEvmTest {
                        anyhow::bail!("Polygon ZKEVM testnet cannot be selected now");
                    }
                }

                let network_config = get_network_config(network_name.parse()?);
                let secret_key =
                    std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

                let receiver_address = if receiver_address.is_empty() {
                    anyhow::bail!("empty recipient");
                } else if receiver_address.starts_with("0x") {
                    if receiver_address.len() != 18 {
                        anyhow::bail!("recipient must be 8 bytes hex string with 0x-prefix");
                    }

                    Address::from_str(&receiver_address)?
                } else if let Some(receiver_address) =
                    nickname_table.nickname_to_address.get(&receiver_address)
                {
                    *receiver_address
                } else {
                    anyhow::bail!("unregistered nickname: recipient");
                };

                let contract_address = if let Some(contract_address) = contract_address {
                    if contract_address.is_empty() {
                        anyhow::bail!("empty token address");
                    } else if contract_address.starts_with("0x") {
                        Address::from_str(&contract_address)?
                    } else if let Some(contract_address) =
                        nickname_table.nickname_to_address.get(&contract_address)
                    {
                        *contract_address
                    } else {
                        anyhow::bail!("unregistered nickname: token address");
                    }
                } else {
                    user_address
                };

                let variable_index = if let Some(variable_index) = variable_index {
                    if is_nft && variable_index == 0u8.into() {
                        anyhow::bail!(
                            "it is recommended that the NFT token ID be something other than 0x00"
                        );
                    }

                    variable_index
                } else {
                    if is_nft {
                        anyhow::bail!("you cannot omit --token-id attribute with --nft flag");
                    }

                    0u8.into()
                };

                let maker_amount = if let Some(maker_amount) = maker_amount {
                    if is_nft {
                        println!("--nft flag was ignored because of --amount attribute");
                    }

                    maker_amount
                } else if is_nft {
                    1
                } else {
                    anyhow::bail!("you cannot omit --amount attribute without --nft flag");
                };

                ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                merge(&service, &mut wallet, user_address, 0).await?;

                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");
                let total_amount_map = user_state.assets.calc_total_amount();

                let balance = total_amount_map
                    .get(&(contract_address.to_string(), variable_index.to_string()))
                    .cloned()
                    .unwrap_or_default();
                anyhow::ensure!(
                    BigUint::from(maker_amount).le(&balance),
                    "transfer amount is too much"
                );

                let signer_key =
                    SigningKey::from_bytes(&hex::decode(&secret_key).unwrap()).unwrap();
                let my_account = secret_key_to_address(&signer_key);
                let sending_transfer_info = MakerTransferInfo {
                    address: my_account,
                    intmax_account: user_address,
                    kind: TokenKind {
                        contract_address,
                        variable_index,
                    },
                    amount: maker_amount,
                };
                let taker_amount = U256::from_little_endian(
                    &BigUint::from_str(&taker_amount).unwrap().to_bytes_le(),
                );
                let receiving_transfer_info = TakerTransferInfo {
                    address: H160::default(), // anyone can activate
                    intmax_account: receiver_address,
                    token_address: H160::default(),
                    amount: taker_amount,
                };

                let offer_id = register_transfer(
                    &network_config,
                    secret_key,
                    sending_transfer_info,
                    receiving_transfer_info,
                    max_gas_price.map(gwei_to_wei),
                )
                .await?;
                println!("offer_id: {}", offer_id);

                let network_name = NetworkName::from_str(&network_name)
                    .map_err(|_| anyhow::anyhow!("invalid network name"))?;
                let receiver_address = match network_name {
                    NetworkName::ScrollAlpha => Address(F::from_canonical_u64(1)),
                    NetworkName::PolygonZkEvmTest => Address(F::from_canonical_u64(2)),
                };
                let output_asset = ContributedAsset {
                    receiver_address,
                    kind: TokenKind {
                        contract_address,
                        variable_index,
                    },
                    amount: maker_amount,
                };
                #[cfg(feature = "verbose")]
                dbg!(serde_json::to_string(&output_asset).unwrap());

                transfer(&service, &mut wallet, user_address, &[output_asset]).await?;

                wallet.backup()?;
            }
            InteroperabilityCommand::Activate {
                offer_id,
                network_name,
                ..
            } => {
                // let user_address = parse_address(&wallet, user_address)?;
                // let user_state = wallet
                //     .data
                //     .get_mut(&user_address)
                //     .expect("user address was not found in wallet");

                {
                    let network_name: NetworkName = network_name.parse()?;
                    if network_name == NetworkName::PolygonZkEvmTest {
                        anyhow::bail!("Polygon ZKEVM testnet cannot be selected now");
                    }
                }

                let network_config = get_network_config(network_name.parse()?);
                let secret_key =
                    std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

                let offer_id: U256 = offer_id.into();
                let is_activated = activate_offer(&network_config, secret_key, offer_id).await?;

                if !is_activated {
                    anyhow::bail!("The activation was succeeded, but it has not reflect yet. Please rerun `intmax io activate <offer-id>` after few minutes.");
                }

                // reflect to deposit tree
                service.trigger_propose_block().await;
                service.trigger_approve_block().await;
            }
            InteroperabilityCommand::Lock {
                user_address,
                receiver_address,
                contract_address,
                token_id: variable_index,
                receiver,
                maker_amount,
                taker_amount,
                is_nft,
                network_name,
            } => {
                let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                {
                    let user_state = wallet
                        .data
                        .get_mut(&user_address)
                        .expect("user address was not found in wallet");

                    service
                        .sync_sent_transaction(user_state, user_address)
                        .await;

                    wallet.backup()?;
                }

                {
                    let network_name: NetworkName = network_name.parse()?;
                    if network_name == NetworkName::PolygonZkEvmTest {
                        anyhow::bail!("Polygon ZKEVM testnet cannot be selected now");
                    }
                }

                let network_config = get_network_config(network_name.parse()?);
                let secret_key =
                    std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

                let receiver_address = if receiver_address.is_empty() {
                    anyhow::bail!("empty recipient");
                } else if receiver_address.starts_with("0x") {
                    if receiver_address.len() != 18 {
                        anyhow::bail!("recipient must be 8 bytes hex string with 0x-prefix");
                    }

                    Address::from_str(&receiver_address)?
                } else if let Some(receiver_address) =
                    nickname_table.nickname_to_address.get(&receiver_address)
                {
                    *receiver_address
                } else {
                    anyhow::bail!("unregistered nickname: recipient");
                };

                let contract_address = if let Some(contract_address) = contract_address {
                    if contract_address.is_empty() {
                        anyhow::bail!("empty token address");
                    } else if contract_address.starts_with("0x") {
                        Address::from_str(&contract_address)?
                    } else if let Some(contract_address) =
                        nickname_table.nickname_to_address.get(&contract_address)
                    {
                        *contract_address
                    } else {
                        anyhow::bail!("unregistered nickname: token address");
                    }
                } else {
                    if receiver_address == Address::default() {
                        anyhow::bail!("contract_address must be non-zero address");
                    }

                    receiver_address
                };

                let variable_index = if let Some(variable_index) = variable_index {
                    if is_nft && variable_index == 0u8.into() {
                        anyhow::bail!(
                            "it is recommended that the NFT token ID be something other than 0x00"
                        );
                    }

                    variable_index
                } else {
                    if is_nft {
                        anyhow::bail!("you cannot omit --token-id attribute with --nft flag");
                    }

                    0u8.into()
                };

                let receiver = if let Some(stripped_receiver) = receiver.strip_prefix("0x") {
                    stripped_receiver.to_string()
                } else {
                    receiver
                };
                let receiver = H160::from_str(&receiver).unwrap();

                let maker_amount = if let Some(maker_amount) = maker_amount {
                    if is_nft {
                        println!("--nft flag was ignored because of --amount attribute");
                    }

                    maker_amount
                } else if is_nft {
                    1
                } else {
                    anyhow::bail!("you cannot omit --amount attribute without --nft flag");
                };

                ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                let signer_key =
                    SigningKey::from_bytes(&hex::decode(&secret_key).unwrap()).unwrap();
                let my_account = secret_key_to_address(&signer_key);
                let taker_amount = U256::from_little_endian(
                    &BigUint::from_str(&taker_amount).unwrap().to_bytes_le(),
                );
                if receiver_address == user_address {
                    anyhow::bail!("recipient must differ from user");
                }

                let sending_transfer_info = TakerTransferInfo {
                    address: my_account,
                    intmax_account: user_address,
                    token_address: H160::default(),
                    amount: taker_amount,
                };
                let receiving_transfer_info = MakerTransferInfo {
                    address: receiver,
                    intmax_account: receiver_address,
                    kind: TokenKind {
                        contract_address,
                        variable_index,
                    },
                    amount: maker_amount,
                };

                let offer_id = lock_offer(
                    &network_config,
                    secret_key,
                    sending_transfer_info,
                    receiving_transfer_info,
                )
                .await;
                println!("offer_id: {}", offer_id);
            }
            InteroperabilityCommand::Unlock {
                user_address,
                offer_id,
                network_name,
            } => {
                let user_address = parse_address(&wallet, &nickname_table, user_address)?;
                {
                    let user_state = wallet
                        .data
                        .get_mut(&user_address)
                        .expect("user address was not found in wallet");

                    service
                        .sync_sent_transaction(user_state, user_address)
                        .await;

                    wallet.backup()?;
                }

                {
                    let network_name: NetworkName = network_name.parse()?;
                    if network_name == NetworkName::PolygonZkEvmTest {
                        anyhow::bail!("Polygon ZKEVM testnet cannot be selected now");
                    }
                }

                let network_config = get_network_config(network_name.parse()?);
                let secret_key =
                    std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

                let offer =
                    get_offer(&network_config, secret_key.clone(), offer_id.into(), true).await;

                if offer.is_none() {
                    anyhow::bail!("this offer is not registered");
                }

                let offer = offer.unwrap();
                if offer.activated {
                    println!("this offer is already unlocked");
                    return anyhow::Ok(());
                }

                let signer_key =
                    SigningKey::from_bytes(&hex::decode(secret_key.clone()).unwrap()).unwrap();
                let my_account = secret_key_to_address(&signer_key);
                if offer.maker != my_account {
                    dbg!(offer.maker, my_account);
                    anyhow::bail!("Only the maker can unlock this offer");
                }

                let maker_token_kind = {
                    let maker_asset_id: U256 = offer.maker_asset_id;
                    let mut maker_asset_id_bytes = [0u8; 32];
                    maker_asset_id.to_little_endian(&mut maker_asset_id_bytes);

                    TokenKind::<GoldilocksField>::from_bytes(&maker_asset_id_bytes)
                };
                let maker_amount = offer.maker_amount.as_u64();

                merge(&service, &mut wallet, user_address, 0).await?;

                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");
                let total_amount_map = user_state.assets.calc_total_amount();

                let balance = total_amount_map
                    .get(&(
                        maker_token_kind.contract_address.to_string(),
                        maker_token_kind.variable_index.to_string(),
                    ))
                    .cloned()
                    .unwrap_or_default();
                #[cfg(feature = "verbose")]
                dbg!(
                    maker_token_kind.contract_address.to_string(),
                    maker_token_kind.variable_index.to_string(),
                    &balance
                );
                anyhow::ensure!(
                    BigUint::from(maker_amount).le(&balance),
                    "transfer amount is too much"
                );

                let maker_address = {
                    let mut tmp = offer.maker_intmax;
                    tmp.reverse();

                    Address::<F>::from_hash_out(*WrappedHashOut::from_bytes(&tmp))
                };
                if maker_address != Address::default() {
                    assert_eq!(maker_address, user_address);
                }
                let taker_address = {
                    let mut tmp = offer.taker_intmax;
                    tmp.reverse();

                    Address::<F>::from_hash_out(*WrappedHashOut::from_bytes(&tmp))
                };
                let output_asset = ContributedAsset {
                    receiver_address: taker_address,
                    kind: maker_token_kind,
                    amount: maker_amount,
                };
                #[cfg(feature = "verbose")]
                dbg!(serde_json::to_string(&output_asset).unwrap());
                let tx_hash = transfer(&service, &mut wallet, user_address, &[output_asset])
                    .await?
                    .expect("no transaction was sent");

                let witness = {
                    // XXX
                    service
                        .get_transaction_confirmation_witness(tx_hash, taker_address)
                        .await?

                    // let eth_wallet = LocalWallet::new_with_signer(
                    //     signer_key,
                    //     my_account,
                    //     network_config.chain_id,
                    // );
                    // let signature = eth_wallet
                    //     .sign_message(Bytes::from(offer.taker_intmax))
                    //     .await?;
                    // signature
                    //     .verify(offer.taker_intmax, my_account)
                    //     .expect("fail to verify signature");
                    // signature.to_vec().into()
                };
                // dbg!(&witness);

                let offer_id: U256 = offer_id.into();
                let _is_unlocked =
                    unlock_offer(&network_config, secret_key, offer_id, witness).await?;

                // if !_is_unlocked {
                //     println!("WARNING: The activation was succeeded, but it has not reflect yet.");
                // }
            }
            InteroperabilityCommand::View {
                offer_id,
                network_name,
                is_reverse_offer,
            } => {
                let network_config = get_network_config(network_name.parse()?);
                let secret_key =
                    std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

                let offer = get_offer(
                    &network_config,
                    secret_key.clone(),
                    offer_id.into(),
                    is_reverse_offer,
                )
                .await;

                if let Some(offer) = offer {
                    let mut maker_asset_id = [0u8; 32];
                    offer.maker_asset_id.to_big_endian(&mut maker_asset_id);
                    let maker_token_kind = TokenKind::<F>::from_bytes(&maker_asset_id);
                    println!(
                        "Status       | {}",
                        if offer.activated {
                            "ACTIVATED"
                        } else {
                            "NOT ACTIVATED"
                        }
                    );
                    println!("Maker        |");
                    println!(
                        "  {:10} | 0x{}",
                        network_name,
                        hex::encode(offer.maker.to_fixed_bytes())
                    );
                    println!("  intmax     | 0x{}", hex::encode(offer.maker_intmax));
                    println!("  Asset      |");
                    println!("    Address  | {}", maker_token_kind.contract_address);
                    println!("    Token ID | {}", maker_token_kind.variable_index);
                    println!("  Amount     | {}", offer.maker_amount);
                    println!("Taker        |",);
                    println!(
                        "  {:10} | 0x{}",
                        network_name,
                        hex::encode(offer.taker.to_fixed_bytes())
                    );
                    println!("  intmax     | 0x{}", hex::encode(offer.taker_intmax));
                    println!("  Asset      |");
                    println!(
                        "    Address  | 0x{}",
                        hex::encode(offer.taker_token_address.to_fixed_bytes())
                    );
                    println!("  Amount     | {}", offer.taker_amount);
                } else {
                    println!("Status       | NOT REGISTERED");
                }
            }
        },
        #[cfg(feature = "bridge")]
        SubCommand::Bridge { bridge_command } => {
            match bridge_command {
                BridgeCommand::Deposit { .. } => {
                    anyhow::bail!("This is a upcoming feature.");
                }
                BridgeCommand::Burn {
                    user_address,
                    contract_address,
                    token_id: variable_index,
                    amount,
                    is_nft,
                    network_name,
                } => {
                    let user_address = parse_address(&wallet, &nickname_table, user_address)?;

                    let network_name = NetworkName::from_str(&network_name)
                        .map_err(|_| anyhow::anyhow!("invalid network name"))?;
                    let receiver_address = match network_name {
                        NetworkName::ScrollAlpha => Address(F::from_canonical_u64(1)),
                        NetworkName::PolygonZkEvmTest => Address(F::from_canonical_u64(2)),
                    };

                    if user_address == receiver_address {
                        anyhow::bail!("cannot send asset to myself");
                    }

                    let contract_address = if let Some(contract_address) = contract_address {
                        if contract_address.is_empty() {
                            anyhow::bail!("empty token address");
                        } else if contract_address.starts_with("0x") {
                            Address::from_str(&contract_address)?
                        } else if let Some(contract_address) =
                            nickname_table.nickname_to_address.get(&contract_address)
                        {
                            *contract_address
                        } else {
                            anyhow::bail!("unregistered nickname: token address");
                        }
                    } else {
                        user_address
                    };

                    let variable_index = if let Some(variable_index) = variable_index {
                        if is_nft && variable_index == 0u8.into() {
                            anyhow::bail!("it is recommended that the NFT token ID be something other than 0x00");
                        }

                        variable_index
                    } else {
                        if is_nft {
                            anyhow::bail!("you cannot omit --token-id attribute with --nft flag");
                        }

                        0u8.into()
                    };
                    let amount = if let Some(amount) = amount {
                        if is_nft {
                            println!("--nft flag was ignored because of --amount attribute");
                        }

                        amount
                    } else if is_nft {
                        1
                    } else {
                        anyhow::bail!("you cannot omit --amount attribute without --nft flag");
                    };

                    if amount == 0 || amount >= 1u64 << 56 {
                        anyhow::bail!("`amount` must be a positive integer less than 2^56");
                    }

                    // let variable_index = VariableIndex::from_str(&variable_index).unwrap();
                    let output_asset = ContributedAsset {
                        receiver_address,
                        kind: TokenKind {
                            contract_address,
                            variable_index,
                        },
                        amount,
                    };
                    #[cfg(feature = "verbose")]
                    dbg!(serde_json::to_string(&output_asset).unwrap());

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    transfer(&service, &mut wallet, user_address, &[output_asset]).await?;
                }
            }
        }
    }

    Ok(())
}
