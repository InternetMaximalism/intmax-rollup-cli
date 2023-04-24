use std::hash::Hash;

use plonky2::{
    field::types::Field,
    hash::{
        hash_types::{HashOut, RichField},
        poseidon::PoseidonHash,
    },
    plonk::config::Hasher,
};
use serde::{Deserialize, Serialize};

use crate::{
    merkle_tree::tree::{get_merkle_proof, get_merkle_proof_with_zero, get_merkle_root},
    sparse_merkle_tree::goldilocks_poseidon::WrappedHashOut,
};

use super::circuits::MergeAndPurgeTransitionPublicInputs;

const LOG_MAX_N_BLOCKS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockHeader<F: Field> {
    pub block_number: u32,
    pub prev_block_hash: HashOut<F>,
    pub block_headers_digest: HashOut<F>, // block header tree root
    pub transactions_digest: HashOut<F>,  // state diff tree root
    pub deposit_digest: HashOut<F>,       // deposit tree root (include scroll root)
    pub proposed_world_state_digest: HashOut<F>,
    pub approved_world_state_digest: HashOut<F>,
    pub latest_account_digest: HashOut<F>, // latest account tree
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "WrappedHashOut<F>: Deserialize<'de>"))]
pub struct SerializableBlockHeader<F: RichField> {
    pub block_number: String,
    pub prev_block_hash: WrappedHashOut<F>,
    pub block_headers_digest: WrappedHashOut<F>,
    pub transactions_digest: WrappedHashOut<F>,
    pub deposit_digest: WrappedHashOut<F>,
    pub proposed_world_state_digest: WrappedHashOut<F>,
    pub approved_world_state_digest: WrappedHashOut<F>,
    pub latest_account_digest: WrappedHashOut<F>,
}

// impl<F: RichField> Default for BlockHeader<F> {
//     fn default() -> Self {
//         unimplemented!("please use `new` function instead")
//     }
// }

impl<'de, F: RichField> Deserialize<'de> for BlockHeader<F> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = SerializableBlockHeader::deserialize(deserializer)?;
        let block_number = {
            let raw = value.block_number;
            let raw_without_prefix = raw.strip_prefix("0x").ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "fail to strip 0x-prefix: given value {raw} does not start with 0x",
                ))
            })?;
            let bytes = hex::decode(raw_without_prefix).map_err(|err| {
                serde::de::Error::custom(format!("fail to parse a hex string: {err}"))
            })?;

            u32::from_be_bytes(bytes.try_into().map_err(|err| {
                serde::de::Error::custom(format!("fail to parse to u32: {:?}", err))
            })?)
        };

        Ok(Self {
            block_number,
            prev_block_hash: *value.prev_block_hash,
            block_headers_digest: *value.block_headers_digest,
            transactions_digest: *value.transactions_digest,
            deposit_digest: *value.deposit_digest,
            proposed_world_state_digest: *value.proposed_world_state_digest,
            approved_world_state_digest: *value.approved_world_state_digest,
            latest_account_digest: *value.latest_account_digest,
        })
    }
}

impl<F: RichField> Serialize for BlockHeader<F> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let block_number = format!("0x{}", hex::encode(self.block_number.to_be_bytes()));

        let raw = SerializableBlockHeader {
            block_number,
            prev_block_hash: self.prev_block_hash.into(),
            block_headers_digest: self.block_headers_digest.into(),
            transactions_digest: self.transactions_digest.into(),
            deposit_digest: self.deposit_digest.into(),
            proposed_world_state_digest: self.proposed_world_state_digest.into(),
            approved_world_state_digest: self.approved_world_state_digest.into(),
            latest_account_digest: self.latest_account_digest.into(),
        };

        raw.serialize(serializer)
    }
}

impl<F: RichField> BlockHeader<F> {
    pub fn new(log_num_txs_in_block: usize) -> Self {
        let default_hash = HashOut::ZERO;

        // transaction tree と deposit tree の深さは同じ.
        let default_interior_deposit_digest = default_hash.into();
        let default_deposit_digest = get_merkle_proof_with_zero(
            &[],
            0,
            log_num_txs_in_block,
            default_interior_deposit_digest,
        )
        .root;
        let default_tx_hash = MergeAndPurgeTransitionPublicInputs::default().tx_hash;
        let default_transactions_digest =
            get_merkle_proof_with_zero(&[], 0, log_num_txs_in_block, default_tx_hash).root;
        let default_block_headers_digest = get_merkle_proof(&[], 0, LOG_MAX_N_BLOCKS).root;

        Self {
            block_number: 0,
            prev_block_hash: default_hash,
            block_headers_digest: *default_block_headers_digest,
            transactions_digest: *default_transactions_digest,
            deposit_digest: *default_deposit_digest,
            proposed_world_state_digest: default_hash,
            approved_world_state_digest: default_hash,
            latest_account_digest: default_hash,
        }
    }

