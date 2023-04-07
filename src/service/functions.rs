use std::{collections::HashMap, str::FromStr};

use intmax_rollup_interface::{
    constants::ROLLUP_CONSTANTS,
    intmax_zkp_core::{
        plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
        sparse_merkle_tree::goldilocks_poseidon::WrappedHashOut,
        transaction::asset::{ContributedAsset, TokenKind},
        zkdsa::account::Address,
    },
};

use crate::utils::{
    key_management::{memory::WalletOnMemory, types::Wallet},
    nickname::NicknameTable,
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

        service.trigger_propose_block().await;
        service.trigger_approve_block().await;
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

    service.trigger_propose_block().await;

    {
        let user_state = wallet
            .data
            .get_mut(&user_address)
            .expect("user address was not found in wallet");

        service.sign_proposed_block(user_state, user_address).await;

        wallet.backup()?;
    }

    service.trigger_approve_block().await;

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

        service.trigger_propose_block().await;
        service.trigger_approve_block().await;
    }

    let purge_diffs = distribution_list
        .into_iter()
        .filter(|v| v.receiver_address != user_address)
        .collect::<Vec<_>>();

    transfer(service, wallet, user_address, &purge_diffs).await?;

    Ok(())
}
