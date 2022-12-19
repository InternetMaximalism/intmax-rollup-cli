use std::collections::HashMap;

use intmax_zkp_core::{
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
use plonky2::field::goldilocks_field::GoldilocksField;
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableWalletOnMemory {
    pub data: Vec<UserState<NodeDataMemory, RootDataMemory>>,
    #[serde(default)]
    pub default_account: Option<Address<F>>,
}

impl From<SerializableWalletOnMemory> for WalletOnMemory {
    fn from(value: SerializableWalletOnMemory) -> Self {
        let mut result = HashMap::new();
        for value in value.data.into_iter() {
            result.insert(value.account.address, value);
        }

        Self {
            data: result,
            default_account: value.default_account,
        }
    }
}

impl<'de> Deserialize<'de> for WalletOnMemory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = SerializableWalletOnMemory::deserialize(deserializer)?;

        Ok(raw.into())
    }
}

impl From<WalletOnMemory> for SerializableWalletOnMemory {
    fn from(value: WalletOnMemory) -> Self {
        Self {
            data: value.data.values().cloned().collect::<Vec<_>>(),
            default_account: value.default_account,
        }
    }
}

impl Serialize for WalletOnMemory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = SerializableWalletOnMemory::from(self.clone());

        raw.serialize(serializer)
    }
}

impl Wallet for WalletOnMemory {
    type Seed = String;
    type Account = Account<F>;
    type Error = anyhow::Error;

    fn new(_password: String) -> Self {
        Self {
            data: HashMap::new(),
            default_account: None,
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
