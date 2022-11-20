pub mod controller;
pub mod service;
pub mod utils;

#[cfg(test)]
mod tests {
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    // type H = <C as GenericConfig<D>>::InnerHasher;
    type F = <C as GenericConfig<D>>::F;
    // type K = Wrapper<HashOut<F>>;
    // type V = Wrapper<HashOut<F>>;
    // type I = Wrapper<HashOut<F>>;

    use std::sync::{Arc, Mutex};

    use intmax_zkp_core::{rollup, sparse_merkle_tree, transaction, zkdsa};
    use plonky2::{
        field::types::Field,
        hash::hash_types::HashOut,
        plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
    };
    use rollup::gadgets::deposit_block::DepositInfo;
    use sparse_merkle_tree::{
        goldilocks_poseidon::{
            GoldilocksHashOut, LayeredLayeredPoseidonSparseMerkleTree, NodeDataMemory,
            PoseidonSparseMerkleTree, WrappedHashOut,
        },
        proof::SparseMerkleInclusionProof,
    };
    use transaction::{block_header::get_block_hash, gadgets::merge::MergeProof};
    use zkdsa::account::{Account, Address};

    use crate::{
        service::Config,
        utils::key_management::{memory::WalletOnMemory, types::Wallet},
    };

    #[test]
    fn test_simple_scenario() -> reqwest::Result<()> {
        let service = Config::new("http://localhost:8080");

        let password = "password";

        let mut wallet = WalletOnMemory::new(password.to_string());

        let sender1_account = Account::<F>::rand();

        wallet.add_account(sender1_account);

        // dbg!(&purge_proof_circuit_data.common);

        let mut sender1_user_asset_tree: LayeredLayeredPoseidonSparseMerkleTree<NodeDataMemory> =
            LayeredLayeredPoseidonSparseMerkleTree::new(Default::default(), Default::default());

        let mut sender1_tx_diff_tree: LayeredLayeredPoseidonSparseMerkleTree<NodeDataMemory> =
            LayeredLayeredPoseidonSparseMerkleTree::new(Default::default(), Default::default());

        let sender1_deposit_list = vec![
            DepositInfo {
                receiver_address: sender1_account.address,
                contract_address: Address(*GoldilocksHashOut::from_u128(305)),
                variable_index: *GoldilocksHashOut::from_u128(8012),
                amount: F::from_canonical_u64(2053),
            },
            DepositInfo {
                receiver_address: sender1_account.address,
                contract_address: Address(*GoldilocksHashOut::from_u128(471)),
                variable_index: *GoldilocksHashOut::from_u128(8012),
                amount: F::from_canonical_u64(1111),
            },
        ];

        // deposit のみの block を作成.
        service.deposit_assets(sender1_deposit_list.clone());
        service.trigger_propose_block();
        let deposit_block = service.trigger_approve_block();

        let node_data = Arc::new(Mutex::new(NodeDataMemory::default()));
        let mut deposit_sender1_tree =
            LayeredLayeredPoseidonSparseMerkleTree::new(node_data.clone(), Default::default());

        for deposit_info in sender1_deposit_list.clone() {
            deposit_sender1_tree
                .set(
                    sender1_account.address.0.into(),
                    deposit_info.contract_address.0.into(),
                    deposit_info.variable_index.into(),
                    HashOut::from_partial(&[deposit_info.amount]).into(),
                )
                .unwrap();

            sender1_user_asset_tree
                .set(
                    get_block_hash(&deposit_block.header).into(),
                    deposit_info.contract_address.0.into(),
                    deposit_info.variable_index.into(),
                    HashOut::from_partial(&[deposit_info.amount]).into(),
                )
                .unwrap();
        }

        let zero = GoldilocksHashOut::from_u128(0);
        let proof1 = sender1_user_asset_tree
            .set(
                get_block_hash(&deposit_block.header).into(),
                sender1_deposit_list[0].contract_address.0.into(),
                sender1_deposit_list[0].variable_index.into(),
                zero,
            )
            .unwrap();
        let proof2 = sender1_user_asset_tree
            .set(
                get_block_hash(&deposit_block.header).into(),
                sender1_deposit_list[1].contract_address.0.into(),
                sender1_deposit_list[1].variable_index.into(),
                zero,
            )
            .unwrap();

        let key3 = (
            GoldilocksHashOut::from_u128(407),
            GoldilocksHashOut::from_u128(305),
            GoldilocksHashOut::from_u128(8012),
        );
        let value3 = GoldilocksHashOut::from_u128(2053);
        let key4 = (
            GoldilocksHashOut::from_u128(832),
            GoldilocksHashOut::from_u128(471),
            GoldilocksHashOut::from_u128(8012),
        );
        let value4 = GoldilocksHashOut::from_u128(1111);

        let proof3 = sender1_tx_diff_tree
            .set(key3.0, key3.1, key3.2, value3)
            .unwrap();
        let proof4 = sender1_tx_diff_tree
            .set(key4.0, key4.1, key4.2, value4)
            .unwrap();

        let sender1_input_witness = vec![proof1, proof2];
        let sender1_output_witness = vec![proof3, proof4];

        let deposit_sender1_tree: PoseidonSparseMerkleTree<NodeDataMemory> =
            deposit_sender1_tree.into();
        let sender1_deposit_root = deposit_sender1_tree
            .get(&sender1_account.address.0.into())
            .unwrap();
        // dbg!(sender1_deposit_root);

        let mut sender1_user_asset_tree: PoseidonSparseMerkleTree<NodeDataMemory> =
            sender1_user_asset_tree.into();
        let merge_process_proof = sender1_user_asset_tree
            .set(
                get_block_hash(&deposit_block.header).into(),
                sender1_deposit_root,
            )
            .unwrap();

        let merge_inclusion_proof2 = deposit_sender1_tree
            .find(&sender1_account.address.0.into())
            .unwrap();

        // pseudo tx hash. 他の merge と proof の形式を合わせるために必要.
        let deposit_tx_hash = HashOut::ZERO; // TODO: block hash を代わりに用いる, あるいは `hash(block_hash || 0)` として merge もこの形式に合わせる.
        let mut deposit_tree = PoseidonSparseMerkleTree::new(node_data, Default::default());
        deposit_tree
            .set(deposit_tx_hash.into(), deposit_sender1_tree.get_root())
            .unwrap();
        let merge_inclusion_proof1 = deposit_tree.find(&deposit_tx_hash.into()).unwrap();

        let default_inclusion_proof = SparseMerkleInclusionProof::with_root(Default::default());
        let merge_witnesses = vec![MergeProof {
            is_deposit: true,
            diff_tree_inclusion_proof: (
                deposit_block.header,
                merge_inclusion_proof1,
                merge_inclusion_proof2,
            ),
            merge_process_proof,
            account_tree_inclusion_proof: default_inclusion_proof,
        }];
        let sender1_user_asset_root = WrappedHashOut::default();
        let transaction = service.send_assets(
            sender1_account,
            &merge_witnesses,
            &sender1_input_witness,
            &sender1_output_witness,
            sender1_user_asset_root,
        );

        let new_world_state_root = service.trigger_propose_block();

        let received_signature = service.sign_to_message(sender1_account, new_world_state_root);

        service.send_received_signature(received_signature, transaction.tx_hash);

        service.trigger_approve_block();

        Ok(())
    }
}
