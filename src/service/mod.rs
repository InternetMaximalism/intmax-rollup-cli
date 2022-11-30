use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use intmax_rollup_interface::{constants::*, interface::*};
use intmax_zkp_core::{
    rollup::{block::BlockInfo, deposit::make_deposit_proof, gadgets::deposit_block::DepositInfo},
    sparse_merkle_tree::{
        gadgets::{process::process_smt::SmtProcessProof, verify::verify_smt::SmtInclusionProof},
        goldilocks_poseidon::{
            LayeredLayeredPoseidonSparseMerkleTree, LayeredPoseidonSparseMerkleTree,
            NodeDataMemory, PoseidonSparseMerkleTree, RootDataMemory, RootDataTmp, WrappedHashOut,
        },
    },
    transaction::{
        asset::{Asset, ReceivedAssetProof, TokenKind},
        block_header::get_block_hash,
        circuits::{make_user_proof_circuit, MergeAndPurgeTransitionPublicInputs},
        gadgets::merge::MergeProof,
    },
    zkdsa::{
        account::{Account, Address},
        circuits::{make_simple_signature_circuit, SimpleSignatureProofWithPublicInputs},
    },
};
use plonky2::{
    field::types::{Field, PrimeField64},
    hash::{hash_types::HashOut, poseidon::PoseidonHash},
    iop::witness::PartialWitness,
    plonk::config::{GenericConfig, Hasher, PoseidonGoldilocksConfig},
};
use serde::{Deserialize, Serialize};

use crate::utils::key_management::memory::UserState;

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

const CONTENT_TYPE: &str = "Content-Type";

#[derive(Clone, Debug)]
pub struct Config {
    aggregator_url: Arc<Mutex<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableConfig {
    pub aggregator_url: String,
}

impl serde::Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let aggregator_url: String = self.aggregator_url.lock().unwrap().clone();
        let raw = SerializableConfig { aggregator_url };

        raw.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = SerializableConfig::deserialize(deserializer)?;

        let result = Config {
            aggregator_url: Arc::new(Mutex::new(raw.aggregator_url)),
        };

        Ok(result)
    }
}

impl Config {
    pub fn new(aggregator_url: &str) -> Self {
        Self {
            aggregator_url: Arc::new(Mutex::new(aggregator_url.to_string())),
        }
    }

    pub fn aggregator_api_url(&self, api_path: &str) -> String {
        let mut base_url: String = self.aggregator_url.lock().unwrap().clone();

        if base_url.ends_with('/') {
            base_url.pop();
        }

        base_url + api_path
    }

    pub fn set_aggregator_url(&self, aggregator_url: Option<String>) {
        if let Some(aggregator_url) = aggregator_url {
            let new_url = aggregator_url;
            let _ = std::mem::replace::<String>(
                &mut self.aggregator_url.lock().unwrap(),
                new_url.clone(),
            );
            println!("The new aggregator URL is {}", new_url);
        } else {
            println!("The aggregator URL is {}", self.aggregator_api_url(""));
        }
    }

    /// test function
    pub fn deposit_assets(&self, deposit_info: Vec<DepositInfo<F>>) {
        let payload = RequestDepositAddBody { deposit_info };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/test/deposit/add"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseDepositAddBody>()
            .expect("fail to parse JSON");

        if resp.ok {
            println!("deposit successfully");
        } else {
            panic!("fail to deposit");
        }
    }

