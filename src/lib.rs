pub mod controller;
pub mod service;
pub mod utils;

pub extern crate intmax_rollup_interface;

#[cfg(test)]
mod tests {
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    // type H = <C as GenericConfig<D>>::InnerHasher;
    type F = <C as GenericConfig<D>>::F;

    use intmax_rollup_interface::{
        constants::*,
        intmax_zkp_core::{
            merkle_tree::tree::get_merkle_proof,
            plonky2::{
                field::{goldilocks_field::GoldilocksField, types::Field},
                hash::{hash_types::HashOut, poseidon::PoseidonHash},
                plonk::config::{GenericConfig, Hasher, PoseidonGoldilocksConfig},
            },
            rollup::gadgets::deposit_block::DepositInfo,
            sparse_merkle_tree::{
                goldilocks_poseidon::{
                    GoldilocksHashOut, LayeredLayeredPoseidonSparseMerkleTree,
                    LayeredPoseidonSparseMerkleTree, NodeDataMemory, PoseidonSparseMerkleTree,
                    RootDataMemory, RootDataTmp, WrappedHashOut,
                },
                proof::SparseMerkleInclusionProof,
            },
            transaction::{block_header::get_block_hash, gadgets::merge::MergeProof},
            zkdsa::account::{Account, Address},
        },
    };

    use crate::{
        service::builder::*,
        utils::key_management::{memory::WalletOnMemory, types::Wallet},
    };

