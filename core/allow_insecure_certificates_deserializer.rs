// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::fmt::{self, Formatter};
use std::marker::PhantomData;

use serde::{de, Deserialize, Deserializer};

pub fn deserialize_allow_insecure_certificates<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
  D: Deserializer<'de>,
{
  struct OptStringOrBoolOrVec(PhantomData<Option<Vec<String>>>);

  impl<'de> de::Visitor<'de> for OptStringOrBoolOrVec {
    type Value = Option<Vec<String>>;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
      formatter.write_str("a string or sequence of strings")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Some(vec![v.to_owned()]))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Some(vec![v]))
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(v.then(Vec::new))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
      D: Deserializer<'de>,
    {
      deserializer.deserialize_any(OptStringOrBoolOrVec(PhantomData))
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
      A: de::SeqAccess<'de>,
    {
      if let Some(size) = seq.size_hint() {
        if size == 0 {
          return Ok(None);
        }
      }

      Vec::<String>::deserialize(de::value::SeqAccessDeserializer::new(seq))
        .map(|v| if v.is_empty() { None } else { Some(v) })
    }
  }

  deserializer.deserialize_option(OptStringOrBoolOrVec(PhantomData))
}
