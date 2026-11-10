//! Compact in-memory identities; saved meters retain their original string keys.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum InstanceKey {
    Dot(u64, u32, u32),
    Named(String),
}

impl From<String> for InstanceKey {
    fn from(text: String) -> Self {
        let mut parts = text.split(':');
        let parsed = (|| {
            let parent = parts.next()?.parse().ok()?;
            let started = parts.next()?.parse().ok()?;
            let power = parts.next()?.parse().ok()?;
            if parts.next().is_some() || text != format!("{parent}:{started}:{power}") {
                return None;
            }
            Some(Self::Dot(parent, started, power))
        })();
        parsed.unwrap_or(Self::Named(text))
    }
}

impl Serialize for InstanceKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Dot(parent, started, power) => {
                serializer.serialize_str(&format!("{parent}:{started}:{power}"))
            }
            Self::Named(text) => serializer.serialize_str(text),
        }
    }
}

impl<'de> Deserialize<'de> for InstanceKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(String::deserialize(deserializer)?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn numeric_instances_keep_legacy_json_and_toml_save_formats() {
        #[derive(Serialize, Deserialize)]
        struct Fixture {
            instances: BTreeMap<InstanceKey, u32>,
        }
        let old: BTreeMap<String, u32> = [
            "aura",
            "control",
            "received",
            "1:2:3",
            "01:2:3",
            "18446744073709551615:4294967295:4294967295",
            "invalid:1:2:3",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, key)| (key.to_owned(), i as u32))
        .collect();
        let json = serde_json::to_string(&old).unwrap();
        let parsed: BTreeMap<InstanceKey, u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.get(&InstanceKey::Dot(1, 2, 3)), old.get("1:2:3"));
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            serde_json::to_value(&old).unwrap()
        );
        let fixture = Fixture { instances: parsed };
        let toml = toml::to_string(&fixture).unwrap();
        let restored: Fixture = toml::from_str(&toml).unwrap();
        assert_eq!(restored.instances, fixture.instances);
    }
}
