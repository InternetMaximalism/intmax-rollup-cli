use std::{sync::Arc, time::Duration};

use intmax_interoperability_plugin::{
    contracts::offer_manager::OfferManagerContractWrapper,
    contracts::offer_manager_reverse::OfferManagerReverseContractWrapper,
    ethers::{
        core::types::U256,
        prelude::{k256::ecdsa::SigningKey, SignerMiddleware},
        providers::{Http, Provider},
        signers::LocalWallet,
        types::{Bytes, H160, H256},
        utils::secret_key_to_address,
    },
};
use intmax_rollup_interface::{
    constants::{ContractConfig, POLYGON_NETWORK_CONFIG, SCROLL_NETWORK_CONFIG},
    intmax_zkp_core::{
        plonky2::{hash::hash_types::RichField, plonk::config::GenericHashOut},
        transaction::asset::TokenKind,
        zkdsa::account::Address,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NetworkName {
    ScrollAlpha,
    PolygonZkEvmTest,
}

impl std::fmt::Display for NetworkName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScrollAlpha => write!(f, "SCROLL_ALPHA"),
            Self::PolygonZkEvmTest => write!(f, "POLYGON_ZK_EVM_TEST"),
        }
    }
}

impl std::str::FromStr for NetworkName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = match s {
            // ScrollAlpha
            "SCROLL_ALPHA" => Self::ScrollAlpha,
            "scroll" => Self::ScrollAlpha,
            "scroll-alpha" => Self::ScrollAlpha,

            // PolygonZkEvmTest
            "POLYGON_ZK_EVM_TEST" => Self::PolygonZkEvmTest,
            "polygon" => Self::PolygonZkEvmTest,
            "polygon-zk-evm" => Self::PolygonZkEvmTest,

            // Error
            _ => anyhow::bail!(format!("network name {s} was not found")),
        };

        Ok(result)
    }
}

