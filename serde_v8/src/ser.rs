// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use rusty_v8 as v8;
use serde::ser;
use serde::ser::{Impossible, Serialize};

use std::cell::RefCell;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::keys::v8_struct_key;
use crate::magic;

type JsValue<'s> = v8::Local<'s, v8::Value>;
type JsResult<'s> = Result<JsValue<'s>>;

type ScopePtr<'a, 'b> = Rc<RefCell<v8::EscapableHandleScope<'a, 'b>>>;

pub fn to_v8<'a, T>(scope: &mut v8::HandleScope<'a>, input: T) -> JsResult<'a>
where
  T: Serialize,
{
  let subscope = v8::EscapableHandleScope::new(scope);
  let scopeptr = Rc::new(RefCell::new(subscope));
  let serializer = Serializer::new(scopeptr.clone());
  let x = input.serialize(serializer)?;
  let x = scopeptr.clone().borrow_mut().escape(x);

  Ok(x)
}

/// Wraps other serializers into an enum tagged variant form.
/// Uses {"Variant": ...payload...} for compatibility with serde-json.
pub struct VariantSerializer<'a, 'b, S> {
  variant: &'static str,
  inner: S,
  scope: ScopePtr<'a, 'b>,
}

impl<'a, 'b, S> VariantSerializer<'a, 'b, S> {
  pub fn new(scope: ScopePtr<'a, 'b>, variant: &'static str, inner: S) -> Self {
    Self {
      scope,
      variant,
      inner,
    }
  }

  fn end(self, inner: impl FnOnce(S) -> JsResult<'a>) -> JsResult<'a> {
    let value = inner(self.inner)?;
    let scope = &mut *self.scope.borrow_mut();
    let obj = v8::Object::new(scope);
    let key = v8_struct_key(scope, self.variant).into();
    obj.set(scope, key, value);
    Ok(obj.into())
  }
}

impl<'a, 'b, S> ser::SerializeTupleVariant for VariantSerializer<'a, 'b, S>
where
  S: ser::SerializeTupleStruct<Ok = JsValue<'a>, Error = Error>,
{
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    value: &T,
  ) -> Result<()> {
    self.inner.serialize_field(value)
  }

  fn end(self) -> JsResult<'a> {
    self.end(S::end)
  }
}

impl<'a, 'b, S> ser::SerializeStructVariant for VariantSerializer<'a, 'b, S>
where
  S: ser::SerializeStruct<Ok = JsValue<'a>, Error = Error>,
{
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    key: &'static str,
    value: &T,
  ) -> Result<()> {
    self.inner.serialize_field(key, value)
  }

  fn end(self) -> JsResult<'a> {
    self.end(S::end)
  }
}

pub struct ArraySerializer<'a, 'b> {
  // serializer: Serializer<'a, 'b>,
  pending: Vec<JsValue<'a>>,
  scope: ScopePtr<'a, 'b>,
}

impl<'a, 'b> ArraySerializer<'a, 'b> {
  pub fn new(scope: ScopePtr<'a, 'b>, len: Option<usize>) -> Self {
    let pending = match len {
      Some(len) => Vec::with_capacity(len),
      None => vec![],
    };
    Self {
      scope,
      pending,
    }
  }
}

impl<'a, 'b> ser::SerializeSeq for ArraySerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_element<T: ?Sized + Serialize>(
    &mut self,
    value: &T,
  ) -> Result<()> {
    let x = value.serialize(Serializer::new(self.scope.clone()))?;
    self.pending.push(x);
    Ok(())
  }

  fn end(self) -> JsResult<'a> {
    let elements = self.pending.iter().as_slice();
    let scope = &mut *self.scope.borrow_mut();
    let arr = v8::Array::new_with_elements(scope, elements);
    Ok(arr.into())
  }
}

impl<'a, 'b> ser::SerializeTuple for ArraySerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_element<T: ?Sized + Serialize>(
    &mut self,
    value: &T,
  ) -> Result<()> {
    ser::SerializeSeq::serialize_element(self, value)
  }

  fn end(self) -> JsResult<'a> {
    ser::SerializeSeq::end(self)
  }
}

