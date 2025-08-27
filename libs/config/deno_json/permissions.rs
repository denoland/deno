// Copyright 2018-2025 the Deno authors. MIT license.

use indexmap::IndexMap;
use serde::Deserialize;
use url::Url;

use super::UndefinedPermissionError;

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PermissionConfigValue {
  All,
  Some(Vec<String>),
  None,
  #[default]
  NotPresent,
}

impl<'de> serde::Deserialize<'de> for PermissionConfigValue {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct Visitor;
    impl<'d> serde::de::Visitor<'d> for Visitor {
      type Value = PermissionConfigValue;

      fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "either an array or bool")
      }

      fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        if v {
          Ok(PermissionConfigValue::All)
        } else {
          Ok(PermissionConfigValue::None)
        }
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'d>,
      {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(8));
        while let Some(element) = seq.next_element::<String>()? {
          out.push(element);
        }

        if out.is_empty() {
          Ok(PermissionConfigValue::None)
        } else {
          Ok(PermissionConfigValue::Some(out))
        }
      }

      fn visit_none<E>(self) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(PermissionConfigValue::None)
      }

      fn visit_unit<E>(self) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(PermissionConfigValue::None)
      }
    }
    deserializer.deserialize_any(Visitor)
  }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(default, deny_unknown_fields)]
pub struct AllowDenyPermissionConfig {
  pub allow: PermissionConfigValue,
  pub deny: PermissionConfigValue,
}

impl PermissionConfigValue {
  pub fn merge(self, other: Self) -> Self {
    match (self, other) {
      (PermissionConfigValue::NotPresent, other) => other,
      (this, PermissionConfigValue::NotPresent) => this,
      (_, other) => other,
    }
  }
}

impl AllowDenyPermissionConfig {
  pub fn merge(self, other: Self) -> Self {
    AllowDenyPermissionConfig {
      allow: self.allow.merge(other.allow),
      deny: self.deny.merge(other.deny),
    }
  }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum AllowDenyPermissionConfigValue {
  Boolean(bool),
  AllowList(Vec<String>),
  Object(AllowDenyPermissionConfig),
}

fn deserialize_allow_deny<'de, D: serde::Deserializer<'de>>(
  de: D,
) -> Result<AllowDenyPermissionConfig, D::Error> {
  AllowDenyPermissionConfigValue::deserialize(de).map(|value| match value {
    AllowDenyPermissionConfigValue::Boolean(b) => {
      if b {
        AllowDenyPermissionConfig {
          allow: PermissionConfigValue::All,
          deny: PermissionConfigValue::None,
        }
      } else {
        AllowDenyPermissionConfig {
          allow: PermissionConfigValue::None,
          deny: PermissionConfigValue::None,
        }
      }
    }
    AllowDenyPermissionConfigValue::AllowList(allow) => {
      AllowDenyPermissionConfig {
        allow: PermissionConfigValue::Some(allow),
        deny: PermissionConfigValue::None,
      }
    }
    AllowDenyPermissionConfigValue::Object(allow_deny) => allow_deny,
  })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(untagged)]
pub enum PermissionNameOrObject {
  Name(String),
  Object(Box<PermissionsObject>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PermissionsObjectWithBase {
  // TODO(dsherret): make this an Arc<Url>
  pub base: Url,
  pub permissions: PermissionsObject,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Default, Hash)]
#[serde(default, deny_unknown_fields)]
pub struct PermissionsObject {
  #[serde(default)]
  pub all: Option<bool>,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub read: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub write: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub import: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub env: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub net: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub run: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub ffi: AllowDenyPermissionConfig,
  #[serde(default, deserialize_with = "deserialize_allow_deny")]
  pub sys: AllowDenyPermissionConfig,
}

#[derive(Clone, Debug, Default)]
pub struct PermissionsConfig {
  pub sets: IndexMap<String, PermissionsObjectWithBase>,
}

impl PermissionsConfig {
  pub fn parse(
    value: serde_json::Value,
    base: &Url,
  ) -> Result<Self, serde_json::Error> {
    let sets: IndexMap<String, PermissionsObject> =
      serde_json::from_value(value)?;
    Ok(Self {
      sets: sets
        .into_iter()
        .map(|(k, permissions)| {
          (
            k,
            PermissionsObjectWithBase {
              base: base.clone(),
              permissions,
            },
          )
        })
        .collect(),
    })
  }

  pub fn get(
    &self,
    name: &str,
  ) -> Result<&PermissionsObjectWithBase, UndefinedPermissionError> {
    match self.sets.get(name) {
      Some(value) => Ok(value),
      None => Err(UndefinedPermissionError(name.to_string())),
    }
  }

  pub fn merge(self, member: Self) -> Self {
    let mut sets = self.sets;

    for (key, value) in member.sets {
      sets.insert(key, value);
    }

    Self { sets }
  }
}
