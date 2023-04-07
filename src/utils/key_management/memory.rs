use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use intmax_rollup_interface::intmax_zkp_core::{
    plonky2::field::goldilocks_field::GoldilocksField,
    sparse_merkle_tree::{
        goldilocks_poseidon::{GoldilocksHashOut, NodeDataMemory, RootDataMemory, WrappedHashOut},
        node_data::{Node, NodeData},
        root_data::RootData,
    },
    transaction::{
        asset::{ReceivedAssetProof, TokenKind},
        tree::user_asset::UserAssetTree,
    },
    zkdsa::account::{Account, Address},
};
use serde::{Deserialize, Serialize};

use super::types::{Assets, Wallet};

type F = GoldilocksField;

#[derive(Clone, Debug)]
pub struct UserState<
    D: NodeData<GoldilocksHashOut, GoldilocksHashOut, GoldilocksHashOut>,
    R: RootData<GoldilocksHashOut>,
> {
    pub account: Account<F>,
    pub asset_tree: UserAssetTree<D, R>,
    pub assets: Assets<F>,
    pub last_seen_block_number: u32,

    pub rest_received_assets: Vec<ReceivedAssetProof<GoldilocksField>>,

    /// the set consisting of `(tx_hash, removed_assets, block_number)`.
    #[allow(clippy::type_complexity)]
    pub sent_transactions:
        HashMap<WrappedHashOut<F>, (Vec<(TokenKind<F>, u64, WrappedHashOut<F>)>, Option<u32>)>,
    // HashSet<(
    //     WrappedHashOut<F>,
    //     Vec<(TokenKind<F>, u64, WrappedHashOut<F>)>,
    //     Option<u32>,
    // )>,
}

#[allow(clippy::type_complexity)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableUserState {
    pub account: Account<F>,
    pub asset_tree_nodes: Vec<(
        WrappedHashOut<F>,
        Node<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>>,
    )>,
    pub asset_tree_root: WrappedHashOut<F>,
    pub assets: Assets<F>,
    #[serde(default)]
    pub last_seen_block_number: u32,

    #[serde(default)]
    pub rest_received_assets: Vec<ReceivedAssetProof<GoldilocksField>>,

    #[serde(default)]
    pub sent_transactions: Vec<(
        WrappedHashOut<F>,
        (Vec<(TokenKind<F>, u64, WrappedHashOut<F>)>, Option<u32>),
    )>,
}

impl From<SerializableUserState> for UserState<NodeDataMemory, RootDataMemory> {
    fn from(value: SerializableUserState) -> Self {
        let mut asset_tree_nodes = NodeDataMemory::default();
        asset_tree_nodes
            .multi_insert(value.asset_tree_nodes)
            .unwrap();
        let asset_tree = UserAssetTree::new(asset_tree_nodes, value.asset_tree_root.into());
        let mut sent_transactions = HashMap::new();
        for (key, value) in value.sent_transactions {
            sent_transactions.insert(key, value);
        }

        Self {
            account: value.account,
            asset_tree,
            assets: value.assets,
            last_seen_block_number: value.last_seen_block_number,
            rest_received_assets: value.rest_received_assets,
            sent_transactions,
        }
    }
}

impl<'de> Deserialize<'de> for UserState<NodeDataMemory, RootDataMemory> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = SerializableUserState::deserialize(deserializer)?;

        Ok(raw.into())
    }
}

impl From<UserState<NodeDataMemory, RootDataMemory>> for SerializableUserState {
    fn from(value: UserState<NodeDataMemory, RootDataMemory>) -> Self {
        let asset_tree_root = value.asset_tree.get_root().unwrap();
        let asset_tree_nodes = value.asset_tree.nodes_db.clone();
        let asset_tree_nodes = asset_tree_nodes
            .nodes
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        let sent_transactions = value.sent_transactions.into_iter().collect::<Vec<_>>();

        Self {
            account: value.account,
            asset_tree_nodes,
            asset_tree_root,
            assets: value.assets,
            last_seen_block_number: value.last_seen_block_number,
            rest_received_assets: value.rest_received_assets,
            sent_transactions,
        }
    }
}

impl Serialize for UserState<NodeDataMemory, RootDataMemory> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = SerializableUserState::from(self.clone());

        raw.serialize(serializer)
    }
}

#[derive(Clone)]
pub struct WalletOnMemory {
    pub data: HashMap<Address<F>, UserState<NodeDataMemory, RootDataMemory>>,
    pub default_account: Option<Address<F>>,
    pub wallet_file_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableWalletOnMemory {
    pub data: Vec<UserState<NodeDataMemory, RootDataMemory>>,
    #[serde(default)]
    pub default_account: Option<Address<F>>,
}

impl WalletOnMemory {
    pub fn read_from_file(wallet_file_path: PathBuf) -> anyhow::Result<Self> {
        let mut file = File::open(wallet_file_path.clone())?;
        let mut encoded_wallet = String::new();
        file.read_to_string(&mut encoded_wallet)?;
        let raw: SerializableWalletOnMemory = serde_json::from_str(&encoded_wallet)?;

        let mut result = HashMap::new();
        for value in raw.data.into_iter() {
            result.insert(value.account.address, value);
        }

        Ok(Self {
            data: result,
            default_account: raw.default_account,
            wallet_file_path,
        })
    }
}

impl WalletOnMemory {
    pub fn backup(&self) -> anyhow::Result<()> {
        let raw = SerializableWalletOnMemory {
            data: self.data.values().cloned().collect::<Vec<_>>(),
            default_account: self.default_account,
        };

        let mut wallet_dir_path = self.wallet_file_path.clone();
        wallet_dir_path.pop();
        let encoded_wallet = serde_json::to_string(&raw).unwrap();
        std::fs::create_dir(wallet_dir_path.clone()).unwrap_or(());
        let mut file = File::create(self.wallet_file_path.clone())?;
        write!(file, "{}", encoded_wallet)?;
        file.flush()?;

        Ok(())
    }
}

impl Wallet for WalletOnMemory {
    type Seed = String;
    type Account = Account<F>;
    type Error = anyhow::Error;

    fn new(wallet_file_path: PathBuf, _password: String) -> Self {
        Self {
            data: HashMap::new(),
            default_account: None,
            wallet_file_path,
        }
    }

    fn add_account(&mut self, account: Account<F>) -> anyhow::Result<()> {
        let asset_tree = UserAssetTree::new(NodeDataMemory::default(), RootDataMemory::default());
        let old_account = self.data.insert(
            account.address,
            UserState {
                account,
                asset_tree,
                assets: Default::default(),
                last_seen_block_number: 0,
                rest_received_assets: Default::default(),
                sent_transactions: Default::default(),
            },
        );
        if old_account.is_some() {
            anyhow::bail!("designated address was already used");
        }

        Ok(())
    }

    fn set_default_account(&mut self, address: Option<Address<F>>) {
        self.default_account = address;
    }

    fn get_default_account(&self) -> Option<Address<F>> {
        self.default_account
    }
}