    #[tokio::test]
    async fn test_simple_scenario() -> reqwest::Result<()> {
        let service = ServiceBuilder::new("http://localhost:8080");

        let password = "password";

        let mut wallet = WalletOnMemory::new("".into(), password.to_string());

        let sender1_account = Account::<F>::rand();

        let sender2_account = Account::<F>::rand();

        wallet.add_account(sender1_account).unwrap();

        // dbg!(&purge_proof_circuit_data.common);

        let sender1_nodes_db = NodeDataMemory::default();

        let deposit_list = vec![
            DepositInfo {
                receiver_address: sender1_account.address,
                contract_address: Address(GoldilocksField::from_canonical_u64(1)),
                variable_index: 0u8.into(),
                amount: F::from_canonical_u64(10),
            },
            // DepositInfo {
            //     receiver_address: sender1_account.address,
            //     contract_address: Address(*GoldilocksHashOut::from_u128(471)),
            //     variable_index: *GoldilocksHashOut::from_u128(8012),
            //     amount: F::from_canonical_u64(1111),
            // },
        ];

        service
            .deposit_assets(
                sender1_account.address,
                deposit_list
                    .iter()
                    .cloned()
                    .map(|v| v.into())
                    .collect::<Vec<_>>(),
            )
            .await
            .unwrap();
        service.trigger_propose_block().await;
        let deposit_block = service.trigger_approve_block().await;

        let mut deposit_sender1_tree = LayeredLayeredPoseidonSparseMerkleTree::new(
            sender1_nodes_db.clone(),
            RootDataTmp::default(),
        );

        for deposit_info in deposit_list.clone() {
            deposit_sender1_tree
                .set(
                    deposit_info.receiver_address.to_hash_out().into(),
                    deposit_info.contract_address.to_hash_out().into(),
                    deposit_info.variable_index.to_hash_out().into(),
                    HashOut::from_partial(&[deposit_info.amount]).into(),
                )
                .unwrap();
        }

        let deposit_diff_root =
            PoseidonHash::two_to_one(*deposit_sender1_tree.get_root().unwrap(), HashOut::ZERO);

        let deposit_sender1_tree: PoseidonSparseMerkleTree<NodeDataMemory, RootDataTmp> =
            deposit_sender1_tree.into();

        let merge_inclusion_proof2 = deposit_sender1_tree
            .find(&sender1_account.address.to_hash_out().into())
            .unwrap();

        // Calculate pseudo tx hash. This is needed to match other merge and proof formats.
        let merge_inclusion_proof1 =
            get_merkle_proof(&[deposit_diff_root.into()], 0, ROLLUP_CONSTANTS.log_n_txs);

        let deposit_nonce = WrappedHashOut::ZERO;
        let default_inclusion_proof = SparseMerkleInclusionProof::with_root(Default::default());

        let mut sender1_inner_user_asset_tree =
            LayeredPoseidonSparseMerkleTree::new(sender1_nodes_db.clone(), RootDataTmp::default());

        for deposit_info in deposit_list.clone() {
            sender1_inner_user_asset_tree
                .set(
                    deposit_info.contract_address.to_hash_out().into(),
                    deposit_info.variable_index.to_hash_out().into(),
                    HashOut::from_partial(&[deposit_info.amount]).into(),
                )
                .unwrap();
        }

        let mut sender1_user_asset_tree =
            PoseidonSparseMerkleTree::new(sender1_nodes_db.clone(), RootDataTmp::default());

        let block_hash = get_block_hash(&deposit_block.header);
        let deposit_merge_key = PoseidonHash::two_to_one(deposit_diff_root, block_hash);
        let merge_process_proof = sender1_user_asset_tree
            .set(
                deposit_merge_key.into(),
                sender1_inner_user_asset_tree.get_root().unwrap(),
            )
            .unwrap();

        let diff_tree_inclusion_proof = (
            deposit_block.header,
            merge_inclusion_proof1,
            merge_inclusion_proof2,
        );
        let merge_witnesses = vec![MergeProof {
            is_deposit: true,
            diff_tree_inclusion_proof,
            merge_process_proof,
            latest_account_tree_inclusion_proof: default_inclusion_proof,
            nonce: deposit_nonce,
        }];

        for merge_witness in merge_witnesses.iter() {
            let block_header = &merge_witness.diff_tree_inclusion_proof.0;
            let root = if merge_witness.is_deposit {
                block_header.deposit_digest
            } else {
                block_header.transactions_digest
            };
            assert_eq!(root, *merge_witness.diff_tree_inclusion_proof.1.root);
        }

        let mut sender1_user_asset_tree: LayeredLayeredPoseidonSparseMerkleTree<NodeDataMemory, _> =
            sender1_user_asset_tree.into();

        let zero = WrappedHashOut::ZERO;
        let proof1 = sender1_user_asset_tree
            .set(
                deposit_merge_key.into(),
                deposit_list[0].contract_address.to_hash_out().into(),
                deposit_list[0].variable_index.to_hash_out().into(),
                zero,
            )
            .unwrap();
        // let proof2 = sender1_user_asset_tree
        //     .set(
        //         deposit_tx_hash.into(),
        //         sender1_deposit_list[1].contract_address.0.into(),
        //         sender1_deposit_list[1].variable_index.into(),
        //         zero,
        //     )
        //     .unwrap();

        let mut sender1_tx_diff_tree = LayeredLayeredPoseidonSparseMerkleTree::new(
            sender1_nodes_db,
            RootDataMemory::default(),
        );

        let key3 = (
            sender2_account.address.to_hash_out().into(),
            deposit_list[0].contract_address.to_hash_out().into(),
            deposit_list[0].variable_index.to_hash_out().into(),
        );
        let value3 = GoldilocksHashOut::from_u128(2);
        let key4 = (
            sender1_account.address.to_hash_out().into(),
            deposit_list[0].contract_address.to_hash_out().into(),
            deposit_list[0].variable_index.to_hash_out().into(),
        );
        let value4 = GoldilocksHashOut::from_u128(8);

        let proof3 = sender1_tx_diff_tree
            .set(key3.0, key3.1, key3.2, value3)
            .unwrap();
        let proof4 = sender1_tx_diff_tree
            .set(key4.0, key4.1, key4.2, value4)
            .unwrap();

        let purge_input_witness = vec![proof1];
        let purge_output_witness = vec![proof3, proof4];

        let sender1_user_asset_root = WrappedHashOut::default();
        let nonce = WrappedHashOut::rand();
        let transaction = service
            .send_assets(
                sender1_account,
                &merge_witnesses,
                &purge_input_witness,
                &purge_output_witness,
                nonce,
                sender1_user_asset_root,
            )
            .await;

        let new_world_state_root = service.trigger_propose_block().await;

        let received_signature = sign_to_message(sender1_account, new_world_state_root).await;

        service
            .send_received_signature(received_signature, transaction.tx_hash)
            .await;

        service.trigger_approve_block().await;

        Ok(())
    }
}
