use std::{
    io::{BufRead, BufReader},
    str::FromStr,
};

use intmax_rollup_interface::intmax_zkp_core::{
    plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
    rollup::gadgets::deposit_block::VariableIndex,
    transaction::asset::{ContributedAsset, TokenKind},
    zkdsa::account::Address,
};

const CSV_EXAMPLE_LINK: &str =
    "https://github.com/InternetMaximalism/intmax-rollup-cli/blob/main/tests/airdrop/README.md";
const CSV_DELIMITER: &str = r"\s*,\s*"; // コンマ区切り

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

pub fn read_distribution_from_csv(
    user_address: Address<F>,
    file: std::fs::File,
) -> anyhow::Result<Vec<ContributedAsset<F>>> {
    let mut distribution = vec![];

    let separator = regex::Regex::new(CSV_DELIMITER).unwrap();
    for (i, row) in BufReader::new(file).lines().enumerate().skip(1) {
        let row = row.unwrap();
        if row.is_empty() {
            continue;
        }

        let data = separator.split(&row).into_iter().collect::<Vec<_>>();
        if data.len() < 5 {
            anyhow::bail!(
                "Columns must be arranged in the following order from left to right: Token Address, Recipient, Fungibility, Token ID, Amount. See {CSV_EXAMPLE_LINK} for more information."
            );
        }

        let contract_address = if data[0].is_empty() {
            user_address
        } else {
            Address::from_str(data[0]).map_err(|_| {
                anyhow::anyhow!(
                    "Given file included invalid token address (row: {i}, column 0). See {CSV_EXAMPLE_LINK} for more information."
                )
            })?
        };
        let receiver_address = if data[1].is_empty() {
            user_address
        } else {
            Address::from_str(data[1]).map_err(|_| {
                anyhow::anyhow!(
                    "Given file included invalid recipient (row: {i}, column 1). See {CSV_EXAMPLE_LINK} for more information."
                )
            })?
        };
        let fungible = if data[2].is_empty() || data[2] == "FT" {
            true
        } else if data[2] == "NFT" {
            false
        } else {
            anyhow::bail!("Given file included invalid fungibility (row: {i}, column 2). See {CSV_EXAMPLE_LINK} for more information.");
        };
        let variable_index = if data[3].is_empty() {
            if fungible {
                0u8.into()
            } else {
                anyhow::bail!(
                    "NFT ID cannot be omitted (row: {i}, column 3). See {CSV_EXAMPLE_LINK} for more information."
                );
            }
        } else {
            VariableIndex::from_str(data[3]).map_err(|_| {
                anyhow::anyhow!(
                    "Given file included invalid token ID (row: {i}, column 3). See {CSV_EXAMPLE_LINK} for more information."
                )
            })?
        };
        let amount = if data[4].is_empty() {
            if fungible {
                anyhow::bail!(
                    "Fungible token amount cannot be omitted (row: {i}, column 4). See {CSV_EXAMPLE_LINK} for more information."
                );
            } else {
                1
            }
        } else {
            u64::from_str(data[4]).map_err(|_| {
                anyhow::anyhow!(
                    "Given file included invalid amount (row: {i}, column 4). See {CSV_EXAMPLE_LINK} for more information."
                )
            })?
        };
        distribution.push(ContributedAsset {
            kind: TokenKind {
                contract_address,
                variable_index,
            },
            receiver_address,
            amount,
        });
    }

    Ok(distribution)
}
