use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use intmax_rollup_interface::{
    constants::*,
    interface::*,
    utils::{make_deposit_proof, BlockInfo},
};
use intmax_zkp_core::{rollup, sparse_merkle_tree, transaction, zkdsa};
use plonky2::{
    field::types::PrimeField64,
    hash::hash_types::HashOut,
    iop::witness::PartialWitness,
    plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
};
use rollup::{
    circuits::merge_and_purge::{make_user_proof_circuit, MergeAndPurgeTransitionPublicInputs},
    gadgets::deposit_block::DepositInfo,
};
use serde::{Deserialize, Serialize};
use sparse_merkle_tree::{
    gadgets::{process::process_smt::SmtProcessProof, verify::verify_smt::SmtInclusionProof},
    goldilocks_poseidon::{NodeDataMemory, WrappedHashOut},
};
use transaction::gadgets::merge::MergeProof;
use zkdsa::{
    account::{Account, Address},
    circuits::{make_simple_signature_circuit, SimpleSignatureProofWithPublicInputs},
};

use crate::utils::key_management::{memory::UserState, types::TokenKind};

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
            panic!("{:?}", &resp);
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
        user_asset_root: WrappedHashOut<F>,
    ) -> MergeAndPurgeTransitionPublicInputs<F> {
        let user_tx_proof = {
            let merge_and_purge_circuit = make_user_proof_circuit::<
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
                *user_asset_root,
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

        let payload = RequestTxSendBody { user_tx_proof };
        let body = serde_json::to_string(&payload).expect("fail to encode");
        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/tx/send"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{:?}", &resp);
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
        user_state: &mut UserState<NodeDataMemory>,
    ) -> Vec<MergeProof<F>> {
        let mut merge_witnesses = vec![];
        for block in blocks {
            let (index, found_deposit_info) = block
                .deposit_list
                .iter()
                .enumerate()
                .find(|(_, leaf)| leaf.receiver_address == user_address)
                .expect("your deposit info was not found");
            let (_deposit_root, deposit_proof) =
                make_deposit_proof(&block.deposit_list, Some(index));
            let (deposit_proof1, deposit_proof2) = deposit_proof.unwrap();

            let pseudo_tx_hash = HashOut::ZERO;
            let merge_process_proof = user_state
                .asset_tree
                .set(
                    pseudo_tx_hash.into(),
                    found_deposit_info.contract_address.0.into(),
                    found_deposit_info.variable_index.into(),
                    HashOut::from_partial(&[found_deposit_info.amount]).into(),
                )
                .unwrap();

            user_state.assets.add(
                TokenKind {
                    contract_address: found_deposit_info.contract_address,
                    variable_index: found_deposit_info.variable_index.into(),
                },
                found_deposit_info.amount.to_canonical_u64(),
                pseudo_tx_hash.into(),
            );

            let (account_tree_inclusion_proof, _) =
                self.get_latest_account_tree_proof(user_address);

            let merge_proof = MergeProof {
                is_deposit: true,
                diff_tree_inclusion_proof: (block.header, deposit_proof1, deposit_proof2),
                merge_process_proof: merge_process_proof.0,
                account_tree_inclusion_proof,
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
            panic!("{:?}", &resp);
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

    pub fn trigger_propose_block(&self) -> HashOut<F> {
        let body = r#"{}"#;

        let resp = reqwest::blocking::Client::new()
            .post(self.aggregator_api_url("/block/propose"))
            .body(body)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .expect("fail to post");
        if resp.status() != 200 {
            panic!("{:?}", &resp);
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
            panic!("{:?}", &resp);
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
            panic!("{:?}", &resp);
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
            panic!("{:?}", &resp);
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

        println!("start proving: sender1_received_signature");
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
            panic!("{:?}", &resp);
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
        println!("send_received_signature");
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
            .expect("fail to post")
            .json::<ResponseSignedDiffSendBody>()
            .expect("fail to parse JSON");

        if resp.ok {
            println!("reset server state successfully");
        } else {
            panic!("fail to reset server state");
        }
    }
}
