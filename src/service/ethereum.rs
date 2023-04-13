use intmax_interoperability_plugin::ethers::types::U256;
use reqwest::header::{HeaderMap, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    from = "SerializableGasStationInfo",
    into = "SerializableGasStationInfo"
)]
pub struct GasStationInfo {
    /// gas prices in GWei
    pub safe_low: U256,
    /// gas prices in GWei
    pub standard: U256,
    /// gas prices in GWei
    pub fast: U256,
    /// gas prices in GWei
    pub fastest: U256,
    /// in seconds, gives average block time of the network
    pub block_time: U256,
    /// provides the information of latest block mined when recommendation was made
    pub block_number: U256,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct SerializableGasStationInfo {
    #[serde(rename = "safeLow")]
    safe_low: f64,
    standard: f64,
    fast: f64,
    fastest: f64,
    #[serde(rename = "blockTime")]
    block_time: u64,
    #[serde(rename = "blockNumber")]
    block_number: u64,
}

pub fn wei_to_gwei(value: U256) -> f64 {
    value.as_u128() as f64 / 1e9
}

impl From<GasStationInfo> for SerializableGasStationInfo {
    fn from(value: GasStationInfo) -> Self {
        Self {
            safe_low: wei_to_gwei(value.safe_low),
            standard: wei_to_gwei(value.standard),
            fast: wei_to_gwei(value.fast),
            fastest: wei_to_gwei(value.fastest),
            block_time: value.block_time.as_u64(),
            block_number: value.block_number.as_u64(),
        }
    }
}

pub fn gwei_to_wei(value: f64) -> U256 {
    ((value * 1e9).floor() as u128).into()
}

impl From<SerializableGasStationInfo> for GasStationInfo {
    fn from(value: SerializableGasStationInfo) -> Self {
        Self {
            safe_low: gwei_to_wei(value.safe_low),
            standard: gwei_to_wei(value.standard),
            fast: gwei_to_wei(value.fast),
            fastest: gwei_to_wei(value.fastest),
            block_time: value.block_time.into(),
            block_number: value.block_number.into(),
        }
    }
}

#[test]
fn test_serde_gas_station() {
    let gas_station_info = GasStationInfo {
        safe_low: 3730000000u64.into(),
        standard: 3730000000u64.into(),
        fast: 3730000000u64.into(),
        fastest: 3730000000u64.into(),
        block_time: 0u64.into(),
        block_number: 109880u64.into(),
    };
    // let encoded_gas_station_info = r#"{"safeLow":3.73,"standard":3.73,"fast":3.73,"fastest":3.73,"blockTime":0,"blockNumber":109880}"#;
    let encoded_gas_station_info = serde_json::to_string(&gas_station_info).unwrap();
    let decoded_gas_station_info: GasStationInfo =
        serde_json::from_str(&encoded_gas_station_info).unwrap();
    dbg!(decoded_gas_station_info);
}

pub async fn fetch_polygon_zkevm_test_gas_price() -> anyhow::Result<GasStationInfo> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    let client = reqwest::Client::builder()
        .user_agent("curl/7.86.0")
        .build()?;
    let resp = client
        .get("https://gasstation.polygon.technology/zkevm")
        .send()
        .await?;
    #[cfg(feature = "verbose")]
    {
        let end = start.elapsed();
        println!("respond: {}.{:03} sec", end.as_secs(), end.subsec_millis());
    }
    if resp.status() != 200 {
        anyhow::bail!("{}", resp.text().await.unwrap());
    }

    let resp = resp.json::<GasStationInfo>().await?;

    Ok(resp)
}

#[tokio::test]
async fn test_fetch_gas_price() {
    let _resp = fetch_polygon_zkevm_test_gas_price().await.unwrap();
}