impl<'a, 'b> ser::SerializeTupleStruct for ArraySerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    value: &T,
  ) -> Result<()> {
    ser::SerializeTuple::serialize_element(self, value)
  }

  fn end(self) -> JsResult<'a> {
    ser::SerializeTuple::end(self)
  }
}

pub struct ObjectSerializer<'a, 'b> {
  scope: ScopePtr<'a, 'b>,
  obj: v8::Local<'a, v8::Object>,
}

impl<'a, 'b> ObjectSerializer<'a, 'b> {
  pub fn new(scope: ScopePtr<'a, 'b>) -> Self {
    let obj = v8::Object::new(&mut *scope.borrow_mut());
    Self { scope, obj }
  }
}

impl<'a, 'b> ser::SerializeStruct for ObjectSerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    key: &'static str,
    value: &T,
  ) -> Result<()> {
    let value = value.serialize(Serializer::new(self.scope.clone()))?;
    let scope = &mut *self.scope.borrow_mut();
    let key = v8_struct_key(scope, key).into();
    self.obj.set(scope, key, value);
    Ok(())
  }

  fn end(self) -> JsResult<'a> {
    Ok(self.obj.into())
  }
}

pub struct MagicSerializer<'a, 'b> {
  scope: ScopePtr<'a, 'b>,
  v8_value: Option<v8::Local<'a, v8::Value>>,
}

impl<'a, 'b> ser::SerializeStruct for MagicSerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    key: &'static str,
    value: &T,
  ) -> Result<()> {
    if key != magic::FIELD {
      unreachable!();
    }
    let v8_value = value.serialize(MagicTransmuter {
      _scope: self.scope.clone(),
    })?;
    self.v8_value = Some(v8_value);
    Ok(())
  }

  fn end(self) -> JsResult<'a> {
    Ok(self.v8_value.unwrap())
  }
}

// Dispatches between magic and regular struct serializers
pub enum StructSerializers<'a, 'b> {
  Magic(MagicSerializer<'a, 'b>),
  Regular(ObjectSerializer<'a, 'b>),
}

impl<'a, 'b> ser::SerializeStruct for StructSerializers<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_field<T: ?Sized + Serialize>(
    &mut self,
    key: &'static str,
    value: &T,
  ) -> Result<()> {
    match self {
      StructSerializers::Magic(s) => s.serialize_field(key, value),
      StructSerializers::Regular(s) => s.serialize_field(key, value),
    }
  }

  fn end(self) -> JsResult<'a> {
    match self {
      StructSerializers::Magic(s) => s.end(),
      StructSerializers::Regular(s) => s.end(),
    }
  }
}

// Serializes to JS Objects, NOT JS Maps ...
pub struct MapSerializer<'a, 'b> {
  scope: ScopePtr<'a, 'b>,
  obj: v8::Local<'a, v8::Object>,
  next_key: Option<JsValue<'a>>,
}

impl<'a, 'b> MapSerializer<'a, 'b> {
  pub fn new(scope: ScopePtr<'a, 'b>) -> Self {
    let obj = v8::Object::new(&mut *scope.borrow_mut());
    Self {
      scope,
      obj,
      next_key: None,
    }
  }
}

impl<'a, 'b> ser::SerializeMap for MapSerializer<'a, 'b> {
  type Ok = JsValue<'a>;
  type Error = Error;

  fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
    debug_assert!(self.next_key.is_none());
    self.next_key = Some(key.serialize(Serializer::new(self.scope.clone()))?);
    Ok(())
  }

  fn serialize_value<T: ?Sized + Serialize>(
    &mut self,
    value: &T,
  ) -> Result<()> {
    let v8_value = value.serialize(Serializer::new(self.scope.clone()))?;
    let scope = &mut *self.scope.borrow_mut();
    self.obj.set(scope, self.next_key.take().unwrap(), v8_value);
    Ok(())
  }

  fn end(self) -> JsResult<'a> {
    debug_assert!(self.next_key.is_none());
    Ok(self.obj.into())
  }
}

#[derive(Clone)]
pub struct Serializer<'a, 'b> {
  scope: ScopePtr<'a, 'b>,
}

