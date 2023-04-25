use std::{collections::HashMap, str::FromStr};

use intmax_interoperability_plugin::{
    contracts::verifier::verifier_contract,
    ethers::types::{Bytes, H256},
};
use intmax_rollup_interface::{
    constants::{LOCAL_NETWORK_CONFIG, ROLLUP_CONSTANTS},
    intmax_zkp_core::{
        merkle_tree::tree::{get_merkle_root, MerkleProof},
        plonky2::{
            field::types::PrimeField64,
            hash::{hash_types::HashOut, poseidon::PoseidonHash},
            plonk::config::{GenericConfig, Hasher, PoseidonGoldilocksConfig},
        },
        sparse_merkle_tree::{
            goldilocks_poseidon::{
                LayeredPoseidonSparseMerkleTreeMemory, PoseidonNodeHash, WrappedHashOut,
            },
            node_data::Node,
            node_hash::NodeHash,
            proof::SparseMerkleInclusionProof,
        },
        transaction::{
            asset::{ContributedAsset, TokenKind},
            block_header::{get_block_hash, BlockHeader},
        },
        zkdsa::account::Address,
    },
};
use serde_json::json;

use crate::{
    service::interoperability::{
        calc_asset_inclusion_proof, update_transactions_digest, verify_asset_inclusion_proof,
    },
    utils::{
        key_management::{memory::WalletOnMemory, types::Wallet},
        nickname::NicknameTable,
    },
};

use super::builder::ServiceBuilder;

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

pub fn parse_address(
    wallet: &WalletOnMemory,
    nickname_table: &NicknameTable,
    user_address: Option<String>,
) -> anyhow::Result<Address<F>> {
    if let Some(user_address) = user_address {
        let user_address = if user_address.is_empty() {
            anyhow::bail!("empty user address");
        } else if user_address.starts_with("0x") {
            Address::from_str(&user_address)?
        } else if let Some(user_address) = nickname_table.nickname_to_address.get(&user_address) {
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
}

// This function merges received assets for a user until the number of unmerged assets is less than `num_unmerged`.
// During each iteration, `N_MERGES` is subtracted from `user_state.rest_received_assets`.
pub async fn merge(
    service: &ServiceBuilder,
    wallet: &mut WalletOnMemory,
    user_address: Address<F>,
    num_unmerged: usize,
) -> anyhow::Result<()> {
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

        // Merge received assets for the user, and purge the merged assets if they exceed the maximum number of unmerged assets.
        service
            .merge_and_purge_asset(user_state, user_address, &[], false)
            .await?;

        wallet.backup()?;

        service.trigger_propose_block().await.unwrap();
        service.trigger_approve_block().await.unwrap();
    }

    Ok(())
}

pub async fn transfer(
    service: &ServiceBuilder,
    wallet: &mut WalletOnMemory,
    user_address: Address<F>,
    purge_diffs: &[ContributedAsset<F>],
) -> anyhow::Result<Option<WrappedHashOut<F>>> {
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

    // Repeat merging until there are `N_MERGES` unmerged differences remaining.
    // The remaining differences are included in the transaction with purge.
    merge(service, wallet, user_address, ROLLUP_CONSTANTS.n_merges).await?;

    let tx_hash = {
        let user_state = wallet
            .data
            .get_mut(&user_address)
            .expect("user address was not found in wallet");

        let result = service
            .merge_and_purge_asset(user_state, user_address, purge_diffs, true)
            .await;
        let tx_hash = match result {
            Ok(tx_hash) => Some(tx_hash),
            Err(err) => {
                if err.to_string() == "nothing to do" {
                    #[cfg(feature = "verbose")]
                    println!("nothing to do");

                    None
                } else {
                    return Err(err);
                }
            }
        };

        wallet.backup()?;

        tx_hash
    };

    service.trigger_propose_block().await.unwrap();

    {
        let user_state = wallet
            .data
            .get_mut(&user_address)
            .expect("user address was not found in wallet");

        service.sign_proposed_block(user_state, user_address).await;

        wallet.backup()?;
    }

    service.trigger_approve_block().await.unwrap();

    Ok(tx_hash)
}

pub async fn bulk_mint(
    service: &ServiceBuilder,
    wallet: &mut WalletOnMemory,
    user_address: Address<F>,
    distribution_list: Vec<ContributedAsset<F>>,
    need_deposit: bool,
) -> anyhow::Result<()> {
    // {
    //     let user_state = wallet
    //         .data
    //         .get_mut(&user_address)
    //         .expect("user address was not found in wallet");

    //     service.sync_sent_transaction(user_state, user_address);

    //     backup_wallet(wallet)?;
    // }

    // Organize by destination and token.
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

        // Even if you issue tokens to others, you must first deposit them to yourself.
        deposit_list
            .iter_mut()
            .for_each(|v| v.receiver_address = user_address);

        service.deposit_assets(user_address, deposit_list).await?;

        service.trigger_propose_block().await.unwrap();
        service.trigger_approve_block().await.unwrap();
    }

    let purge_diffs = distribution_list
        .into_iter()
        .filter(|v| v.receiver_address != user_address)
        .collect::<Vec<_>>();

    transfer(service, wallet, user_address, &purge_diffs).await?;

    Ok(())
}