    pub fn send_assets(
        &self,
        account: Account<F>,
        merge_witnesses: &[MergeProof<F>],
        purge_input_witnesses: &[(SmtProcessProof<F>, SmtProcessProof<F>, SmtProcessProof<F>)],
        purge_output_witnesses: &[(SmtProcessProof<F>, SmtProcessProof<F>, SmtProcessProof<F>)],
        nonce: WrappedHashOut<F>,
        user_asset_root: WrappedHashOut<F>,
    ) -> MergeAndPurgeTransitionPublicInputs<F> {
        let user_tx_proof = {
            let merge_and_purge_circuit = make_user_proof_circuit::<
                F,
                C,
                D,
                N_LOG_MAX_USERS,
                N_LOG_MAX_TXS,
                N_LOG_MAX_CONTRACTS,
                N_LOG_MAX_VARIABLES,
                N_LOG_TXS,
                N_LOG_RECIPIENTS,
                N_LOG_CONTRACTS,
                N_LOG_VARIABLES,
                N_DIFFS,
                N_MERGES,
            >();

            let mut pw = PartialWitness::new();
            let _public_inputs = merge_and_purge_circuit.targets.set_witness(
                &mut pw,
                account.address,
                merge_witnesses,
                purge_input_witnesses,
                purge_output_witnesses,
                nonce,
                user_asset_root,
            );
            // dbg!(serde_json::to_string(&public_inputs).unwrap());

            println!("start proving: user_tx_proof");
            let start = Instant::now();
            let user_tx_proof = merge_and_purge_circuit.prove(pw).unwrap();
            let end = start.elapsed();
            println!("prove: {}.{:03} sec", end.as_secs(), end.subsec_millis());

            // dbg!(&sender1_tx_proof.public_inputs);

            match merge_and_purge_circuit.verify(user_tx_proof.clone()) {
                Ok(()) => println!("Ok!"),
                Err(x) => println!("{}", x),
            }

            user_tx_proof
        };

        let transaction = user_tx_proof.public_inputs.clone();
        println!("transaction hash is 0x{}", transaction.diff_root);

        let payload = RequestTxSendBody { user_tx_proof };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/tx/send"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseTxSendBody>()
            .expect("fail to parse JSON");

        assert_eq!(resp.tx_hash, transaction.tx_hash);

        transaction
    }

    pub fn merge_deposits(
        &self,
        blocks: Vec<BlockInfo<F>>,
        user_address: Address<F>,
        user_state: &mut UserState<NodeDataMemory, RootDataMemory>,
    ) -> Vec<MergeProof<F>> {
        let mut merge_witnesses = vec![];
        for block in blocks {
            let user_deposits = block
                .deposit_list
                .iter()
                .filter(|leaf| leaf.receiver_address == user_address)
                .collect::<Vec<_>>();
            let (deposit_proof1, deposit_proof2) =
                make_deposit_proof(&block.deposit_list, user_address, N_LOG_TXS);
            dbg!(&deposit_proof1.root, deposit_proof1.value);

            let deposit_tx_hash =
                PoseidonHash::two_to_one(*deposit_proof1.value, get_block_hash(&block.header))
                    .into();
            dbg!(&deposit_tx_hash);
            let merge_key = deposit_tx_hash;

            let diff_tree_inclusion_proof = (block.header, deposit_proof1, deposit_proof2);

            for found_deposit_info in user_deposits {
                let mut inner_asset_tree = LayeredPoseidonSparseMerkleTree::new(
                    user_state.asset_tree.nodes_db.clone(),
                    RootDataTmp::default(),
                );
                {
                    user_state.assets.add(
                        TokenKind {
                            contract_address: found_deposit_info.contract_address,
                            variable_index: found_deposit_info.variable_index.into(),
                        },
                        found_deposit_info.amount.to_canonical_u64(),
                        merge_key,
                    );
                    inner_asset_tree
                        .set(
                            found_deposit_info.contract_address.0.into(),
                            found_deposit_info.variable_index.into(),
                            HashOut::from_partial(&[found_deposit_info.amount]).into(),
                        )
                        .unwrap();
                }

                let asset_root = inner_asset_tree.get_root().unwrap();

                let mut asset_tree = PoseidonSparseMerkleTree::new(
                    user_state.asset_tree.nodes_db.clone(),
                    RootDataTmp::from(user_state.asset_tree.get_root().unwrap()),
                );
                let merge_process_proof = asset_tree.set(merge_key, asset_root).unwrap();
                user_state
                    .asset_tree
                    .change_root(asset_tree.get_root().unwrap())
                    .unwrap();
                let amount = user_state
                    .asset_tree
                    .find(
                        &merge_key,
                        &found_deposit_info.contract_address.0.into(),
                        &found_deposit_info.variable_index.into(),
                    )
                    .unwrap();
                assert_ne!(amount.2.value, WrappedHashOut::ZERO);

                // deposit のときは nonce が 0
                let deposit_nonce = Default::default();
                let merge_proof = MergeProof {
                    is_deposit: true,
                    diff_tree_inclusion_proof: diff_tree_inclusion_proof.clone(),
                    merge_process_proof,
                    latest_account_tree_inclusion_proof: SmtInclusionProof::with_root(
                        Default::default(),
                    ),
                    nonce: deposit_nonce,
                };
                merge_witnesses.push(merge_proof);
            }
        }

        merge_witnesses
    }

