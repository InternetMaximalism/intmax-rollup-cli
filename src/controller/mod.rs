use std::{
    collections::HashMap,
    fs::{create_dir, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};

use intmax_rollup_interface::{
    constants::*,
    intmax_zkp_core::{
        plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
        rollup::gadgets::deposit_block::VariableIndex,
        sparse_merkle_tree::goldilocks_poseidon::WrappedHashOut,
        transaction::asset::{ContributedAsset, TokenKind},
        zkdsa::account::{Account, Address},
    },
};
use structopt::StructOpt;

use crate::{
    service::*,
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
    /// commands for accounts
    #[structopt(name = "account")]
    Account {
        #[structopt(subcommand)]
        account_command: AccountCommand,
    },
    /// Mint your token with the same token address as your user address.
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
    },
    /// Display your assets.
    #[structopt(name = "assets")]
    Assets {
        #[structopt(long, short = "u")]
        user_address: Option<String>,
    },
    /// commands for transactions
    #[structopt(name = "tx")]
    Transaction {
        #[structopt(subcommand)]
        tx_command: TransactionCommand,
    },
    /// commands for blocks
    #[structopt(name = "block")]
    Block {
        #[structopt(subcommand)]
        block_command: BlockCommand,
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
    /// Initializing your wallet and delete your all accounts
    #[structopt(name = "reset")]
    Reset {},
    /// Add your account
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
    /// List your addresses
    #[structopt(name = "list")]
    List {},
    /// Sets the default user account used when --user-address attribute is omitted in other commands.
    #[structopt(name = "set-default")]
    SetDefault {
        /// default user address
        user_address: Option<String>,
    },
    /// [deprecated features]
    Export {},
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
        /// `amount` must be a positive integer less than 2^56.
        #[structopt(long, short = "q")]
        amount: Option<u64>,
        /// Send NFT (an alias of `--amount 1`)
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

pub fn get_input(prompt: &str) -> String {
    println!("{}", prompt);
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {}
        Err(_no_updates_is_fine) => {}
    }
    input.trim().to_string()
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

    let backup_wallet = |wallet: &WalletOnMemory| -> anyhow::Result<()> {
        let encoded_wallet = serde_json::to_string(&wallet).unwrap();
        std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
        let mut file = File::create(wallet_file_path.clone())?;
        write!(file, "{}", encoded_wallet)?;
        file.flush()?;

        Ok(())
    };

    let Cli { sub_command } = Cli::from_args();

    let password = "password"; // unused
    if let SubCommand::Account {
        account_command: AccountCommand::Reset {},
    } = sub_command
    {
        // 本当に実行しますか？
        let response = get_input(
            "This operation cannot be undone. Do you really want to reset the wallet? [y/N]",
        );
        if response.to_lowercase() != "y" {
            println!("Wallet was not reset");

            return Ok(());
        }

        let wallet = WalletOnMemory::new(password.to_string());

        backup_wallet(&wallet)?;

        println!("Wallet initialized");

        return Ok(());
    }

    let mut wallet = {
        let result = File::open(wallet_file_path.clone()).and_then(|mut file| {
            let mut encoded_wallet = String::new();
            file.read_to_string(&mut encoded_wallet)?;
            let wallet = serde_json::from_str(&encoded_wallet)?;

            Ok(wallet)
        });
        if let Ok(wallet) = result {
            wallet
        } else {
            let wallet = WalletOnMemory::new(password.to_string());

            backup_wallet(&wallet)?;

            println!("Wallet initialized");

            wallet
        }
    };

    if let SubCommand::Config { config_command: _ } = sub_command {
        // nothing to do
    } else {
        check_compatibility_with_server(&service)?;
    }

    let parse_address = |wallet: &WalletOnMemory,
                         //  nickname_table: &NicknameTable,
                         user_address: Option<String>|
     -> anyhow::Result<Address<F>> {
        if let Some(user_address) = user_address {
            let user_address = if user_address.is_empty() {
                anyhow::bail!("empty user address");
            } else if user_address.starts_with("0x") {
                Address::from_str(&user_address)?
            } else if let Some(user_address) = nickname_table.nickname_to_address.get(&user_address)
            {
                *user_address
            } else {
                anyhow::bail!("unregistered nickname");
            };

            Ok(user_address)
        } else if let Some(user_address) = wallet.get_default_account() {
            Ok(user_address)
        } else {
            anyhow::bail!("--user-address was not given");
        }
    };

    let set_nickname = |nickname_table: &mut NicknameTable,
                        address: Address<F>,
                        nickname: String|
     -> anyhow::Result<()> {
        if nickname.starts_with("0x") {
            anyhow::bail!("nickname must not start with 0x");
        }

        nickname_table.insert(address, nickname)?;

        let encoded_nickname_table = serde_json::to_string(&nickname_table).unwrap();
        std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
        let mut file = File::create(nickname_file_path.clone())?;
        write!(file, "{}", encoded_nickname_table)?;
        file.flush()?;

        Ok(())
    };

    // マージしていない差分が残り `num_unmerged` 個以下になるまでマージ処理を繰り返す.
    // 1 回のループで `N_MERGES` 個ずつ減っていく.
    let merge = |wallet: &mut WalletOnMemory,
                 user_address: Address<F>,
                 num_unmerged: usize|
     -> anyhow::Result<()> {
        loop {
            let user_state = wallet
                .data
                .get_mut(&user_address)
                .expect("user address was not found in wallet");

            if user_state.rest_received_assets.len() <= num_unmerged {
                #[cfg(feature = "verbose")]
                println!("the number of unmerged differences is sufficiently small");
                break;
            }

            // 前の行で break しなかった場合, `user_state.rest_received_assets.len()` は 0 でないので,
            // "nothing to do" のエラーはハンドルする必要がない.
            service.merge_and_purge_asset(user_state, user_address, &[], false)?;

            backup_wallet(wallet)?;

            service.trigger_propose_block();
            service.trigger_approve_block();
        }

        Ok(())
    };

    let transfer = |wallet: &mut WalletOnMemory,
                    user_address: Address<F>,
                    purge_diffs: &[ContributedAsset<F>]|
     -> anyhow::Result<()> {
        {
            let user_state = wallet
                .data
                .get_mut(&user_address)
                .expect("user address was not found in wallet");

            service.sync_sent_transaction(user_state, user_address);

            backup_wallet(wallet)?;
        }

        // マージしていない差分が残り `N_MERGES` 個になるまでマージを繰り返す.
        // 残った差分は purge と一緒のトランザクションに含める.
        merge(wallet, user_address, ROLLUP_CONSTANTS.n_merges)?;

        {
            let user_state = wallet
                .data
                .get_mut(&user_address)
                .expect("user address was not found in wallet");

            let result = service.merge_and_purge_asset(user_state, user_address, purge_diffs, true);
            match result {
                Ok(_) => {}
                Err(err) => {
                    if err.to_string() == "nothing to do" {
                        #[cfg(feature = "verbose")]
                        println!("nothing to do");
                    } else {
                        return Err(err);
                    }
                }
            }

            backup_wallet(wallet)?;
        }

        service.trigger_propose_block();

        {
            let user_state = wallet
                .data
                .get_mut(&user_address)
                .expect("user address was not found in wallet");

            service.sign_proposed_block(user_state, user_address);

            backup_wallet(wallet)?;
        }

        service.trigger_approve_block();

        Ok(())
    };

    let bulk_mint = |wallet: &mut WalletOnMemory,
                     user_address: Address<F>,
                     distribution_list: Vec<ContributedAsset<F>>,
                     need_deposit: bool|
     -> anyhow::Result<()> {
        // {
        //     let user_state = wallet
        //         .data
        //         .get_mut(&user_address)
        //         .expect("user address was not found in wallet");

        //     service.sync_sent_transaction(user_state, user_address);

        //     backup_wallet(wallet)?;
        // }

        // 宛先とトークンごとに整理する.
        let mut distribution_map: HashMap<(Address<F>, TokenKind<F>), u64> = HashMap::new();
        for asset in distribution_list.iter() {
            if let Some(v) = distribution_map.get_mut(&(asset.receiver_address, asset.kind)) {
                *v += asset.amount;
            } else {
                distribution_map.insert((asset.receiver_address, asset.kind), asset.amount);
            }
        }

        let distribution_list = distribution_map
            .iter()
            .map(|(k, v)| ContributedAsset {
                receiver_address: k.0,
                kind: k.1,
                amount: *v,
            })
            .collect::<Vec<_>>();

        if distribution_list.is_empty() {
            anyhow::bail!("asset list is empty");
        }

        if distribution_list.len() > ROLLUP_CONSTANTS.n_diffs.min(ROLLUP_CONSTANTS.n_merges) {
            anyhow::bail!("too many destinations and token kinds");
        }

        if need_deposit {
            let mut deposit_list = distribution_list.clone();
            for deposit_info in deposit_list.iter() {
                if deposit_info.kind.contract_address != user_address {
                    anyhow::bail!("The token address must be your user address. You can only issue new tokens linked to your user address.");
                }
            }

            // 他人に対してトークンを発行する場合でも, まずは自分に対して deposit する.
            deposit_list
                .iter_mut()
                .for_each(|v| v.receiver_address = user_address);

            service.deposit_assets(user_address, deposit_list)?;

            service.trigger_propose_block();
            service.trigger_approve_block();
        }

        let purge_diffs = distribution_list
            .into_iter()
            .filter(|v| v.receiver_address != user_address)
            .collect::<Vec<_>>();

        transfer(wallet, user_address, &purge_diffs)?;

        Ok(())
    };

    match sub_command {
        SubCommand::Config { config_command } => match config_command {
            ConfigCommand::AggregatorUrl { aggregator_url } => {
                service.set_aggregator_url(aggregator_url)?;

                let encoded_service = serde_json::to_string(&service).unwrap();
                let mut file = File::create(config_file_path)?;
                write!(file, "{}", encoded_service)?;
                file.flush()?;
            }
        },
        SubCommand::Account { account_command } => match account_command {
            AccountCommand::Reset {} => {}
            AccountCommand::Add {
                private_key,
                nickname,
                is_default,
            } => {
                let private_key = private_key
                    // .map(|v| WrappedHashOut::from_str(&v).expect("fail to parse user address"))
                    .unwrap_or_else(WrappedHashOut::rand);
                let account = Account::new(*private_key);
                service.register_account(account.public_key);
                wallet.add_account(account)?;

                println!("new account added: {}", account.address);

                if is_default {
                    wallet.set_default_account(Some(account.address));
                    println!("set the above account as default");
                }

                backup_wallet(&wallet)?;

                if let Some(nickname) = nickname {
                    set_nickname(&mut nickname_table, account.address, nickname.clone())?;
                    println!("the above account appears replaced by {nickname}");
                }

                service.trigger_propose_block();
                service.trigger_approve_block();
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

                backup_wallet(&wallet)?;
            }
            AccountCommand::Export { .. } => {
                anyhow::bail!("This is a deprecated feature.");
            }
            AccountCommand::Nickname { nickname_command } => match nickname_command {
                NicknameCommand::Set { address, nickname } => {
                    if address.len() != 66 {
                        anyhow::bail!("address must be 32 bytes hex string with 0x-prefix");
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
        SubCommand::Deposit {
            user_address,
            token_id: variable_index,
            amount,
            is_nft,
        } => {
            let user_address = parse_address(&wallet, user_address)?;
            let _user_state = wallet
                .data
                .get(&user_address)
                .expect("user address was not found in wallet");

            // receiver_address と同じ contract_address をもつトークンしか mint できない
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
            service.deposit_assets(user_address, vec![deposit_info])?;

            service.trigger_propose_block();
            service.trigger_approve_block();
        }
        SubCommand::Assets {
            user_address,
            // verbose,
        } => {
            let user_address = parse_address(&wallet, user_address)?;
            {
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                service.sync_sent_transaction(user_state, user_address);

                backup_wallet(&wallet)?;
            }

            let user_state = wallet
                .data
                .get_mut(&user_address)
                .expect("user address was not found in wallet");

            // NOTICE: ここでの `user_state` の変更はファイルに保存しない.
            calc_merge_witnesses(user_state, user_state.rest_received_assets.clone());

            let total_amount_map = user_state.assets.calc_total_amount();

            let separator = "--------------------------------------------------------------------------------------";
            println!("User: {}", user_address);
            println!("{}", separator);
            if total_amount_map.is_empty() {
                println!("  No assets held");
                println!("{}", separator);
            } else {
                for ((contract_address, variable_index), total_amount) in total_amount_map {
                    let decoded_contract_address = Address::from_str(&contract_address).unwrap();
                    let contract_address = nickname_table
                        .address_to_nickname
                        .get(&decoded_contract_address)
                        // .map(|nickname| format!("{nickname} ({contract_address})"))
                        .unwrap_or(&contract_address);
                    println!("  Token Address | {}", contract_address);
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
        SubCommand::Transaction { tx_command } => {
            match tx_command {
                TransactionCommand::Merge { user_address } => {
                    let user_address = parse_address(&wallet, user_address)?;

                    {
                        let user_state = wallet
                            .data
                            .get_mut(&user_address)
                            .expect("user address was not found in wallet");

                        service.sync_sent_transaction(user_state, user_address);

                        backup_wallet(&wallet)?;
                    }

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    merge(&mut wallet, user_address, 0)?;
                }
                TransactionCommand::Send {
                    user_address,
                    receiver_address,
                    contract_address,
                    token_id: variable_index,
                    amount,
                    is_nft,
                } => {
                    let user_address = parse_address(&wallet, user_address)?;

                    let receiver_address = if receiver_address.is_empty() {
                        anyhow::bail!("empty recipient");
                    } else if receiver_address.starts_with("0x") {
                        if receiver_address.len() != 66 {
                            anyhow::bail!("recipient must be 32 bytes hex string with 0x-prefix");
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

                    transfer(&mut wallet, user_address, &[output_asset])?;
                }
                TransactionCommand::BulkMint {
                    user_address,
                    csv_path,
                    // json
                } => {
                    let user_address = parse_address(&wallet, user_address)?;

                    let file =
                        File::open(csv_path).map_err(|_| anyhow::anyhow!("file was not found"))?;
                    let json = read_distribution_from_csv(user_address, file)?;

                    ctrlc::set_handler(|| {}).expect("Error setting Ctrl-C handler");

                    bulk_mint(&mut wallet, user_address, json, true)?;
                }
                TransactionCommand::BulkTransfer {
                    user_address,
                    csv_path,
                    // json
                } => {
                    let user_address = parse_address(&wallet, user_address)?;

                    let file =
                        File::open(csv_path).map_err(|_| anyhow::anyhow!("file was not found"))?;
                    let json = read_distribution_from_csv(user_address, file)?;

                    bulk_mint(&mut wallet, user_address, json, false)?;
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
                let user_address = parse_address(&wallet, user_address)?;
                let user_state = wallet
                    .data
                    .get_mut(&user_address)
                    .expect("user address was not found in wallet");

                service.sign_proposed_block(user_state, user_address);

                backup_wallet(&wallet)?;
            }
            // BlockCommand::Approve {} => {
            //     service.trigger_approve_block();
            // }
            BlockCommand::Verify { block_number } => {
                service.verify_block(block_number).unwrap();
            }
        },
    }

    Ok(())
}