pub fn smt_proof_to_merkle_proof(
    smt_proof: &SparseMerkleInclusionProof<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>>,
) -> anyhow::Result<MerkleProof<F>> {
    if !smt_proof.found {
        anyhow::bail!("cannot convert a exclusion SMT proof to Merkle proof");
    }

    let mut index_rbo = smt_proof.key.elements[0].to_canonical_u64(); // reverse bit order
    let mut index = 0u64;
    for _ in smt_proof.siblings.iter() {
        index <<= 1;
        index += index_rbo & 1;
        index_rbo >>= 1;
    }

    let mut siblings = smt_proof.siblings.clone();
    siblings.reverse();

    let value = PoseidonNodeHash::calc_node_hash(Node::Leaf(smt_proof.key, smt_proof.value));

    Ok(MerkleProof {
        root: smt_proof.root,
        index: index as usize,
        value,
        siblings,
    })
}

pub async fn create_transaction_proof(service: &ServiceBuilder, tx_hash: HashOut<F>) {
    let (tx_details, transaction_proof, block_header, signature) =
        service.get_transaction_proof(tx_hash).await.unwrap();
    // dbg!(&tx_details, &transaction_proof, &block_header);

    let mut tx_diff_tree = LayeredPoseidonSparseMerkleTreeMemory::default();

    for asset in tx_details.assets.iter() {
        tx_diff_tree
            .set(
                asset.kind.contract_address.to_hash_out().into(),
                asset.kind.variable_index.to_hash_out().into(),
                WrappedHashOut::from_u64(asset.amount),
            )
            .unwrap();
    }

    // dbg!(tx_diff_tree.get_root().unwrap());

    assert_eq!(tx_details.assets.len(), 1);
    let target_asset = &tx_details.assets[0];
    let asset_proof = tx_diff_tree
        .find(
            &target_asset.kind.contract_address.to_hash_out().into(),
            &target_asset.kind.variable_index.to_hash_out().into(),
        )
        .unwrap();
    // dbg!(&asset_proof);

    let calculated_tx_hash =
        PoseidonHash::two_to_one(*tx_details.inclusion_witness.root, *tx_details.nonce);
    // dbg!(&calculated_tx_hash);

    // let block_number = block_header.block_number;
    // let block_details = service.get_block_details(block_number).await.unwrap();
    // let user_address = ;

    let block_hash = get_block_hash(&block_header);
    // dbg!(WrappedHashOut::from(block_hash).to_string());

    let asset_proof0 = tx_details.inclusion_witness;
    let asset_proof1 = asset_proof.0;
    let asset_proof2 = asset_proof.1;

    {
        let proof = &asset_proof0;
        let proof = smt_proof_to_merkle_proof(proof).unwrap();
        assert_eq!(
            get_merkle_root(proof.index, proof.value, &proof.siblings),
            proof.root
        );
        let proof = &asset_proof1;
        let proof = smt_proof_to_merkle_proof(proof).unwrap();
        assert_eq!(
            get_merkle_root(proof.index, proof.value, &proof.siblings),
            proof.root
        );
        let proof = &asset_proof2;
        let proof = smt_proof_to_merkle_proof(proof).unwrap();
        assert_eq!(
            get_merkle_root(proof.index, proof.value, &proof.siblings),
            proof.root
        );
    }
    // let witness = (
    //     signature.clone(),
    //     block_hash,
    //     block_header.clone(),
    //     transaction_proof.clone(),
    //     calculated_tx_hash,
    //     tx_details.nonce,
    //     asset_proof0.clone(),
    //     asset_proof1,
    //     asset_proof2,
    //     *target_asset,
    // );
    // dbg!(&witness);

    let network_config = LOCAL_NETWORK_CONFIG;
    let secret_key = std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");

    update_transactions_digest(
        &network_config,
        secret_key.clone(),
        block_header.clone(),
        Bytes::from_str(&signature[2..]).unwrap(),
    )
    .await;

    let witness = calc_asset_inclusion_proof(
        &network_config,
        secret_key.clone(),
        H256::from_str(&tx_details.nonce.to_string()[2..])
            .unwrap()
            .into(),
        asset_proof0
            .siblings
            .iter()
            .map(|v| H256::from_str(&v.to_string()[2..]).unwrap().into())
            .collect::<Vec<_>>(),
        transaction_proof.clone(),
        block_header.clone(),
    )
    .await;

    let asset: verifier_contract::Asset = verifier_contract::Asset {
        recipient: H256::from_str(&asset_proof0.key.to_string()[2..])
            .unwrap()
            .into(),
        token_address: H256::from_str(
            &WrappedHashOut::from(target_asset.kind.contract_address.to_hash_out()).to_string()
                [2..],
        )
        .unwrap()
        .into(),
        token_id: target_asset.kind.variable_index.0.into(),
        amount: target_asset.amount.into(),
    };
    let ok = verify_asset_inclusion_proof(&network_config, secret_key, asset, witness).await;
    assert!(ok);

    let encoded_transaction_proof_siblings = transaction_proof
        .siblings
        .iter()
        .map(|v| v.to_string()[2..].to_string())
        .collect::<Vec<_>>();
    let encoded_transaction_proof = transaction_proof.root.to_string()
        + &hex::encode(transaction_proof.index.to_be_bytes())
        + &transaction_proof.value.to_string()[2..]
        + &hex::encode(encoded_transaction_proof_siblings.len().to_be_bytes())
        + &encoded_transaction_proof_siblings.join("");

    {
        let encoded_transaction_proof = encoded_transaction_proof[2..].to_string();
        let decoded_root =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_transaction_proof[0..64]))
                .unwrap();
        let decoded_index = usize::from_be_bytes(
            hex::decode(&encoded_transaction_proof[64..80])
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let decoded_value =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_transaction_proof[80..144]))
                .unwrap();
        let decoded_siblings_len = usize::from_be_bytes(
            hex::decode(&encoded_transaction_proof[144..160])
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let mut decoded_siblings = vec![];
        for i in 0..decoded_siblings_len {
            decoded_siblings.push(
                WrappedHashOut::from_str(
                    &("0x".to_string()
                        + &encoded_transaction_proof[160 + i * 64..160 + (i + 1) * 64]),
                )
                .unwrap(),
            );
        }
        let decoded_transaction_proof = MerkleProof {
            index: decoded_index,
            value: decoded_value,
            siblings: decoded_siblings,
            root: decoded_root,
        };
        println!("encoded_transaction_proof: {}", encoded_transaction_proof);
        dbg!(format!("{}", json!(&transaction_proof)));
        assert_eq!(decoded_transaction_proof, transaction_proof);
    }

    let encoded_block_header = "0x".to_string()
        + &WrappedHashOut::<F>::from_u32(block_header.block_number).to_string()[2..]
        + &WrappedHashOut::from(block_header.prev_block_hash).to_string()[2..]
        + &WrappedHashOut::from(block_header.block_headers_digest).to_string()[2..]
        + &WrappedHashOut::from(block_header.transactions_digest).to_string()[2..]
        + &WrappedHashOut::from(block_header.deposit_digest).to_string()[2..]
        + &WrappedHashOut::from(block_header.proposed_world_state_digest).to_string()[2..]
        + &WrappedHashOut::from(block_header.approved_world_state_digest).to_string()[2..]
        + &WrappedHashOut::from(block_header.latest_account_digest).to_string()[2..];

    {
        let encoded_block_header = encoded_block_header[2..].to_string();
        let decoded_block_number =
            WrappedHashOut::<F>::from_str(&("0x".to_string() + &encoded_block_header[0..64]))
                .unwrap()
                .to_u32();
        let decoded_prev_block_hash =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[64..128])).unwrap();
        let decoded_block_headers_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[128..192]))
                .unwrap();
        let decoded_transactions_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[192..256]))
                .unwrap();
        let decoded_deposit_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[256..320]))
                .unwrap();
        let decoded_proposed_world_state_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[320..384]))
                .unwrap();
        let decoded_approved_world_state_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[384..448]))
                .unwrap();
        let decoded_latest_account_digest =
            WrappedHashOut::from_str(&("0x".to_string() + &encoded_block_header[448..512]))
                .unwrap();
        let decoded_block_header = BlockHeader::<F> {
            block_number: decoded_block_number,
            prev_block_hash: *decoded_prev_block_hash,
            block_headers_digest: *decoded_block_headers_digest,
            transactions_digest: *decoded_transactions_digest,
            deposit_digest: *decoded_deposit_digest,
            proposed_world_state_digest: *decoded_proposed_world_state_digest,
            approved_world_state_digest: *decoded_approved_world_state_digest,
            latest_account_digest: *decoded_latest_account_digest,
        };
        println!("encoded_block_header: {}", encoded_block_header);
        dbg!(format!("{}", json!(&block_header)));
        assert_eq!(decoded_block_header, block_header);
    }

    // let diff_tree_inclusion_proof = transaction_proof;
    // let block_info: BlockInfo<F> = block_details.into();
    // let block_header_keccak = block_info.calc_block_header_keccak(ROLLUP_CONSTANTS.log_n_txs);
    // let block_hash_keccak = block_header_keccak.block_hash();

    // // transaction details
    // let recipient = "0x00000000000000000000000000000000000000000000000010d1cb00b658931e";
    // let token_address = "0x000000000000000000000000000000000000000000000000f7c23e5c2d79b6ae";
    // let token_id = "0x0000000000000000000000000000000000000000000000000000000000000000";
    // let token_amount = 3;
    // let nonce = "0xa710189dc0d8eb00a46e0411c0b1965192f80c50fbd8cbd51b5c67b26fc9dff1";

    // let mut recipient_merkle_siblings = todo!();
    // recipient_merkle_siblings.reverse();
}