    pub fn merge_received_asset(
        &self,
        received_asset_witness: Vec<ReceivedAssetProof<F>>,
        _user_address: Address<F>,
        user_state: &mut UserState<NodeDataMemory, RootDataMemory>,
    ) -> Vec<MergeProof<F>> {
        let mut merge_witnesses = vec![];
        for witness in received_asset_witness {
            // let (index, found_deposit_info) = block
            //     .deposit_list
            //     .iter()
            //     .enumerate()
            //     .find(|(_, leaf)| leaf.receiver_address == user_address)
            //     .expect("your deposit info was not found");
            // let (_deposit_root, deposit_proof) =
            //     make_deposit_proof(&block.deposit_list, Some(index));
            // let (deposit_proof1, deposit_proof2) = deposit_proof.unwrap();

            // let pseudo_tx_hash = HashOut::ZERO;
            let tx_hash = witness.diff_tree_inclusion_proof.1.value;
            let asset_root = witness.diff_tree_inclusion_proof.2.value;

            let block_hash = get_block_hash(&witness.diff_tree_inclusion_proof.0);
            let merge_key = if witness.is_deposit {
                PoseidonHash::two_to_one(*tx_hash, block_hash).into()
            } else {
                tx_hash
            };

            // witness.assets から asset_root が計算されることを検証する.
            // ついでに user_state.asset_tree.nodes_db に contract_address 層と variable_index 層のノードを cache する
            let mut inner_asset_tree = LayeredPoseidonSparseMerkleTree::new(
                user_state.asset_tree.nodes_db.clone(),
                RootDataTmp::default(),
            );
            for asset in witness.assets {
                user_state.assets.add(asset.kind, asset.amount, merge_key);
                inner_asset_tree
                    .set(
                        asset.kind.contract_address.to_hash_out().into(),
                        asset.kind.variable_index,
                        HashOut::from_partial(&[F::from_canonical_u64(asset.amount)]).into(),
                    )
                    .unwrap();
            }

            // assert_eq!(inner_asset_tree.get_root(), asset_root);
            dbg!(inner_asset_tree.get_root().unwrap(), asset_root);

            let mut asset_tree = PoseidonSparseMerkleTree::new(
                user_state.asset_tree.nodes_db.clone(),
                RootDataTmp::from(user_state.asset_tree.get_root().unwrap()),
            );
            let merge_process_proof = asset_tree.set(merge_key, asset_root).unwrap();
            dbg!(&merge_process_proof);
            user_state
                .asset_tree
                .change_root(asset_tree.get_root().unwrap())
                .unwrap();

            let merge_proof = MergeProof {
                is_deposit: witness.is_deposit,
                diff_tree_inclusion_proof: witness.diff_tree_inclusion_proof,
                merge_process_proof,
                latest_account_tree_inclusion_proof: witness.latest_account_tree_inclusion_proof,
                nonce: witness.nonce,
            };
            merge_witnesses.push(merge_proof);
        }

        merge_witnesses
    }

    pub fn check_health(&self) {
        let resp = reqwest::blocking::Client::new()
            .get(self.aggregator_api_url(""))
            .send()
            .expect("fail to fetch");
        if resp.status() != 200 {
            panic!("{:?}", &resp);
        }

        let resp = resp
            .json::<ResponseSingleMessage>()
            .expect("fail to parse JSON");

        println!("{}", resp.message);
    }

