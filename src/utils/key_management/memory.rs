use std::collections::HashMap;

use intmax_zkp_core::{
    sparse_merkle_tree::{
        goldilocks_poseidon::{
            GoldilocksHashOut, LayeredLayeredPoseidonSparseMerkleTree, NodeDataMemory,
            RootDataMemory, WrappedHashOut,
        },
        node_data::{Node, NodeData},
        root_data::RootData,
    },
    transaction::{circuits::MergeAndPurgeTransitionPublicInputs, gadgets::merge::MergeProof},
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
    pub asset_tree: LayeredLayeredPoseidonSparseMerkleTree<D, R>,
    pub assets: Assets<F>,
    pub last_seen_block_number: u32,
    pub transactions: HashMap<WrappedHashOut<F>, MergeAndPurgeTransitionPublicInputs<F>>,
    pub rest_merge_witnesses: Vec<MergeProof<GoldilocksField>>,
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
    pub transactions: Vec<MergeAndPurgeTransitionPublicInputs<F>>, // pending_transactions
    #[serde(default)]
    pub rest_merge_witnesses: Vec<MergeProof<GoldilocksField>>,
}

impl From<SerializableUserState> for UserState<NodeDataMemory, RootDataMemory> {
    fn from(value: SerializableUserState) -> Self {
        let mut asset_tree_nodes = NodeDataMemory::default();
        asset_tree_nodes
            .multi_insert(value.asset_tree_nodes)
            .unwrap();
        let asset_tree = LayeredLayeredPoseidonSparseMerkleTree::new(
            asset_tree_nodes,
            value.asset_tree_root.into(),
        );
        let mut transactions = HashMap::new();
        for tx in value.transactions {
            transactions.insert(tx.tx_hash, tx);
        }

        Self {
            account: value.account,
            asset_tree,
            assets: value.assets,
            last_seen_block_number: value.last_seen_block_number,
            transactions,
            rest_merge_witnesses: value.rest_merge_witnesses,
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
        let transactions = value.transactions.values().cloned().collect::<Vec<_>>();

        Self {
            account: value.account,
            asset_tree_nodes,
            asset_tree_root,
            assets: value.assets,
            transactions,
            last_seen_block_number: value.last_seen_block_number,
            rest_merge_witnesses: value.rest_merge_witnesses,
        }
    }
}

impl Serialize for UserState<NodeDataMemory, RootDataMemory> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = SerializableUserState::from(self.clone());

        raw.serialize(serializer)
    }
}

impl<
        D: NodeData<GoldilocksHashOut, GoldilocksHashOut, GoldilocksHashOut>,
        R: RootData<GoldilocksHashOut>,
    > UserState<D, R>
{
    pub fn insert_pending_transactions(
        &mut self,
        pending_transactions: &[MergeAndPurgeTransitionPublicInputs<GoldilocksField>],
    ) {
        for tx in pending_transactions {
            self.transactions.insert(tx.tx_hash, tx.clone());
        }
    }

    pub fn get_pending_transaction_hashes(&self) -> Vec<GoldilocksHashOut> {
        self.transactions.keys().cloned().collect::<Vec<_>>()
    }

    pub fn remove_pending_transactions(
        &mut self,
        tx_hash: GoldilocksHashOut,
    ) -> Option<MergeAndPurgeTransitionPublicInputs<GoldilocksField>> {
        self.transactions.remove(&tx_hash)
    }
}

#[derive(Clone)]
pub struct WalletOnMemory {
    pub data: HashMap<Address<F>, UserState<NodeDataMemory, RootDataMemory>>,
    pub default_account: Option<Address<F>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableWalletOnMemory {
    // #[serde(bound(
    //     serialize = "Vec<UserState<NodeDataMemory, RootDataMemory>>: Serialize",
    //     deserialize = "Vec<UserState<NodeDataMemory, RootDataMemory>>: Deserialize<'de>"
    // ))]
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

    fn new(_password: String) -> Self {
        Self {
            data: HashMap::new(),
            default_account: None,
        }
    }

    fn add_account(&mut self, account: Account<F>) {
        let asset_tree = LayeredLayeredPoseidonSparseMerkleTree::default();
        let old_account = self.data.insert(
            account.address,
            UserState {
                account,
                asset_tree,
                assets: Default::default(),
                transactions: Default::default(),
                last_seen_block_number: 0, // TODO: current latest block number
                rest_merge_witnesses: Default::default(),
            },
        );
        assert!(old_account.is_none(), "designated address was already used");
    }

    fn set_default_account(&mut self, address: Option<Address<F>>) {
        self.default_account = address;
    }

    fn get_default_account(&self) -> Option<Address<F>> {
        self.default_account
    }

    fn insert_pending_transactions(
        &mut self,
        user_address: Address<F>,
        pending_transactions: &[MergeAndPurgeTransitionPublicInputs<GoldilocksField>],
    ) {
        self.data
            .get_mut(&user_address)
            .expect("account was not found")
            .insert_pending_transactions(pending_transactions);
    }

    fn get_pending_transaction_hashes(&self, user_address: Address<F>) -> Vec<GoldilocksHashOut> {
        self.data
            .get(&user_address)
            .expect("account was not found")
            .get_pending_transaction_hashes()
    }

    fn remove_pending_transactions(
        &mut self,
        user_address: Address<F>,
        tx_hash: GoldilocksHashOut,
    ) -> Option<MergeAndPurgeTransitionPublicInputs<GoldilocksField>> {
        self.data
            .get_mut(&user_address)
            .expect("account was not found")
            .remove_pending_transactions(tx_hash)
    }
}
