use std::collections::{HashMap, HashSet};

use intmax_zkp_core::{
    sparse_merkle_tree::goldilocks_poseidon::{GoldilocksHashOut, WrappedHashOut},
    transaction::asset::TokenKind,
    zkdsa::account::Address,
};
use plonky2::{field::goldilocks_field::GoldilocksField, hash::hash_types::RichField};
use serde::{Deserialize, Serialize};

/// 受け取った token を merge key とともに保管する構造体
/// `(token_kind, amount, merge_key)` の集合
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
    // TODO: tx_hash ではなく merge_key
    pub fn add(&mut self, kind: TokenKind<F>, amount: u64, merge_key: WrappedHashOut<F>) {
        // NOTICE: どの kind と merge_key の組み合わせに対しても要素は高々一つ
        self.0.insert((kind, amount, merge_key));
    }

    pub fn filter(&self, kind: TokenKind<F>) -> Self {
        let mut input_assets = self.0.clone();
        input_assets.retain(|asset| asset.0 == kind);

        Self(input_assets)
    }

    pub fn remove(&mut self, kind: TokenKind<F>) {
        self.0.retain(|asset| asset.0 != kind);
    }

    /// 各 token kind について所持している金額を算出する.
    /// NOTICE: Assets は token を受け取った transaction ごとにバラバラに管理されている.
    pub fn calc_total_amount(&self) -> HashMap<TokenKind<F>, u64> {
        let mut total_amount_map = HashMap::new();
        for asset in self.0.iter() {
            if let Some(amount_list) = total_amount_map.get_mut(&asset.0) {
                *amount_list += asset.1;
            } else {
                total_amount_map.insert(asset.0, asset.1);
            }
        }

        total_amount_map
    }
}

pub trait Wallet {
    type Error;

    /// the type of passwords for accessing this wallet
    type Seed;

    /// the type of account
    /// e.g., a pair of secret key and public key
    type Account;

    /// Initialize the wallet with seed.
    fn new(seed: Self::Seed) -> Self;

    /// Store a account in a wallet.
    /// Panic if the address of the account was already used.
    fn add_account(&mut self, account: Self::Account) -> Result<(), Self::Error>;

    // fn remove_account(&mut self, address: Address<GoldilocksField>);

    /// Change your default account.
    fn set_default_account(&mut self, address: Option<Address<GoldilocksField>>);

    /// Fetch your default account.
    fn get_default_account(&self) -> Option<Address<GoldilocksField>>;
}