    pub fn reset_server_state(&self) {
        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/test/reset"))
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseResetStateBody>()
            .expect("fail to parse JSON");

        if resp.ok {
            println!("reset server state successfully");
        } else {
            panic!("fail to reset server state");
        }
    }

    pub fn merge_and_purge_asset(
        &self,
        user_state: &mut UserState<NodeDataMemory, RootDataMemory>,
        user_address: Address<F>,
        diffs: &[(Address<F>, Asset<F>)],
        broadcast: bool,
    ) {
        let old_user_asset_root = user_state.asset_tree.get_root().unwrap();
        // dbg!(old_user_asset_root.to_string());

        let (blocks, latest_block_number_deposit) = self.get_blocks(
            user_address,
            Some(user_state.last_seen_block_number_deposit),
            None,
        );
        // dbg!(&blocks.len());

        let merge_witnesses_deposit = self.merge_deposits(blocks, user_address, user_state);
        dbg!(&merge_witnesses_deposit.len(), latest_block_number_deposit);

        let (merge_witnesses_received, latest_block_number_merge) = self
            .get_merge_transaction_witness(
                user_address,
                Some(user_state.last_seen_block_number_merge),
                None,
            );
        let mut merge_witnesses_received =
            self.merge_received_asset(merge_witnesses_received, user_address, user_state);
        dbg!(&merge_witnesses_received.len(), latest_block_number_merge);

        let mut merge_witnesses = merge_witnesses_deposit;
        merge_witnesses.append(&mut merge_witnesses_received);

        let _new_user_asset_root = user_state.asset_tree.get_root();
        // dbg!(new_user_asset_root.to_string());

        // 人に渡す asset から構成される tree
        let mut tx_diff_tree = LayeredLayeredPoseidonSparseMerkleTree::new(
            NodeDataMemory::default(),
            RootDataTmp::default(),
        );

        let mut purge_input_witness = vec![];
        let mut purge_output_witness = vec![];
        let mut output_asset_map = HashMap::new();
        for (receiver_address, output_asset) in diffs {
            dbg!(receiver_address.to_string(), output_asset);
            let output_witness = tx_diff_tree
                .set(
                    receiver_address.0.into(),
                    output_asset.kind.contract_address.0.into(),
                    output_asset.kind.variable_index,
                    HashOut::from_partial(&[F::from_canonical_u64(output_asset.amount)]).into(),
                )
                .unwrap();

            purge_output_witness.push(output_witness);

            // token の種類ごとに output amount を合計する
            let old_amount: u64 = output_asset_map
                .get(&output_asset.kind)
                .cloned()
                .unwrap_or_default();
            output_asset_map.insert(output_asset.kind, old_amount + output_asset.amount);
        }

        for (kind, output_amount) in output_asset_map {
            let input_assets = user_state.assets.filter(kind);

            let mut input_amount = 0;
            for asset in input_assets.0.iter() {
                input_amount += asset.1;
            }

            if output_amount > input_amount {
                panic!("output asset amount is too much");
            }

            let rest_asset = Asset {
                kind,
                amount: input_amount - output_amount,
            };

            // input (所有している分) と output (人に渡す分) の差額を自身に渡す
            let rest_witness = tx_diff_tree
                .set(
                    user_address.0.into(),
                    rest_asset.kind.contract_address.0.into(),
                    rest_asset.kind.variable_index,
                    HashOut::from_partial(&[F::from_canonical_u64(rest_asset.amount)]).into(),
                )
                .unwrap();

            purge_output_witness.push(rest_witness);

            // input に含めた asset を取り除く
            for input_asset in input_assets.0.iter() {
                let rest_amount = user_state
                    .asset_tree
                    .find(
                        &input_asset.2, // merge_key
                        &input_asset.0.contract_address.0.into(),
                        &input_asset.0.variable_index,
                    )
                    .unwrap();
                // dbg!(&rest_amount);
                assert_ne!(rest_amount.2.value, WrappedHashOut::ZERO);
                let input_witness = user_state
                    .asset_tree
                    .set(
                        input_asset.2, // merge_key
                        input_asset.0.contract_address.0.into(),
                        input_asset.0.variable_index,
                        // HashOut::from_partial(&[F::from_canonical_u64(input_asset.1)]).into(),
                        HashOut::ZERO.into(),
                    )
                    .unwrap();
                purge_input_witness.push(input_witness);
            }

            user_state.assets.remove(kind);
        }

        let nonce = WrappedHashOut::rand();

        let transaction = self.send_assets(
            user_state.account,
            &merge_witnesses,
            &purge_input_witness,
            &purge_output_witness,
            nonce,
            old_user_asset_root,
        );
        dbg!(transaction.diff_root);

        // 宛先ごとに渡す asset を整理する
        let tx_diff_tree: PoseidonSparseMerkleTree<NodeDataMemory, RootDataTmp> =
            tx_diff_tree.into();
        // key: receiver_address, value: (purge_output_inclusion_witness, assets)
        let mut assets_map: HashMap<_, (_, Vec<_>)> = HashMap::new();
        for witness in purge_output_witness.iter() {
            let receiver_address = witness.0.new_key;
            let asset = Asset {
                kind: TokenKind {
                    contract_address: Address(*witness.1.new_key),
                    variable_index: witness.2.new_key,
                },
                amount: witness.2.new_value.0.elements[0].to_canonical_u64(),
            };
            if let Some((_, assets)) = assets_map.get_mut(&receiver_address) {
                assets.push(asset);
            } else {
                let proof = tx_diff_tree.find(&receiver_address).unwrap();
                assets_map.insert(receiver_address, (proof, vec![asset]));
            }
        }

        let mut purge_output_inclusion_witnesses = vec![];
        let mut assets_list = vec![];
        for (_, (purge_output_inclusion_witness, assets)) in assets_map {
            purge_output_inclusion_witnesses.push(purge_output_inclusion_witness);
            assets_list.push(assets);
        }
        if broadcast {
            self.broadcast_transaction(
                user_address,
                transaction.tx_hash,
                nonce,
                purge_output_inclusion_witnesses,
                assets_list,
            );
        }

        user_state.insert_pending_transactions(&[transaction]);
        user_state.last_seen_block_number_deposit = latest_block_number_deposit;
        user_state.last_seen_block_number_merge = latest_block_number_merge;
    }