impl<'a, 'b> Serializer<'a, 'b> {
  pub fn new(scope: ScopePtr<'a, 'b>) -> Self {
    Serializer { scope }
  }
}

macro_rules! forward_to {
    ($($name:ident($ty:ty, $to:ident, $lt:lifetime);)*) => {
        $(fn $name(self, v: $ty) -> JsResult<$lt> {
            self.$to(v as _)
        })*
    };
}

impl<'a, 'b> ser::Serializer for Serializer<'a, 'b> {
  type Ok = v8::Local<'a, v8::Value>;
  type Error = Error;

  type SerializeSeq = ArraySerializer<'a, 'b>;
  type SerializeTuple = ArraySerializer<'a, 'b>;
  type SerializeTupleStruct = ArraySerializer<'a, 'b>;
  type SerializeTupleVariant =
    VariantSerializer<'a, 'b, ArraySerializer<'a, 'b>>;
  type SerializeMap = MapSerializer<'a, 'b>;
  type SerializeStruct = StructSerializers<'a, 'b>;
  type SerializeStructVariant =
    VariantSerializer<'a, 'b, StructSerializers<'a, 'b>>;

  forward_to! {
      serialize_i8(i8, serialize_i32, 'a);
      serialize_i16(i16, serialize_i32, 'a);

      serialize_u8(u8, serialize_u32, 'a);
      serialize_u16(u16, serialize_u32, 'a);

      serialize_f32(f32, serialize_f64, 'a);
      serialize_u64(u64, serialize_f64, 'a);
      serialize_i64(i64, serialize_f64, 'a);
  }

  fn serialize_i32(self, v: i32) -> JsResult<'a> {
    Ok(v8::Integer::new(&mut self.scope.borrow_mut(), v).into())
  }

  fn serialize_u32(self, v: u32) -> JsResult<'a> {
    Ok(v8::Integer::new_from_unsigned(&mut self.scope.borrow_mut(), v).into())
  }

  fn serialize_f64(self, v: f64) -> JsResult<'a> {
    Ok(v8::Number::new(&mut self.scope.borrow_mut(), v).into())
  }

  fn serialize_bool(self, v: bool) -> JsResult<'a> {
    Ok(v8::Boolean::new(&mut self.scope.borrow_mut(), v).into())
  }

  fn serialize_char(self, _v: char) -> JsResult<'a> {
    unimplemented!();
  }

  fn serialize_str(self, v: &str) -> JsResult<'a> {
    v8::String::new(&mut self.scope.borrow_mut(), v)
      .map(|v| v.into())
      .ok_or(Error::ExpectedString)
  }

  fn serialize_bytes(self, _v: &[u8]) -> JsResult<'a> {
    // TODO: investigate using Uint8Arrays
    unimplemented!()
  }

