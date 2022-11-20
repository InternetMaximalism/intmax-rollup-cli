use std::collections::HashSet;

use intmax_zkp_core::{rollup, sparse_merkle_tree, zkdsa};
use plonky2::{field::goldilocks_field::GoldilocksField, hash::hash_types::RichField};
use rollup::circuits::merge_and_purge::MergeAndPurgeTransitionPublicInputs;
use serde::{Deserialize, Serialize};
use sparse_merkle_tree::goldilocks_poseidon::{GoldilocksHashOut, WrappedHashOut};
use zkdsa::account::Address;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenKind<F: RichField> {
    #[serde(bound(deserialize = "Address<F>: Deserialize<'de>"))]
    pub contract_address: Address<F>,
    #[serde(bound(deserialize = "WrappedHashOut<F>: Deserialize<'de>"))]
    pub variable_index: WrappedHashOut<F>,
}

#[test]
fn test_serde_token_kind() {
    use plonky2::{field::types::Sample, hash::hash_types::HashOut};

    let kind: TokenKind<GoldilocksField> = TokenKind {
        contract_address: Address::rand(),
        variable_index: HashOut::rand().into(),
    };
    let encoded_kind = serde_json::to_string(&kind).unwrap();
    let decoded_kind: TokenKind<GoldilocksField> = serde_json::from_str(&encoded_kind).unwrap();
    assert_eq!(decoded_kind, kind);
}

#[derive(Copy, Clone, Debug)]
pub struct Asset<F: RichField> {
    pub kind: TokenKind<F>,
    pub amount: u64,
}

#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct Assets<F: RichField>(pub HashSet<(TokenKind<F>, u64, WrappedHashOut<F>)>);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SerializableAssets(pub Vec<(TokenKind<GoldilocksField>, u64, GoldilocksHashOut)>);

impl From<SerializableAssets> for Assets<GoldilocksField> {
    fn from(value: SerializableAssets) -> Self {
        let mut result = HashSet::new();
        for asset in value.0 {
            result.insert(asset);
        }
        Self(result)
    }
}

impl<'de> Deserialize<'de> for Assets<GoldilocksField> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = SerializableAssets::deserialize(deserializer)?;

        Ok(raw.into())
    }
}

impl From<Assets<GoldilocksField>> for SerializableAssets {
    fn from(value: Assets<GoldilocksField>) -> Self {
        Self(value.0.into_iter().collect::<Vec<_>>())
    }
}

impl Serialize for Assets<GoldilocksField> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = SerializableAssets::from(self.clone());

        raw.serialize(serializer)
    }
}

impl<F: RichField> Assets<F> {
    pub fn add(&mut self, kind: TokenKind<F>, amount: u64, tx_hash: WrappedHashOut<F>) {
        // NOTICE: どの kind と tx_hash の組み合わせに対しても要素は高々一つ
        self.0.insert((kind, amount, tx_hash));
    }

    pub fn filter(&self, kind: TokenKind<F>) -> Self {
        let mut input_assets = self.0.clone();
        input_assets.retain(|asset| asset.0 == kind);

        Self(input_assets)
    }

    pub fn remove(&mut self, kind: TokenKind<F>) {
        self.0.retain(|asset| asset.0 != kind);
    }
}

pub trait Wallet {
    /// the type of passwords for accessing this wallet
    type Seed;

    /// the type of account
    /// e.g., a pair of secret key and public key
    type Account;

    /// Initialize the wallet with seed.
    fn new(seed: Self::Seed) -> Self;

    /// Store a account in a wallet.
    /// Panic if the address of the account was already used.
    fn add_account(&mut self, account: Self::Account);

    // fn remove_account(&mut self, address: Address<GoldilocksField>);

    /// Change your default account.
    fn set_default_account(&mut self, address: Option<Address<GoldilocksField>>);

    /// Fetch your default account.
    fn get_default_account(&self) -> Option<Address<GoldilocksField>>;

    /// Add your pending transactions.
    fn insert_pending_transactions(
        &mut self,
        user_address: Address<GoldilocksField>,
        pending_transactions: &[MergeAndPurgeTransitionPublicInputs<GoldilocksField>],
    );

    /// Fetch your all pending transactions.
    fn get_pending_transaction_hashes(
        &self,
        user_address: Address<GoldilocksField>,
    ) -> Vec<GoldilocksHashOut>;

    /// Returns the removed transaction.
    fn remove_pending_transactions(
        &mut self,
        user_address: Address<GoldilocksField>,
        tx_hash: GoldilocksHashOut,
    ) -> Option<MergeAndPurgeTransitionPublicInputs<GoldilocksField>>;
}