    /// `purge_output_inclusion_witnesses` は tx_diff_tree の receiver_address 層に関する inclusion proof
    pub fn broadcast_transaction(
        &self,
        user_address: Address<F>,
        tx_hash: WrappedHashOut<F>,
        nonce: WrappedHashOut<F>,
        purge_output_inclusion_witnesses: Vec<SmtInclusionProof<F>>,
        assets: Vec<Vec<Asset<F>>>,
    ) {
        if purge_output_inclusion_witnesses.is_empty() {
            println!("no purging transaction given");
            return;
        }

        let payload = RequestTxBroadcastBody {
            signer_address: user_address,
            tx_hash,
            nonce,
            purge_output_inclusion_witnesses,
            assets,
        };

        let body = serde_json::to_string(&payload).expect("fail to encode");

        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/tx/broadcast"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseTxBroadcastBody>()
            .expect("fail to parse JSON");

        if resp.ok {
            println!("broadcast transaction successfully");
        } else {
            panic!("fail to broadcast transaction");
        }
    }

    pub fn trigger_propose_block(&self) -> HashOut<F> {
        let body = r#"{}"#;

        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/block/propose"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseBlockProposeBody>()
            .expect("fail to parse JSON");

        *resp.new_world_state_root
    }

    pub fn trigger_approve_block(&self) -> BlockInfo<F> {
        let body = r#"{}"#;

        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/block/approve"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseBlockApproveBody>()
            .expect("fail to parse JSON");

        resp.new_block
    }

