use rusty_v8 as v8;
use serde;

use std::fmt;
use std::marker::PhantomData;

pub const FIELD: &'static str = "$__v8_magic_value";
pub const NAME: &'static str = "$__v8_magic_Value";

pub struct Value<'s> {
    pub v8_value: v8::Local<'s, v8::Value>
}

impl<'s> From<v8::Local<'s, v8::Value>> for Value<'s> {
    fn from(v8_value: v8::Local<'s, v8::Value>) -> Self {
        Self { v8_value }
    }
}

impl<'s> From<Value<'s>> for v8::Local<'s, v8::Value> {
    fn from(v: Value<'s>) -> Self {
        v.v8_value
    }
}

impl serde::Serialize for Value<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut s = serializer.serialize_struct(NAME, 1)?;
        let mv = Value { v8_value: self.v8_value };
        let hack: u64 = unsafe { std::mem::transmute(mv) };
        s.serialize_field(FIELD, &hack)?;
        s.end()
    }
}

impl<'de, 's> serde::Deserialize<'de> for Value<'s> {
    fn deserialize<D>(deserializer: D) -> Result<Value<'s>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValueVisitor<'s> {
            p1: PhantomData<&'s ()>,
        }

        impl<'de, 's> serde::de::Visitor<'de> for ValueVisitor<'s> {
            type Value = Value<'s>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a v8::Value")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mv: Value<'s> = unsafe { std::mem::transmute(v) };
                Ok(mv)
            }
        }

        static FIELDS: [&str; 1] = [FIELD];
        let visitor = ValueVisitor { p1: PhantomData };
        deserializer.deserialize_struct(NAME, &FIELDS, visitor)
    }
}