pub fn get_network_config(network_name: NetworkName) -> ContractConfig<'static> {
    match network_name {
        NetworkName::ScrollAlpha => SCROLL_NETWORK_CONFIG,
        NetworkName::PolygonZkEvmTest => POLYGON_NETWORK_CONFIG,
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MakerTransferInfo<F: RichField> {
    pub address: H160,
    pub intmax_account: Address<F>,
    pub kind: TokenKind<F>,
    pub amount: u64,
}

impl<F: RichField> MakerTransferInfo<F> {
    pub fn address(&self) -> H160 {
        self.address
    }

    pub fn intmax_account(&self) -> [u8; 32] {
        let mut address_bytes = self.intmax_account.to_hash_out().to_bytes();
        address_bytes.reverse();
        address_bytes.try_into().unwrap()
    }

    pub fn asset_id(&self) -> U256 {
        let mut buffer = self.kind.to_bytes();
        buffer.resize(32, 0);

        U256::from_little_endian(&buffer)
    }

    pub fn amount(&self) -> U256 {
        self.amount.into()
    }
}

// NOTICE: ERC20 only
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TakerTransferInfo<F: RichField> {
    pub address: H160,
    pub intmax_account: Address<F>,
    pub token_address: H160,
    pub amount: U256,
}

impl<F: RichField> TakerTransferInfo<F> {
    pub fn address(&self) -> H160 {
        self.address
    }

    pub fn intmax_account(&self) -> [u8; 32] {
        let mut address_bytes = self.intmax_account.to_hash_out().to_bytes();
        address_bytes.reverse();
        address_bytes.try_into().unwrap()
    }

    pub fn token_address(&self) -> H160 {
        self.token_address
    }

    pub fn amount(&self) -> U256 {
        self.amount
    }
}

pub async fn register_transfer<F: RichField>(
    network_config: &ContractConfig<'static>,
    secret_key: String,
    sending_transfer_info: MakerTransferInfo<F>,
    receiving_transfer_info: TakerTransferInfo<F>,
) -> U256 {
    let provider = Provider::<Http>::try_from(network_config.rpc_url)
        .unwrap()
        .interval(Duration::from_millis(10u64));
    let signer_key = SigningKey::from_bytes(&hex::decode(secret_key).unwrap()).unwrap();
    let my_account = secret_key_to_address(&signer_key);
    let wallet = LocalWallet::new_with_signer(signer_key, my_account, network_config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    let offer_manager_contract_address = network_config
        .offer_manager_contract_address
        .parse()
        .unwrap();
    let contract = OfferManagerContractWrapper::new(offer_manager_contract_address, client);

    // TODO: register also `receiving_transfer_info`
    println!("start register()");
    contract
        .register(
            sending_transfer_info.intmax_account(),
            sending_transfer_info.asset_id(),
            sending_transfer_info.amount(),
            receiving_transfer_info.address(),
            receiving_transfer_info.intmax_account(),
            receiving_transfer_info.token_address(),
            receiving_transfer_info.amount(),
        )
        .send()
        .await
        .unwrap();
    println!("end register()");

    // let logs = contract.get_register_events().await.unwrap();
    // dbg!(logs);

    let offer_id = contract.next_offer_id().await.unwrap() - U256::from(1u8);
    let is_registered = contract.is_registered(offer_id).await.unwrap();
    assert!(is_registered);

    offer_id
}

pub async fn activate_offer(
    network_config: &ContractConfig<'static>,
    secret_key: String,
    offer_id: U256,
) -> anyhow::Result<bool> {
    let provider = Provider::<Http>::try_from(network_config.rpc_url)
        .unwrap()
        .interval(Duration::from_millis(10u64));
    let signer_key = SigningKey::from_bytes(&hex::decode(secret_key).unwrap()).unwrap();
    let my_account = secret_key_to_address(&signer_key);
    let wallet = LocalWallet::new_with_signer(signer_key, my_account, network_config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    let offer_manager_contract_address = network_config
        .offer_manager_contract_address
        .parse()
        .unwrap();
    let contract = OfferManagerContractWrapper::new(offer_manager_contract_address, client);

    let is_activated: bool = contract.is_activated(offer_id).await.unwrap();
    if is_activated {
        println!("given offer ID is already activated");
        return Ok(true);
    }

    let offer_ids: Vec<U256> = vec![offer_id];
    let topic1 = offer_ids
        .iter()
        .map(|offer_id| {
            let mut bytes = [0u8; 32];
            offer_id.to_big_endian(&mut bytes);

            H256::from(bytes)
        })
        .collect::<Vec<_>>();

    let logs_register = contract.get_register_events(topic1).await.unwrap();
    debug_assert_eq!(logs_register.len(), 1);

    // send token and activate flag on scroll
    println!("start activate()");
    contract
        .activate(offer_id)
        .value(logs_register[0].taker_amount)
        .send()
        .await
        .unwrap();
    println!("end activate()");

    // let logs = contract.get_activate_events().await.unwrap();
    // dbg!(logs);

    let is_activated: bool = contract.is_activated(offer_id).await.unwrap();

    Ok(is_activated)
}

pub async fn get_offer(
    network_config: &ContractConfig<'static>,
    secret_key: String,
    offer_id: U256,
    is_reverse_offer: bool,
) -> Option<Offer> {
    let provider = Provider::<Http>::try_from(network_config.rpc_url)
        .unwrap()
        .interval(Duration::from_millis(10u64));
    let signer_key = SigningKey::from_bytes(&hex::decode(secret_key).unwrap()).unwrap();
    let my_account = secret_key_to_address(&signer_key);
    let wallet = LocalWallet::new_with_signer(signer_key, my_account, network_config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    let offer_manager_contract_address = if is_reverse_offer {
        network_config.reverse_offer_manager_contract_address
    } else {
        network_config.offer_manager_contract_address
    };
    let offer_manager_contract_address = offer_manager_contract_address.parse().unwrap();
    let contract = OfferManagerReverseContractWrapper::new(offer_manager_contract_address, client);

    let (
        maker,
        maker_intmax,
        maker_asset_id,
        maker_amount,
        taker,
        taker_intmax,
        taker_token_address,
        taker_amount,
        activated,
    ) = contract.get_offer(offer_id).await.unwrap();

    if !is_reverse_offer && maker == H160::default() {
        return None;
    }

    if is_reverse_offer && taker == H160::default() {
        return None;
    }

    Some(Offer {
        maker,
        maker_intmax,
        maker_asset_id,
        maker_amount,
        taker,
        taker_intmax,
        taker_token_address,
        taker_amount,
        activated,
    })
}

pub async fn lock_offer<F: RichField>(
    network_config: &ContractConfig<'static>,
    secret_key: String,
    sending_transfer_info: TakerTransferInfo<F>,
    receiving_transfer_info: MakerTransferInfo<F>,
) -> U256 {
    let provider = Provider::<Http>::try_from(network_config.rpc_url)
        .unwrap()
        .interval(Duration::from_millis(10u64));
    let signer_key = SigningKey::from_bytes(&hex::decode(secret_key).unwrap()).unwrap();
    let my_account = secret_key_to_address(&signer_key);
    let wallet = LocalWallet::new_with_signer(signer_key, my_account, network_config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    let reverse_offer_manager_contract_address = network_config
        .reverse_offer_manager_contract_address
        .parse()
        .unwrap();
    let contract =
        OfferManagerReverseContractWrapper::new(reverse_offer_manager_contract_address, client);

    // TODO: register also `receiving_transfer_info`
    println!("start register()");
    let taker_intmax_address = sending_transfer_info.intmax_account();
    let taker_token_address = H160::default(); // ETH
    let taker_amount = sending_transfer_info.amount();
    let maker = receiving_transfer_info.address();
    // let maker_intmax_address = receiving_transfer_info.intmax_account();
    let maker_asset_id = receiving_transfer_info.asset_id();
    let maker_amount = receiving_transfer_info.amount();
    // dbg!(
    //     taker_intmax_address,
    //     taker_token_address,
    //     taker_amount,
    //     maker,
    //     maker_asset_id,
    //     maker_amount,
    // );
    contract
        .register(
            taker_intmax_address,
            taker_token_address,
            taker_amount,
            maker,
            maker_asset_id,
            maker_amount,
        )
        .value(sending_transfer_info.amount())
        .send()
        .await
        .unwrap();
    println!("end register()");

    // let logs = contract.get_register_events().await.unwrap();
    // dbg!(logs);

    let offer_id = contract.next_offer_id().await.unwrap() - U256::from(1u8);
    let is_locked = contract.is_registered(offer_id).await.unwrap();
    assert!(is_locked);

    offer_id
}

pub async fn unlock_offer(
    network_config: &ContractConfig<'static>,
    secret_key: String,
    offer_id: U256,
    witness: Bytes,
) -> anyhow::Result<bool> {
    let provider =
        Provider::<Http>::try_from(network_config.rpc_url)?.interval(Duration::from_millis(10u64));
    let signer_key = SigningKey::from_bytes(&hex::decode(secret_key.clone()).unwrap()).unwrap();
    let my_account = secret_key_to_address(&signer_key);
    let wallet = LocalWallet::new_with_signer(signer_key, my_account, network_config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    let reverse_offer_manager_contract_address = network_config
        .reverse_offer_manager_contract_address
        .parse()
        .unwrap();
    let contract =
        OfferManagerReverseContractWrapper::new(reverse_offer_manager_contract_address, client);

    let offer = get_offer(network_config, secret_key, offer_id, true).await;
    if offer.is_none() {
        anyhow::bail!("given offer ID is not registered");
    }

    let offer = offer.unwrap();
    if offer.activated {
        println!("given offer ID is already unlocked");
        return Ok(true);
    }

    // send token and activate flag on scroll
    println!("start activate()");
    // dbg!(offer_id, &witness);
    contract.check_witness(offer_id, witness).send().await?;
    println!("end activate()");

    // let logs = contract.get_activate_events().await.unwrap();
    // dbg!(logs);

    let is_unlocked: bool = contract.is_activated(offer_id).await?;

    Ok(is_unlocked)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub maker: H160,
    pub maker_intmax: [u8; 32],
    pub maker_asset_id: U256,
    pub maker_amount: U256,
    pub taker: H160,
    pub taker_intmax: [u8; 32],
    pub taker_token_address: H160,
    pub taker_amount: U256,
    pub activated: bool,
}

// #[cfg(test)]
// mod tests {
//     use std::{sync::Arc, time::Duration};

//     use dotenv::dotenv;
//     use intmax_interoperability_plugin::{
//         ethers::{
//             contract::abigen,
//             core::types::{Address, U256},
//             prelude::{k256::ecdsa::SigningKey, SignerMiddleware},
//             providers::{Http, Provider},
//             signers::LocalWallet,
//             types::Filter,
//             utils::secret_key_to_address,
//         },
//         OfferManagerContractWrapper,
//     };

//     use super::*;

//     #[tokio::test]
//     async fn test_register_transfer() {
//         let _ = dotenv().ok();
//         let secret_key =
//             std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env file");
//         let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set in .env file");
//         let chain_id: u64 = std::env::var("CHAIN_ID")
//             .expect("CHAIN_ID must be set in .env file")
//             .parse()
//             .unwrap();
//         let contract_address: Address = std::env::var("CONTRACT_ADDRESS")
//             .expect("CONTRACT_ADDRESS must be set in .env file")
//             .parse()
//             .unwrap();

//         register_transfer(rpc_url, chain_id, contract_address, secret_key);
//     }
// }