    fn encode(&self) -> String {
        hex::encode(self.block_number.to_be_bytes())
            + &WrappedHashOut::from(self.prev_block_hash).to_string()
            + &WrappedHashOut::from(self.selfs_digest).to_string()
            + &WrappedHashOut::from(self.transactions_digest).to_string()
            + &WrappedHashOut::from(self.deposit_digest).to_string()
            + &WrappedHashOut::from(self.proposed_world_state_digest).to_string()
            + &WrappedHashOut::from(self.approved_world_state_digest).to_string()
            + &WrappedHashOut::from(self.latest_account_digest).to_string();
    }

    fn decode(encoded: &str) -> anyhow::Result<Self> {
        let decoded_block_number = dbg!(u32::from_be_bytes(
            hex::decode(&encoded[0..8]).unwrap().try_into().unwrap()
        ));
        let decoded_prev_block_hash = dbg!(WrappedHashOut::from_str(&encoded[8..72]).unwrap());
        let decoded_block_headers_digest =
            dbg!(WrappedHashOut::from_str(&encoded[72..136]).unwrap());
        let decoded_transactions_digest =
            dbg!(WrappedHashOut::from_str(&encoded[136..200]).unwrap());
        let decoded_deposit_digest = dbg!(WrappedHashOut::from_str(&encoded[200..264]).unwrap());
        let decoded_proposed_world_state_digest =
            dbg!(WrappedHashOut::from_str(&encoded[264..328]).unwrap());
        let decoded_approved_world_state_digest =
            dbg!(WrappedHashOut::from_str(&encoded[328..392]).unwrap());
        let decoded_latest_account_digest =
            dbg!(WrappedHashOut::from_str(&encoded[392..456]).unwrap());

        Ok(Self {
            block_number: decoded_block_number,
            prev_block_hash: *decoded_prev_block_hash,
            block_headers_digest: *decoded_block_headers_digest,
            transactions_digest: *decoded_transactions_digest,
            deposit_digest: *decoded_deposit_digest,
            proposed_world_state_digest: *decoded_proposed_world_state_digest,
            approved_world_state_digest: *decoded_approved_world_state_digest,
            latest_account_digest: *decoded_latest_account_digest,
        })
    }
}

pub fn get_block_hash<F: RichField>(block_header: &BlockHeader<F>) -> HashOut<F> {
    let a = PoseidonHash::two_to_one(
        HashOut::from_partial(&[F::from_canonical_u32(block_header.block_number)]),
        block_header.latest_account_digest,
    );
    let b = PoseidonHash::two_to_one(
        block_header.deposit_digest,
        block_header.transactions_digest,
    );
    let c = PoseidonHash::two_to_one(a, b);
    let d = PoseidonHash::two_to_one(
        block_header.proposed_world_state_digest,
        block_header.approved_world_state_digest,
    );
    let e = PoseidonHash::two_to_one(c, d);

    PoseidonHash::two_to_one(block_header.block_headers_digest, e)
}

pub fn get_block_header_tree_proof<F: RichField>(
    block_hashes: &[WrappedHashOut<F>],
    new_block_hash: WrappedHashOut<F>,
    depth: usize,
) -> (Vec<WrappedHashOut<F>>, WrappedHashOut<F>, WrappedHashOut<F>) {
    let current_index = block_hashes.len();
    let old_proof = get_merkle_proof(block_hashes, current_index, depth);
    let new_root = get_merkle_root(current_index, new_block_hash, &old_proof.siblings);

    (old_proof.siblings, old_proof.root, new_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_block_header() {
        use plonky2::field::goldilocks_field::GoldilocksField;

        type F = GoldilocksField;

        let block_header = BlockHeader {
            block_number: 0,
            prev_block_hash: *WrappedHashOut::from_u32(1),
            block_headers_digest: *WrappedHashOut::from_u32(2),
            transactions_digest: *WrappedHashOut::from_u32(3),
            deposit_digest: *WrappedHashOut::from_u32(4),
            proposed_world_state_digest: *WrappedHashOut::from_u32(5),
            approved_world_state_digest: *WrappedHashOut::from_u32(6),
            latest_account_digest: *WrappedHashOut::from_u32(7),
        };
        let _encoded_block_header = serde_json::to_string(&block_header).unwrap();
        let encoded_block_header = "{\"block_number\":\"0x00000000\",\"prev_block_hash\":\"0x0000000000000000000000000000000000000000000000000000000000000001\",\"block_headers_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000002\",\"transactions_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000003\",\"deposit_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000004\",\"proposed_world_state_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000005\",\"approved_world_state_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000006\",\"latest_account_digest\":\"0x0000000000000000000000000000000000000000000000000000000000000007\"}";
        let decoded_block_header: BlockHeader<F> =
            serde_json::from_str(encoded_block_header).unwrap();
        assert_eq!(decoded_block_header, block_header);
    }

    #[test]
    fn test_encode_block_header() {
        let block_header = BlockHeader::new();
        let encoded_block_header = block_header.encode();
        let decoded_block_header = BlockHeader::<F>::decode(&encoded_block_header);

        assert_eq!(decoded_block_header, block_header);
    }
}
