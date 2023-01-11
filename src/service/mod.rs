use reqwest::Client;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use intmax_rollup_interface::{
    constants::ROLLUP_CONSTANTS,
    interface::*,
    intmax_zkp_core::{
        merkle_tree::tree::MerkleProof,
        plonky2::{
            field::types::{Field, PrimeField64},
            hash::{hash_types::HashOut, poseidon::PoseidonHash},
            iop::witness::PartialWitness,
            plonk::{
                circuit_data::CircuitConfig,
                config::{GenericConfig, Hasher, PoseidonGoldilocksConfig},
            },
        },
        rollup::{
            block::BlockInfo,
            circuits::{make_block_proof_circuit, BlockDetail},
            gadgets::deposit_block::VariableIndex,
        },
        sparse_merkle_tree::{
            gadgets::{
                process::process_smt::SmtProcessProof, verify::verify_smt::SmtInclusionProof,
            },
            goldilocks_poseidon::{
                LayeredLayeredPoseidonSparseMerkleTree, NodeDataMemory, PoseidonSparseMerkleTree,
                RootDataTmp, WrappedHashOut,
            },
            node_data::NodeData,
            root_data::RootData,
        },
        transaction::{
            asset::{Asset, ContributedAsset, ReceivedAssetProof, TokenKind},
            block_header::get_block_hash,
            circuits::{make_user_proof_circuit, MergeAndPurgeTransitionPublicInputs},
            gadgets::merge::MergeProof,
        },
        zkdsa::{
            account::{Account, Address, PublicKey},
            circuits::{make_simple_signature_circuit, SimpleSignatureProofWithPublicInputs},
        },
    },
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

pub async fn check_compatibility_with_server(service: &Config) -> anyhow::Result<()> {
    let version_info = service.check_health().await;
    match version_info {
        Ok(version_info) => {
            if version_info.name != *AGGREGATOR_NAME {
                anyhow::bail!("Given aggregator URL is invalid.");
            }

            if !version_info.version.starts_with("v0.4") {
                anyhow::bail!("Given aggregator URL is valid but is an incompatible version. If you get this error, synchronizing this CLI to the latest version may solve the problem. For more information, see https://github.com/InternetMaximalism/intmax-rollup-cli#update .");
            }
        }
        Err(_) => {
            anyhow::bail!("Given aggregator URL is invalid.");
        }
    }

    Ok(())
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

    pub async fn set_aggregator_url(&self, aggregator_url: Option<String>) -> anyhow::Result<()> {
        if let Some(new_url) = aggregator_url {
            check_compatibility_with_server(&Config::new(&new_url)).await?;

            let _ = std::mem::replace::<String>(
                &mut self.aggregator_url.lock().unwrap(),
                new_url.clone(),
            );
            println!("The new aggregator URL is {new_url} .");
        } else {
            println!(
                "The current aggregator URL is {} .",
                self.aggregator_api_url("")
            );
        }

        Ok(())
    }

    pub async fn register_account(&self, public_key: PublicKey<F>) -> Address<F> {
        let payload = RequestAccountRegisterBody {
            public_key: public_key.into(),
        };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let api_path = "/account/register";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseAccountRegisterBody>()
            .await
            .expect("fail to parse JSON");

        resp.address
    }

    /// test function
    pub async fn deposit_assets(
        &self,
        user_address: Address<F>,
        deposit_list: Vec<ContributedAsset<F>>,
    ) -> anyhow::Result<()> {
        for asset in deposit_list.iter() {
            if asset.kind.contract_address != user_address {
                anyhow::bail!("token address must be your user address");
            }
            if asset.amount == 0 || asset.amount >= 1u64 << 56 {
                anyhow::bail!("deposit amount must be a positive integer less than 2^56");
            }
        }

        let payload = RequestDepositAddBody {
            deposit_info: deposit_list
                .into_iter()
                .map(|v| v.into())
                .collect::<Vec<_>>(),
        };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let api_path = "/test/deposit/add";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseDepositAddBody>()
            .await
            .expect("fail to parse JSON");

        if resp.ok {
            println!("deposit successfully");
        } else {
            panic!("fail to deposit");
        }

        Ok(())
    }

    pub async fn send_assets(
        &self,
        account: Account<F>,
        merge_witnesses: &[MergeProof<F>],
        purge_input_witnesses: &[(SmtProcessProof<F>, SmtProcessProof<F>, SmtProcessProof<F>)],
        purge_output_witnesses: &[(SmtProcessProof<F>, SmtProcessProof<F>, SmtProcessProof<F>)],
        nonce: WrappedHashOut<F>,
        user_asset_root: WrappedHashOut<F>,
    ) -> MergeAndPurgeTransitionPublicInputs<F> {
        let user_tx_proof = {
            let config = CircuitConfig::standard_recursion_config();
            let merge_and_purge_circuit =
                make_user_proof_circuit::<F, C, D>(config, ROLLUP_CONSTANTS);

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
                Ok(()) => {}
                Err(x) => println!("{}", x),
            }

            user_tx_proof
        };

        let transaction = user_tx_proof.public_inputs.clone();
        println!("transaction hash is {}", transaction.tx_hash);

        let payload = RequestTxSendBody { user_tx_proof };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let api_path = "/tx/send";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseTxSendBody>()
            .await
            .expect("fail to parse JSON");

        assert_eq!(resp.tx_hash, transaction.tx_hash);

        transaction
    }

    pub async fn check_health(&self) -> anyhow::Result<ResponseCheckHealth> {
        let api_path = "/";
        let resp = Client::new()
            .get(self.aggregator_api_url(api_path))
            .send()
            .await?;
        if resp.status() != 200 {
            let error_message = resp.text().await?;
            anyhow::bail!("unexpected response from {api_path}: {error_message}");
        }

        let resp = resp.json::<ResponseCheckHealth>().await?;

        Ok(resp)
    }

    pub async fn sync_sent_transaction<
        D: NodeData<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>> + Clone,
        R: RootData<WrappedHashOut<F>> + Clone,
    >(
        &self,
        user_state: &mut UserState<D, R>,
        user_address: Address<F>,
    ) {
        let (mut raw_merge_witnesses, last_seen_block_number) = self
            .get_merge_transaction_witness(
                user_address,
                Some(user_state.last_seen_block_number),
                None,
            )
            .await
            .unwrap_or_else(|err| {
                dbg!(err);

                (vec![], user_state.last_seen_block_number)
            });
        let (blocks, _) = self
            .get_blocks(
                Some(user_state.last_seen_block_number),
                Some(last_seen_block_number),
            )
            .await
            .unwrap_or_else(|err| {
                dbg!(err);

                (vec![], last_seen_block_number)
            });

        // 自分が cancel した transaction に含まれる asset を自分の残高に反映させる.
        {
            let canceled_transactions = blocks
                .iter()
                .flat_map(|block| {
                    block
                        .address_list
                        .iter()
                        .zip(block.transactions.iter())
                        .filter(|(v, _)| v.sender_address == user_address && !v.is_valid)
                    // .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            // dbg!(&canceled_transactions
            //     .iter()
            //     .map(|v| v.1.to_string())
            //     .collect::<Vec<_>>());
            // dbg!(&user_state
            //     .sent_transactions
            //     .iter()
            //     .map(|v| v.0.to_string())
            //     .collect::<Vec<_>>());

            // `sent_transactions` の中で cancel した transaction であるものを列挙する.
            let recovered_assets = canceled_transactions
                .iter()
                .filter_map(|(_, tx_hash)| {
                    // dbg!(tx_hash.to_string());

                    user_state.sent_transactions.get(tx_hash)
                })
                .cloned()
                .collect::<Vec<_>>();
            for assets in recovered_assets {
                for asset in assets.0 {
                    // Assets that have already been already recovered will not be recorded again.
                    let old_amount = user_state
                        .asset_tree
                        .find(
                            &asset.2,
                            &asset.0.contract_address.to_hash_out().into(),
                            &asset.0.variable_index.to_hash_out().into(),
                        )
                        .unwrap()
                        .2
                        .value;
                    if old_amount != Default::default() {
                        continue;
                    }

                    user_state
                        .asset_tree
                        .set(
                            asset.2,
                            asset.0.contract_address.to_hash_out().into(),
                            asset.0.variable_index.to_hash_out().into(),
                            HashOut::from_partial(&[F::from_canonical_u64(asset.1)]).into(),
                        )
                        .unwrap();
                    user_state.assets.add(asset.0, asset.1, asset.2);
                }
            }

            // cancel した transaction は後で署名することもないので削除する.
            for (_, target_tx_hash) in canceled_transactions {
                user_state.sent_transactions.remove(target_tx_hash);
            }

            // proposed_block_number が last_seen_block_number 以下のものを削除する.
            user_state.sent_transactions.retain(|_, v| {
                if let Some(proposed_block_number) = v.1 {
                    proposed_block_number > last_seen_block_number
                } else {
                    true
                }
            });
        }

        user_state
            .rest_received_assets
            .append(&mut raw_merge_witnesses);
        user_state.last_seen_block_number = last_seen_block_number;
    }

    pub async fn merge_and_purge_asset<
        D: NodeData<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>> + Clone,
        R: RootData<WrappedHashOut<F>> + Clone,
    >(
        &self,
        user_state: &mut UserState<D, R>,
        user_address: Address<F>,
        purge_diffs: &[ContributedAsset<F>],
        broadcast: bool,
    ) -> anyhow::Result<()> {
        let old_user_asset_root = user_state.asset_tree.get_root().unwrap();
        // dbg!(&old_user_asset_root);

        let n_txs = 1 << ROLLUP_CONSTANTS.log_n_txs;
        let dequeued_len = n_txs.min(user_state.rest_received_assets.len());
        #[cfg(feature = "verbose")]
        dbg!(user_state.rest_received_assets.len());

        if dequeued_len == 0 && purge_diffs.is_empty() {
            anyhow::bail!("nothing to do");
        }

        let raw_merge_witnesses = user_state.rest_received_assets[0..dequeued_len].to_vec();
        let merge_witnesses = calc_merge_witnesses(user_state, raw_merge_witnesses.clone());

        // let middle_user_asset_root = user_state.asset_tree.get_root().unwrap();
        // dbg!(&middle_user_asset_root);

        // 人に渡す asset から構成される tree
        let mut tx_diff_tree = LayeredLayeredPoseidonSparseMerkleTree::new(
            NodeDataMemory::default(),
            RootDataTmp::default(),
        );

        let mut purge_input_witness = vec![];
        let mut purge_output_witness = vec![];
        let mut output_asset_map = HashMap::new();
        for output_asset in purge_diffs {
            if output_asset.amount == 0 || output_asset.amount >= 1u64 << 56 {
                anyhow::bail!("sending amount must be a positive integer less than 2^56");
            }

            // dbg!(receiver_address.to_string(), output_asset);
            let output_witness = tx_diff_tree
                .set(
                    output_asset.receiver_address.to_hash_out().into(),
                    output_asset.kind.contract_address.to_hash_out().into(),
                    output_asset.kind.variable_index.to_hash_out().into(),
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

        let mut removed_assets = vec![];
        for (kind, output_amount) in output_asset_map {
            let mut target_assets = user_state
                .assets
                .filter(kind)
                .0
                .into_iter()
                .collect::<Vec<_>>();

            // 大きい amount をもつ leaf から処理する.
            // ただし, output_amount と同じ値をもつ leaf があればそれを優先する.
            target_assets.sort_by(|a, b| {
                (a.1 == output_amount, a.1)
                    .partial_cmp(&(b.1 == output_amount, b.1))
                    .unwrap()
                    .reverse()
            });

            let mut input_assets = vec![];
            let mut input_amount = 0;
            for asset in target_assets {
                input_amount += asset.1;
                input_assets.push(asset);

                if output_amount <= input_amount {
                    break;
                }
            }

            if output_amount > input_amount {
                anyhow::bail!("output asset amount is too much");
            }

            // input (所有している分) と output (人に渡す分) の差額を自身に渡す
            if input_amount > output_amount {
                let rest_asset = Asset {
                    kind,
                    amount: input_amount - output_amount,
                };
                let rest_witness = tx_diff_tree
                    .set(
                        user_address.to_hash_out().into(),
                        rest_asset.kind.contract_address.to_hash_out().into(),
                        rest_asset.kind.variable_index.to_hash_out().into(),
                        HashOut::from_partial(&[F::from_canonical_u64(rest_asset.amount)]).into(),
                    )
                    .unwrap();

                purge_output_witness.push(rest_witness);
            }

            // input に含めた asset を取り除く
            for input_asset in input_assets.iter() {
                let rest_amount = user_state
                    .asset_tree
                    .find(
                        &input_asset.2, // merge_key
                        &input_asset.0.contract_address.to_hash_out().into(),
                        &input_asset.0.variable_index.to_hash_out().into(),
                    )
                    .unwrap();
                // dbg!(&rest_amount);
                assert_ne!(rest_amount.2.value, WrappedHashOut::ZERO);
                let input_witness = user_state
                    .asset_tree
                    .set(
                        input_asset.2, // merge_key
                        input_asset.0.contract_address.to_hash_out().into(),
                        input_asset.0.variable_index.to_hash_out().into(),
                        HashOut::ZERO.into(),
                    )
                    .unwrap();
                purge_input_witness.push(input_witness);
            }

            user_state
                .assets
                .0
                .retain(|asset| input_assets.iter().all(|t| asset != t));

            removed_assets.append(&mut input_assets);
        }

        // 必要な input_assets が多すぎる場合, 送信は失敗する.
        if purge_input_witness.len() > ROLLUP_CONSTANTS.n_diffs {
            anyhow::bail!("too many fragments of assets");
        }

        // 必要な output_assets が多すぎる場合, 送信は失敗する.
        if purge_output_witness.len() > ROLLUP_CONSTANTS.n_diffs {
            anyhow::bail!("too many destinations and token kinds");
        }

        let nonce = WrappedHashOut::rand();

        println!("WARNING: DO NOT interrupt execution of this program while a transaction is being sent.");

        let transaction = self
            .send_assets(
                user_state.account,
                &merge_witnesses,
                &purge_input_witness,
                &purge_output_witness,
                nonce,
                old_user_asset_root,
            )
            .await;
        // dbg!(transaction.diff_root);

        // send API に含めた merge transaction は削除する.
        user_state
            .rest_received_assets
            .retain(|v| !raw_merge_witnesses.iter().any(|w| v == w));

        // 宛先ごとに渡す asset を整理する
        let tx_diff_tree: PoseidonSparseMerkleTree<_, _> = tx_diff_tree.into();
        // key: receiver_address, value: (purge_output_inclusion_witness, assets)
        let mut assets_map: HashMap<_, (_, Vec<_>)> = HashMap::new();
        for witness in purge_output_witness.iter() {
            let receiver_address = witness.0.new_key;
            let asset = Asset {
                kind: TokenKind {
                    contract_address: Address(*witness.1.new_key),
                    variable_index: VariableIndex::from_hash_out(*witness.2.new_key),
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
            )
            .await;
        }

        user_state
            .sent_transactions
            .insert(transaction.tx_hash, (removed_assets, None));

        Ok(())
    }

    /// `purge_output_inclusion_witnesses` は tx_diff_tree の receiver_address 層に関する inclusion proof
    pub async fn broadcast_transaction(
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

        let api_path = "/tx/broadcast";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseTxBroadcastBody>()
            .await
            .expect("fail to parse JSON");

        if resp.ok {
            println!("broadcast transaction successfully");
        } else {
            panic!("fail to broadcast transaction");
        }
    }

    pub async fn trigger_propose_block(&self) -> HashOut<F> {
        let body = r#"{}"#;

        let api_path = "/block/propose";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseBlockProposeBody>()
            .await
            .expect("fail to parse JSON");

        *resp.new_world_state_root
    }

    pub async fn trigger_approve_block(&self) -> BlockInfo<F> {
        let body = r#"{}"#;

        let api_path = "/block/approve";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseBlockApproveBody>()
            .await
            .expect("fail to parse JSON");

        resp.new_block
    }

    pub async fn verify_block(&self, block_number: Option<u32>) -> anyhow::Result<()> {
        let latest_block = self.get_latest_block().await.unwrap();
        let block_number = block_number.unwrap_or(latest_block.header.block_number);
        println!("block number: {block_number}");
        let block_details = self.get_block_details(block_number).await.unwrap();

        let config = CircuitConfig::standard_recursion_config();
        let simple_signature_circuit = make_simple_signature_circuit(config.clone());
        let merge_and_purge_circuit = make_user_proof_circuit(config.clone(), ROLLUP_CONSTANTS);
        let block_circuit = make_block_proof_circuit::<F, C, D>(
            config,
            ROLLUP_CONSTANTS,
            &merge_and_purge_circuit,
            &simple_signature_circuit,
        );

        let nodes_db = NodeDataMemory::default();
        let mut deposit_tree =
            LayeredLayeredPoseidonSparseMerkleTree::new(nodes_db, RootDataTmp::default());
        let deposit_process_proofs = block_details
            .deposit_list
            .iter()
            .map(|leaf| {
                deposit_tree
                    .set(
                        leaf.receiver_address.0.into(),
                        leaf.contract_address.0.into(),
                        leaf.variable_index.to_hash_out().into(),
                        HashOut::from_partial(&[leaf.amount]).into(),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let inputs = BlockDetail {
            block_number: block_details.block_number,
            user_tx_proofs: block_details.user_tx_proofs,
            deposit_process_proofs,
            world_state_process_proofs: block_details.world_state_process_proofs,
            world_state_revert_proofs: block_details.world_state_revert_proofs,
            received_signature_proofs: block_details.received_signature_proofs,
            latest_account_process_proofs: block_details.latest_account_process_proofs,
            block_headers_proof_siblings: block_details.block_headers_proof_siblings,
            prev_block_header: block_details.prev_block_header,
        };
        println!("start proving: block_proof");
        let start = Instant::now();
        let block_proof = block_circuit
            .set_witness_and_prove(
                &inputs,
                &block_details.default_user_tx_proof,
                &block_details.default_simple_signature_proof,
            )
            .unwrap();
        let end = start.elapsed();
        println!("prove: {}.{:03} sec", end.as_secs(), end.subsec_millis());

        block_circuit.verify(block_proof)
    }

    /// 最新の block を取得する.
    pub async fn get_latest_block(&self) -> anyhow::Result<BlockInfo<F>> {
        // let mut query = vec![];

        let api_path = "/block/latest";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .get(self.aggregator_api_url(api_path))
            .send()
            .await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            let error_message = resp.text().await?;
            anyhow::bail!("unexpected response from {}: {}", api_path, error_message);
        }

        let resp = resp.json::<ResponseLatestBlockQuery>().await?;

        Ok(resp.block)
    }

    /// block number が since より大きく until 以下の block を可能な限り全て取得する.
    /// Returns `(blocks, until_or_latest_block_number)`
    pub async fn get_blocks(
        &self,
        since: Option<u32>,
        until: Option<u32>,
    ) -> anyhow::Result<(Vec<BlockInfo<F>>, u32)> {
        // let query = RequestBlockQuery {
        //     since,
        //     until,
        // };

        let mut query = vec![];
        if let Some(since) = since {
            query.push(("since", since.to_string()));
        }

        if let Some(until) = until {
            query.push(("until", until.to_string()));
        }

        let api_path = "/block";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let request = Client::new()
            .get(self.aggregator_api_url(api_path))
            .query(&query);
        let resp = request.send().await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            anyhow::bail!("{}", resp.text().await.unwrap());
        }

        let resp = resp.json::<ResponseBlockQuery>().await?;
        let latest_block_number = until.unwrap_or(resp.latest_block_number);

        Ok((resp.blocks, latest_block_number))
    }

    pub async fn get_block_details(&self, block_number: u32) -> anyhow::Result<BlockDetails> {
        let query = vec![("block_number", block_number.to_string())];

        let api_path = "/block/detail";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let request = Client::new()
            .get(self.aggregator_api_url(api_path))
            .query(&query);
        let resp = request.send().await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            anyhow::bail!("{}", resp.text().await.unwrap());
        }

        let resp = resp.json::<ResponseBlockDetailQuery>().await?;

        Ok(resp.block_details)
    }

    pub fn sign_to_message(
        &self,
        sender_account: Account<F>,
        message: HashOut<F>,
    ) -> SimpleSignatureProofWithPublicInputs<F, C, D> {
        let config = CircuitConfig::standard_recursion_config();
        let simple_signature_circuit = make_simple_signature_circuit(config);

        let mut pw = PartialWitness::new();
        simple_signature_circuit
            .targets
            .set_witness(&mut pw, sender_account.private_key, message);

        println!("start proving: received_signature");
        let start = Instant::now();
        let received_signature = simple_signature_circuit.prove(pw).unwrap();
        let end = start.elapsed();
        println!("prove: {}.{:03} sec", end.as_secs(), end.subsec_millis());

        match simple_signature_circuit.verify(received_signature.clone()) {
            Ok(()) => {}
            Err(x) => println!("{}", x),
        }

        received_signature
    }

    pub async fn sign_proposed_block<
        D: NodeData<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>>,
        R: RootData<WrappedHashOut<F>>,
    >(
        &self,
        user_state: &mut UserState<D, R>,
        user_address: Address<F>,
    ) {
        for (tx_hash, (_, proposed_block_number)) in user_state.sent_transactions.iter_mut() {
            if proposed_block_number.is_some() {
                continue;
            }

            let (_tx_inclusion_witness, user_asset_inclusion_witness) = self
                .get_transaction_inclusion_witness(user_address, *tx_hash)
                .await
                .unwrap();

            let latest_block = self.get_latest_block().await.unwrap();
            let proposed_world_state_root = user_asset_inclusion_witness.root;
            let received_signature =
                self.sign_to_message(user_state.account, *proposed_world_state_root);
            self.send_received_signature(received_signature, *tx_hash)
                .await;

            *proposed_block_number = Some(latest_block.header.block_number + 1);
        }
    }

    /// Returns `()`
    pub async fn get_transaction_inclusion_witness(
        &self,
        user_address: Address<F>,
        tx_hash: WrappedHashOut<F>,
    ) -> anyhow::Result<(MerkleProof<F>, SmtInclusionProof<F>)> {
        let query = vec![
            ("user_address", format!("{}", user_address)),
            ("tx_hash", format!("{}", tx_hash)),
        ];

        let api_path = "/tx/receipt";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let request = Client::new()
            .get(self.aggregator_api_url(api_path))
            .query(&query);
        let resp = request.send().await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            anyhow::bail!("{}", resp.text().await.unwrap());
        }

        let resp = resp.json::<ResponseTxReceiptQuery>().await?;

        Ok((resp.tx_inclusion_witness, resp.user_asset_inclusion_witness))
    }

    pub async fn send_received_signature(
        &self,
        received_signature: SimpleSignatureProofWithPublicInputs<F, C, D>,
        tx_hash: WrappedHashOut<F>,
    ) {
        let payload = RequestSignedDiffSendBody {
            tx_hash,
            received_signature,
        };

        let body = serde_json::to_string(&payload).expect("fail to encode");

        let api_path = "/signed-diff/send";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let resp = Client::new()
            .post(self.aggregator_api_url(api_path))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .expect("fail to post");
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            panic!("{}", resp.text().await.unwrap());
        }

        let resp = resp
            .json::<ResponseSignedDiffSendBody>()
            .await
            .expect("fail to parse JSON");

        if resp.ok {
            println!("send received signature successfully");
        } else {
            panic!("fail to send received signature");
        }
    }

    /// Returns `(raw_merge_witnesses, until_or_latest_block_number)`
    pub async fn get_merge_transaction_witness(
        &self,
        user_address: Address<F>,
        since: Option<u32>,
        until: Option<u32>,
    ) -> anyhow::Result<(Vec<ReceivedAssetProof<F>>, u32)> {
        let mut query = vec![("user_address", format!("{}", user_address))];
        if let Some(since) = since {
            query.push(("since", format!("{}", since)));
        }
        if let Some(until) = until {
            query.push(("until", format!("{}", until)));
        }

        let api_path = "/asset/received";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let request = Client::new()
            .get(self.aggregator_api_url(api_path))
            .query(&query);
        let resp = request.send().await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            anyhow::bail!("{}", resp.text().await.unwrap());
        }

        let resp = resp.json::<ResponseAssetReceivedQuery>().await?;
        let latest_block_number = until.unwrap_or(resp.latest_block_number);

        Ok((resp.proofs, latest_block_number))
    }

    pub async fn get_possession_proof(
        &self,
        user_address: Address<F>,
    ) -> anyhow::Result<SmtInclusionProof<F>> {
        let query = vec![
            ("user_address", format!("{}", user_address)),
            ("world_state_digest", format!("{}", user_address)),
        ];
        let api_path = "/account/user-asset-proof";
        #[cfg(feature = "verbose")]
        let start = {
            println!("request {api_path}");
            Instant::now()
        };
        let request = Client::new()
            .get(self.aggregator_api_url(api_path))
            .query(&query);
        let resp = request.send().await?;
        #[cfg(feature = "verbose")]
        {
            let end = start.elapsed();
            println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
        }
        if resp.status() != 200 {
            anyhow::bail!("{}", resp.text().await.unwrap());
        }

        let resp = resp.json::<ResponseUserAssetProofBody>().await?;

        Ok(resp.proof)
    }
}

pub fn calc_merge_witnesses<
    D: NodeData<WrappedHashOut<F>, WrappedHashOut<F>, WrappedHashOut<F>> + Clone,
    R: RootData<WrappedHashOut<F>> + Clone,
>(
    user_state: &mut UserState<D, R>,
    received_asset_witness: Vec<ReceivedAssetProof<F>>,
) -> Vec<MergeProof<F>> {
    let mut merge_witnesses = vec![];
    for witness in received_asset_witness {
        // let pseudo_tx_hash = HashOut::ZERO;
        let tx_hash = witness.diff_tree_inclusion_proof.1.value;
        let asset_root = witness.diff_tree_inclusion_proof.2.value;

        let block_hash = get_block_hash(&witness.diff_tree_inclusion_proof.0);
        let merge_key = if witness.is_deposit {
            PoseidonHash::two_to_one(*tx_hash, block_hash).into()
        } else {
            tx_hash
        };

        // sender が cancel した transaction は受け取ることができない.
        {
            let is_valid_confirmed_block_number =
                witness.latest_account_tree_inclusion_proof.value.to_u32()
                    == witness.diff_tree_inclusion_proof.0.block_number;
            if !witness.is_deposit && !is_valid_confirmed_block_number {
                println!("The following transaction was canceled: {}", tx_hash);
                continue;
            }
        }

        // 同じ transaction を二度 merge することはできない.
        {
            let asset_tree = PoseidonSparseMerkleTree::new(
                user_state.asset_tree.nodes_db.clone(),
                user_state.asset_tree.roots_db.clone(),
            );
            let old_asset_root_with_merge_key = asset_tree.get(&merge_key).unwrap();
            if old_asset_root_with_merge_key != Default::default() {
                println!("The following transaction has already merged: {}", tx_hash);
                continue;
            }
        }

        for asset in witness.assets {
            user_state.assets.add(asset.kind, asset.amount, merge_key);
            user_state
                .asset_tree
                .set(
                    merge_key,
                    asset.kind.contract_address.to_hash_out().into(),
                    asset.kind.variable_index.to_hash_out().into(),
                    HashOut::from_partial(&[F::from_canonical_u64(asset.amount)]).into(),
                )
                .unwrap();
        }

        // witness.assets から asset_root が計算されることを検証する.
        assert_eq!(
            user_state.asset_tree.get_asset_root(&merge_key).unwrap(),
            asset_root
        );

        let merge_process_proof = {
            // ここでのシミュレーションは `user_state.asset_tree` に反映しない.
            let mut asset_tree = PoseidonSparseMerkleTree::new(
                user_state.asset_tree.nodes_db.clone(),
                RootDataTmp::from(user_state.asset_tree.get_root().unwrap()),
            );
            let asset_root_with_merge_key = asset_tree
                .set(merge_key, Default::default())
                .unwrap()
                .old_value;

            if cfg!(debug_assertion) {
                assert_eq!(
                    *asset_root_with_merge_key,
                    PoseidonHash::two_to_one(*asset_root, *merge_key)
                );
            }

            asset_tree
                .set(merge_key, asset_root_with_merge_key)
                .unwrap()
        };
        // dbg!(&merge_process_proof);

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
