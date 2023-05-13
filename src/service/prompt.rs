use dialoguer::{theme::ColorfulTheme, Select};
use intmax_interoperability_plugin::ethers::types::H160;
use intmax_rollup_interface::constants::ContractConfig;

use super::interoperability::{get_token_allow_list, get_token_metadata, TokenMetadata};

pub async fn select_payment_method(
    network_config: &ContractConfig<'static>,
    is_reverse_offer: bool,
) -> anyhow::Result<Option<TokenMetadata>> {
    let allow_list = get_token_allow_list(network_config, is_reverse_offer).await?;

    let mut allow_list_with_metadata = vec![];

    for token_address in allow_list {
        let metadata = get_token_metadata(network_config, token_address).await?;
        allow_list_with_metadata.push(metadata);
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which token is the payment method?")
        .items(
            &allow_list_with_metadata
                .iter()
                .map(|v| {
                    if v.address == H160::zero() {
                        v.symbol.to_string()
                    } else {
                        format!("{} ({:?})", v.symbol, v.address)
                    }
                })
                .collect::<Vec<_>>(),
        )
        .default(0)
        .interact_opt()?;

    Ok(selection.map(|index| allow_list_with_metadata[index].clone()))
}
