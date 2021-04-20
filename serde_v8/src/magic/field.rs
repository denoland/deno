use crate::error::{Error, Result};
use serde::ser::{Impossible, Serialize, Serializer};

/// All serde_v8 "magic" values are reduced to structs with 1 or 2 u64 fields
/// assuming usize==u64, most types are simply a pointer or pointer+len (e.g: Box<T>)
pub type TransmutedField = u64;
pub type FieldResult = Result<TransmutedField>;

macro_rules! not_reachable {
    ($($name:ident($ty:ty);)*) => {
        $(fn $name(self, _v: $ty) -> FieldResult {
            unreachable!();
        })*
    };
}

/// FieldSerializer is a simple serde::Serializer that only returns u64s
/// it allows the "magic" struct serializers to obtain the transmuted field values
pub struct FieldSerializer {}

impl Serializer for FieldSerializer {
  type Ok = TransmutedField;
  type Error = Error;

  type SerializeSeq = Impossible<TransmutedField, Error>;
  type SerializeTuple = Impossible<TransmutedField, Error>;
  type SerializeTupleStruct = Impossible<TransmutedField, Error>;
  type SerializeTupleVariant = Impossible<TransmutedField, Error>;
  type SerializeMap = Impossible<TransmutedField, Error>;
  type SerializeStruct = Impossible<TransmutedField, Error>;
  type SerializeStructVariant = Impossible<TransmutedField, Error>;

  fn serialize_u64(self, transmuted_field: u64) -> FieldResult {
    Ok(transmuted_field)
  }

  not_reachable! {
      serialize_i8(i8);
      serialize_i16(i16);
      serialize_i32(i32);
      serialize_i64(i64);
      serialize_u8(u8);
      serialize_u16(u16);
      serialize_u32(u32);
      // serialize_u64(TransmutedField); the chosen one
      serialize_f32(f32);
      serialize_f64(f64);
      serialize_bool(bool);
      serialize_char(char);
      serialize_str(&str);
      serialize_bytes(&[u8]);
  }

  fn serialize_none(self) -> FieldResult {
    unreachable!();
  }

  fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> FieldResult {
    unreachable!();
  }

  fn serialize_unit(self) -> FieldResult {
    unreachable!();
  }

  fn serialize_unit_struct(self, _name: &'static str) -> FieldResult {
    unreachable!();
  }

  fn serialize_unit_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
  ) -> FieldResult {
    unreachable!();
  }

  fn serialize_newtype_struct<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _value: &T,
  ) -> FieldResult {
    unreachable!();
  }

  fn serialize_newtype_variant<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
    _value: &T,
  ) -> FieldResult {
    unreachable!();
  }
  fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
    unreachable!();
  }

  fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
    unreachable!();
  }

  fn serialize_tuple_struct(
    self,
    _name: &'static str,
    _len: usize,
  ) -> Result<Self::SerializeTupleStruct> {
    unreachable!();
  }

  fn serialize_tuple_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
    _len: usize,
  ) -> Result<Self::SerializeTupleVariant> {
    unreachable!();
  }

  fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
    unreachable!();
  }

  fn serialize_struct(
    self,
    _name: &'static str,
    _len: usize,
  ) -> Result<Self::SerializeStruct> {
    unreachable!();
  }

  fn serialize_struct_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
    _len: usize,
  ) -> Result<Self::SerializeStructVariant> {
    unreachable!();
  }
}