    pub fn get_blocks(
        &self,
        user_address: Address<F>,
        since: Option<u32>,
        until: Option<u32>,
    ) -> (Vec<BlockInfo<F>>, u32) {
        // let query = RequestBlockQuery {
        //     user_address,
        //     since,
        //     until,
        // };

        let mut query = vec![("user_address", format!("0x{}", user_address))];
        if let Some(since) = since {
            query.push(("since", since.to_string()));
        }

        if let Some(until) = until {
            query.push(("until", until.to_string()));
        }

        let request = reqwest::blocking::Client::new()
            .get(self.aggregator_api_url("/block"))
            .query(&query);
        let resp = request.send().expect("fail to fetch");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseBlockQuery>()
            .expect("fail to parse JSON");

        (resp.blocks, resp.latest_block_number)
    }

    pub fn get_latest_account_tree_proof(
        &self,
        user_address: Address<F>,
    ) -> (SmtInclusionProof<F>, u32) {
        // let query = RequestAccountLatestBlockQuery {
        //     user_address,
        // };

        let query = vec![("user_address", format!("0x{}", user_address))];

        let request = reqwest::blocking::Client::new()
            .get(self.aggregator_api_url("/account/latest-block"))
            .query(&query);
        let resp = request.send().expect("fail to fetch");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseAccountLatestBlockQuery>()
            .expect("fail to parse JSON");

        (resp.proof, resp.latest_block_number)
    }

    pub fn sign_to_message(
        &self,
        sender_account: Account<F>,
        message: HashOut<F>,
    ) -> SimpleSignatureProofWithPublicInputs<F, C, D> {
        let simple_signature_circuit = make_simple_signature_circuit();

        let mut pw = PartialWitness::new();
        simple_signature_circuit
            .targets
            .set_witness(&mut pw, sender_account.private_key, message);

        println!("start proving: received_signature");
        let start = Instant::now();
        let received_signature = simple_signature_circuit.prove(pw).unwrap();
        let end = start.elapsed();
        println!("prove: {}.{:03} sec", end.as_secs(), end.subsec_millis());

        received_signature
    }

    pub fn get_transaction_inclusion_witness(
        &self,
        user_address: Address<F>,
        tx_hash: WrappedHashOut<F>,
    ) -> SmtInclusionProof<F> {
        let query = vec![
            ("user_address", format!("0x{}", user_address)),
            ("tx_hash", format!("0x{}", tx_hash)),
        ];

        let request = reqwest::blocking::Client::new()
            .get(self.aggregator_api_url("/tx/witness"))
            .query(&query);
        let resp = request.send().expect("fail to fetch");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseTxWitnessQuery>()
            .expect("fail to parse JSON");

        resp.tx_inclusion_witness
    }

    pub fn send_received_signature(
        &self,
        received_signature: SimpleSignatureProofWithPublicInputs<F, C, D>,
        tx_hash: WrappedHashOut<F>,
    ) {
        let payload = RequestSignedDiffSendBody {
            tx_hash,
            received_signature,
        };

        let body = serde_json::to_string(&payload).expect("fail to encode");

        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/signed-diff/send"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseSignedDiffSendBody>()
            .expect("fail to parse JSON");

        if resp.ok {
            println!("send received signature successfully");
        } else {
            panic!("fail to send received signature");
        }
    }

    pub fn get_merge_transaction_witness(
        &self,
        user_address: Address<F>,
        since: Option<u32>,
        until: Option<u32>,
    ) -> (Vec<ReceivedAssetProof<F>>, u32) {
        let mut query = vec![("user_address", format!("0x{}", user_address))];
        if let Some(since) = since {
            query.push(("since", format!("{}", since)));
        }
        if let Some(until) = until {
            query.push(("until", format!("{}", until)));
        }

        let request = reqwest::blocking::Client::new()
            .get(self.aggregator_api_url("/tx/received"))
            .query(&query);
        let resp = request.send().expect("fail to fetch");
        if resp.status() != 200 {
            panic!("{}", resp.text().unwrap());
        }

        let resp = resp
            .json::<ResponseTxReceivedQuery>()
            .expect("fail to parse JSON");

        (resp.proofs, resp.latest_block_number)
    }
}
