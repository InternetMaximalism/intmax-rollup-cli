use std::collections::{BTreeMap, HashMap};

use intmax_rollup_interface::intmax_zkp_core::{
    plonky2::field::goldilocks_field::GoldilocksField, zkdsa::account::Address,
};
use serde::{Deserialize, Serialize};

type F = GoldilocksField;

#[derive(Clone, Debug, Default)]
pub struct NicknameTable {
    pub address_to_nickname: HashMap<Address<F>, String>,
    pub nickname_to_address: BTreeMap<String, Address<F>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SerializableNicknameTable(#[serde(default)] pub Vec<(Address<F>, String)>);

impl From<SerializableNicknameTable> for NicknameTable {
    fn from(value: SerializableNicknameTable) -> Self {
        let mut address_to_nickname = HashMap::new();
        let mut nickname_to_address = BTreeMap::new();
        for (address, nickname) in value.0 {
            address_to_nickname.insert(address, nickname.clone());
            nickname_to_address.insert(nickname.clone(), address);
        }

        Self {
            address_to_nickname,
            nickname_to_address,
        }
    }
}

impl<'de> Deserialize<'de> for NicknameTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = SerializableNicknameTable::deserialize(deserializer)?;

        Ok(raw.into())
    }
}

impl From<NicknameTable> for SerializableNicknameTable {
    fn from(value: NicknameTable) -> Self {
        let mut nickname_list = vec![];
        for (address, nickname) in value.address_to_nickname {
            nickname_list.push((address, nickname));
        }

        Self(nickname_list)
    }
}

impl Serialize for NicknameTable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = SerializableNicknameTable::from(self.clone());

        raw.serialize(serializer)
    }
}

impl NicknameTable {
    pub fn insert(&mut self, address: Address<F>, nickname: String) -> anyhow::Result<()> {
        let old_address = self.nickname_to_address.get(&nickname);
        if old_address.is_some() {
            anyhow::bail!("this nickname is already used");
        }

        self.nickname_to_address.insert(nickname.clone(), address);
        self.address_to_nickname.insert(address, nickname);

        Ok(())
    }

    pub fn remove(&mut self, nickname: String) -> anyhow::Result<()> {
        let old_address = self.nickname_to_address.remove(&nickname);
        if let Some(old_address) = old_address {
            self.address_to_nickname.remove(&old_address);
        } else {
            anyhow::bail!("{nickname} is not used");
        }

        Ok(())
    }
}
