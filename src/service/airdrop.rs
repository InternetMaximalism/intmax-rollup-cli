use std::{
    io::{BufRead, BufReader},
    str::FromStr,
};

use intmax_zkp_core::{
    rollup::gadgets::deposit_block::VariableIndex,
    transaction::asset::{ContributedAsset, TokenKind},
    zkdsa::account::Address,
};
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

const CSV_EXAMPLE_LINK: &str =
    "https://github.com/InternetMaximalism/intmax-rollup-cli/blob/main/tests/airdrop/example.csv";
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
        if data.len() != 3 {
            anyhow::bail!(
                r#"Columns must be arranged in the following order from left to right: Token ID, Recipient, Amount.
See {CSV_EXAMPLE_LINK} for more information."#
            );
        }
        distribution.push(ContributedAsset {
            kind: TokenKind {
                contract_address: user_address,
                variable_index: VariableIndex::from_str(data[0]).map_err(|_| {
                    anyhow::anyhow!(
                        r#"Given file included invalid token ID (row: {i}, column 0).
See {CSV_EXAMPLE_LINK} for more information."#
                    )
                })?,
            },
            receiver_address: Address::from_str(data[1]).map_err(|_| {
                anyhow::anyhow!(
                    r#"Given file included invalid recipient (row: {i}, column 1).
See {CSV_EXAMPLE_LINK} for more information."#
                )
            })?,
            amount: u64::from_str(data[2]).map_err(|_| {
                anyhow::anyhow!(
                    r#"Given file included invalid amount (row: {i}, column 2).
See {CSV_EXAMPLE_LINK} for more information."#
                )
            })?,
        });
    }

    Ok(distribution)
}