  fn serialize_none(self) -> JsResult<'a> {
    Ok(v8::null(&mut self.scope.borrow_mut()).into())
  }

  fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> JsResult<'a> {
    value.serialize(self)
  }

  fn serialize_unit(self) -> JsResult<'a> {
    Ok(v8::null(&mut self.scope.borrow_mut()).into())
  }

  fn serialize_unit_struct(self, _name: &'static str) -> JsResult<'a> {
    Ok(v8::null(&mut self.scope.borrow_mut()).into())
  }

  /// For compatibility with serde-json, serialises unit variants as "Variant" strings.
  fn serialize_unit_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    variant: &'static str,
  ) -> JsResult<'a> {
    Ok(v8_struct_key(&mut self.scope.borrow_mut(), variant).into())
  }

  fn serialize_newtype_struct<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    value: &T,
  ) -> JsResult<'a> {
    value.serialize(self)
  }

  fn serialize_newtype_variant<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _variant_index: u32,
    variant: &'static str,
    value: &T,
  ) -> JsResult<'a> {
    let scope = self.scope.clone();
    let x = self.serialize_newtype_struct(variant, value)?;
    VariantSerializer::new(scope, variant, x).end(Ok)
  }

  /// Serialises any Rust iterable into a JS Array
  fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
    Ok(ArraySerializer::new(self.scope, len))
  }

  fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
    self.serialize_seq(Some(len))
  }

  fn serialize_tuple_struct(
    self,
    _name: &'static str,
    len: usize,
  ) -> Result<Self::SerializeTupleStruct> {
    self.serialize_tuple(len)
  }

  fn serialize_tuple_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    variant: &'static str,
    len: usize,
  ) -> Result<Self::SerializeTupleVariant> {
    Ok(VariantSerializer::new(
      self.scope.clone(),
      variant,
      self.serialize_tuple_struct(variant, len)?,
    ))
  }

  fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
    // Serializes a rust Map (e.g: BTreeMap, HashMap) to a v8 Object
    // TODO: consider allowing serializing to v8 Maps (e.g: via a magic type)
    // since they're lighter and better suited for K/V data
    // and maybe restrict keys (e.g: strings and numbers)
    Ok(MapSerializer::new(self.scope))
  }

  /// Serialises Rust typed structs into plain JS objects.
  fn serialize_struct(
    self,
    name: &'static str,
    _len: usize,
  ) -> Result<Self::SerializeStruct> {
    if name == magic::NAME {
      let m = MagicSerializer {
        scope: self.scope,
        v8_value: None,
      };
      return Ok(StructSerializers::Magic(m));
    }
    let o = ObjectSerializer::new(self.scope);
    Ok(StructSerializers::Regular(o))
  }

  fn serialize_struct_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    variant: &'static str,
    len: usize,
  ) -> Result<Self::SerializeStructVariant> {
    let scope = self.scope.clone();
    let x = self.serialize_struct(variant, len)?;
    Ok(VariantSerializer::new(scope, variant, x))
  }
}

macro_rules! not_reachable {
    ($($name:ident($ty:ty, $lt:lifetime);)*) => {
        $(fn $name(self, _v: $ty) -> JsResult<$lt> {
            unreachable!();
        })*
    };
}

/// A VERY hackish serde::Serializer
/// that exists solely to transmute a u64 to a serde_v8::Value
struct MagicTransmuter<'a, 'b> {
  _scope: ScopePtr<'a, 'b>,
}

impl<'a, 'b> ser::Serializer for MagicTransmuter<'a, 'b> {
  type Ok = v8::Local<'a, v8::Value>;
  type Error = Error;

  type SerializeSeq = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeTuple = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeTupleStruct = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeTupleVariant = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeMap = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeStruct = Impossible<v8::Local<'a, v8::Value>, Error>;
  type SerializeStructVariant = Impossible<v8::Local<'a, v8::Value>, Error>;

  // The only serialize method for this hackish struct
  fn serialize_u64(self, v: u64) -> JsResult<'a> {
    let mv: magic::Value = unsafe { std::mem::transmute(v) };
    Ok(mv.v8_value)
  }

  not_reachable! {
      serialize_i8(i8, 'a);
      serialize_i16(i16, 'a);
      serialize_i32(i32, 'a);
      serialize_i64(i64, 'a);
      serialize_u8(u8, 'a);
      serialize_u16(u16, 'a);
      serialize_u32(u32, 'a);
      // serialize_u64(u64, 'a); the chosen one
      serialize_f32(f32, 'a);
      serialize_f64(f64, 'a);
      serialize_bool(bool, 'a);
      serialize_char(char, 'a);
      serialize_str(&str, 'a);
      serialize_bytes(&[u8], 'a);
  }

  fn serialize_none(self) -> JsResult<'a> {
    unreachable!();
  }

  fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> JsResult<'a> {
    unreachable!();
  }

  fn serialize_unit(self) -> JsResult<'a> {
    unreachable!();
  }

  fn serialize_unit_struct(self, _name: &'static str) -> JsResult<'a> {
    unreachable!();
  }

  /// For compatibility with serde-json, serialises unit variants as "Variant" strings.
  fn serialize_unit_variant(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
  ) -> JsResult<'a> {
    unreachable!();
  }

  fn serialize_newtype_struct<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _value: &T,
  ) -> JsResult<'a> {
    unreachable!();
  }

  fn serialize_newtype_variant<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _variant_index: u32,
    _variant: &'static str,
    _value: &T,
  ) -> JsResult<'a> {
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

  /// Serialises Rust typed structs into plain JS objects.
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
