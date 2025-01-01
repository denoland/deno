// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Display;

use deno_ast::swc::common::Span;
use deno_ast::swc::common::DUMMY_SP;
use indexmap::IndexMap;

/// Each property has this flag to mark what kind of value it holds-
/// Plain objects and arrays are not supported yet, but could be easily
/// added if needed.
#[derive(Debug, PartialEq)]
pub enum PropFlags {
  Ref,
  RefArr,
  String,
  Bool,
  Null,
  Undefined,
}

impl From<PropFlags> for u8 {
  fn from(m: PropFlags) -> u8 {
    m as u8
  }
}

impl TryFrom<u8> for PropFlags {
  type Error = &'static str;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(PropFlags::Ref),
      1 => Ok(PropFlags::RefArr),
      2 => Ok(PropFlags::String),
      3 => Ok(PropFlags::Bool),
      4 => Ok(PropFlags::Null),
      5 => Ok(PropFlags::Undefined),
      _ => Err("Unknown Prop flag"),
    }
  }
}

const MASK_U32_1: u32 = 0b11111111_00000000_00000000_00000000;
const MASK_U32_2: u32 = 0b00000000_11111111_00000000_00000000;
const MASK_U32_3: u32 = 0b00000000_00000000_11111111_00000000;
const MASK_U32_4: u32 = 0b00000000_00000000_00000000_11111111;

// TODO: There is probably a native Rust function to do this.
pub fn append_u32(result: &mut Vec<u8>, value: u32) {
  let v1: u8 = ((value & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((value & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((value & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (value & MASK_U32_4) as u8;

  result.push(v1);
  result.push(v2);
  result.push(v3);
  result.push(v4);
}

pub fn append_usize(result: &mut Vec<u8>, value: usize) {
  let raw = u32::try_from(value).unwrap();
  append_u32(result, raw);
}

pub fn write_usize(result: &mut [u8], value: usize, idx: usize) {
  let raw = u32::try_from(value).unwrap();

  let v1: u8 = ((raw & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((raw & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((raw & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (raw & MASK_U32_4) as u8;

  result[idx] = v1;
  result[idx + 1] = v2;
  result[idx + 2] = v3;
  result[idx + 3] = v4;
}

#[derive(Debug)]
pub struct StringTable {
  id: usize,
  table: IndexMap<String, usize>,
}

impl StringTable {
  pub fn new() -> Self {
    Self {
      id: 0,
      table: IndexMap::new(),
    }
  }

  pub fn insert(&mut self, s: &str) -> usize {
    if let Some(id) = self.table.get(s) {
      return *id;
    }

    let id = self.id;
    self.id += 1;
    self.table.insert(s.to_string(), id);
    id
  }

  pub fn serialize(&mut self) -> Vec<u8> {
    let mut result: Vec<u8> = vec![];
    append_u32(&mut result, self.table.len() as u32);

    // Assume that it's sorted by id
    for (s, _id) in &self.table {
      let bytes = s.as_bytes();
      append_u32(&mut result, bytes.len() as u32);
      result.append(&mut bytes.to_vec());
    }

    result
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRef(pub usize);

/// Represents an offset to a node whose schema hasn't been committed yet
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingNodeRef(pub NodeRef);

#[derive(Debug)]
pub struct BoolPos(pub usize);
#[derive(Debug)]
pub struct FieldPos(pub usize);
#[derive(Debug)]
pub struct FieldArrPos(pub usize);
#[derive(Debug)]
pub struct StrPos(pub usize);
#[derive(Debug)]
pub struct UndefPos(pub usize);
#[derive(Debug)]
pub struct NullPos(pub usize);

#[derive(Debug)]
pub enum NodePos {
  Bool(BoolPos),
  #[allow(dead_code)]
  Field(FieldPos),
  #[allow(dead_code)]
  FieldArr(FieldArrPos),
  Str(StrPos),
  Undef(UndefPos),
  #[allow(dead_code)]
  Null(NullPos),
}

pub trait AstBufSerializer<K, P>
where
  K: Into<u8> + Display,
  P: Into<u8> + Display,
{
  fn header(&mut self, kind: K, parent: NodeRef, span: &Span)
    -> PendingNodeRef;
  fn ref_field(&mut self, prop: P) -> FieldPos;
  fn ref_vec_field(&mut self, prop: P, len: usize) -> FieldArrPos;
  fn str_field(&mut self, prop: P) -> StrPos;
  fn bool_field(&mut self, prop: P) -> BoolPos;
  fn undefined_field(&mut self, prop: P) -> UndefPos;
  #[allow(dead_code)]
  fn null_field(&mut self, prop: P) -> NullPos;
  fn commit_schema(&mut self, offset: PendingNodeRef) -> NodeRef;

  fn write_ref(&mut self, pos: FieldPos, value: NodeRef);
  fn write_maybe_ref(&mut self, pos: FieldPos, value: Option<NodeRef>);
  fn write_refs(&mut self, pos: FieldArrPos, value: Vec<NodeRef>);
  fn write_str(&mut self, pos: StrPos, value: &str);
  fn write_bool(&mut self, pos: BoolPos, value: bool);

  fn serialize(&mut self) -> Vec<u8>;
}

#[derive(Debug)]
pub struct SerializeCtx {
  buf: Vec<u8>,
  start_buf: NodeRef,
  str_table: StringTable,
  kind_map: Vec<usize>,
  prop_map: Vec<usize>,
  field_count: u8,
}

/// This is the internal context used to allocate and fill the buffer. The point
/// is to be able to write absolute offsets directly in place.
///
/// The typical workflow is to reserve all necessary space for the currrent
/// node with placeholders for the offsets of the child nodes. Once child
/// nodes have been traversed, we know their offsets and can replace the
/// placeholder values with the actual ones.
impl SerializeCtx {
  pub fn new(kind_len: u8, prop_len: u8) -> Self {
    let kind_size = kind_len as usize;
    let prop_size = prop_len as usize;
    let mut ctx = Self {
      start_buf: NodeRef(0),
      buf: vec![],
      str_table: StringTable::new(),
      kind_map: vec![0; kind_size],
      prop_map: vec![0; prop_size],
      field_count: 0,
    };

    let empty_str = ctx.str_table.insert("");

    // Placeholder node is always 0
    ctx.append_node(0, NodeRef(0), &DUMMY_SP, 0);
    ctx.kind_map[0] = empty_str;
    ctx.start_buf = NodeRef(ctx.buf.len());

    // Insert default props that are always present
    let type_str = ctx.str_table.insert("type");
    let parent_str = ctx.str_table.insert("parent");
    let range_str = ctx.str_table.insert("range");
    let length_str = ctx.str_table.insert("length");

    // These values are expected to be in this order on the JS side
    ctx.prop_map[0] = empty_str;
    ctx.prop_map[1] = type_str;
    ctx.prop_map[2] = parent_str;
    ctx.prop_map[3] = range_str;
    ctx.prop_map[4] = length_str;

    ctx
  }

  /// Allocate a node's header
  fn field_header<P>(&mut self, prop: P, prop_flags: PropFlags) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_count += 1;

    let offset = self.buf.len();

    let n: u8 = prop.clone().into();
    self.buf.push(n);

    if let Some(v) = self.prop_map.get::<usize>(n.into()) {
      if *v == 0 {
        let id = self.str_table.insert(&format!("{prop}"));
        self.prop_map[n as usize] = id;
      }
    }

    let flags: u8 = prop_flags.into();
    self.buf.push(flags);

    offset
  }

  /// Allocate a property pointing to another node.
  fn field<P>(&mut self, prop: P, prop_flags: PropFlags) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    let offset = self.field_header(prop, prop_flags);

    append_usize(&mut self.buf, 0);

    offset
  }

  fn append_node(
    &mut self,
    kind: u8,
    parent: NodeRef,
    span: &Span,
    prop_count: usize,
  ) -> PendingNodeRef {
    let offset = self.buf.len();

    // Node type fits in a u8
    self.buf.push(kind);

    // Offset to the parent node. Will be 0 if none exists
    append_usize(&mut self.buf, parent.0);

    // Span, the start and end location of this node
    append_u32(&mut self.buf, span.lo.0);
    append_u32(&mut self.buf, span.hi.0);

    // No node has more than <10 properties
    debug_assert!(prop_count < 10);
    self.buf.push(prop_count as u8);

    PendingNodeRef(NodeRef(offset))
  }

  pub fn commit_schema(&mut self, node_ref: PendingNodeRef) -> NodeRef {
    let mut offset = node_ref.0 .0;

    // type + parentId + span lo + span hi
    offset += 1 + 4 + 4 + 4;

    self.buf[offset] = self.field_count;
    self.field_count = 0;

    node_ref.0
  }

  /// Allocate the node header. It's always the same for every node.
  ///   <type u8>
  ///   <parent offset u32>
  ///   <span lo u32>
  ///   <span high u32>
  ///   <property count u8> (There is no node with more than 10 properties)
  pub fn header<N>(
    &mut self,
    kind: N,
    parent: NodeRef,
    span: &Span,
  ) -> PendingNodeRef
  where
    N: Into<u8> + Display + Clone,
  {
    let n: u8 = kind.clone().into();

    if let Some(v) = self.kind_map.get::<usize>(n.into()) {
      if *v == 0 {
        let id = self.str_table.insert(&format!("{kind}"));
        self.kind_map[n as usize] = id;
      }
    }

    // Prop count will be filled with the actual value when the
    // schema is committed.
    self.append_node(n, parent, span, 0)
  }

  /// Allocate a reference property that will hold the offset of
  /// another node.
  pub fn ref_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field(prop, PropFlags::Ref)
  }

  /// Allocate a property that is a vec of node offsets pointing to other
  /// nodes.
  pub fn ref_vec_field<P>(&mut self, prop: P, len: usize) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    let offset = self.field(prop, PropFlags::RefArr);

    for _ in 0..len {
      append_u32(&mut self.buf, 0);
    }

    offset
  }

  // Allocate a property representing a string. Strings are deduplicated
  // in the message and the property will only contain the string id.
  pub fn str_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field(prop, PropFlags::String)
  }

  /// Allocate a bool field
  pub fn bool_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    let offset = self.field_header(prop, PropFlags::Bool);
    self.buf.push(0);
    offset
  }

  /// Allocate an undefined field
  pub fn undefined_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Undefined)
  }

  /// Allocate an undefined field
  #[allow(dead_code)]
  pub fn null_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Null)
  }

  /// Replace the placeholder of a reference field with the actual offset
  /// to the node we want to point to.
  pub fn write_ref(&mut self, field_offset: usize, value: NodeRef) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Ref {
        panic!("Trying to write a ref into a non-ref field")
      }
    }

    write_usize(&mut self.buf, value.0, field_offset + 2);
  }

  /// Helper for writing optional node offsets
  pub fn write_maybe_ref(
    &mut self,
    field_offset: usize,
    value: Option<NodeRef>,
  ) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Ref {
        panic!("Trying to write a ref into a non-ref field")
      }
    }

    let ref_value = if let Some(v) = value { v } else { NodeRef(0) };
    write_usize(&mut self.buf, ref_value.0, field_offset + 2);
  }

  /// Write a vec of node offsets into the property. The necessary space
  /// has been reserved earlier.
  pub fn write_refs(&mut self, field_offset: usize, value: Vec<NodeRef>) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::RefArr {
        panic!("Trying to write a ref into a non-ref array field")
      }
    }

    let mut offset = field_offset + 2;
    write_usize(&mut self.buf, value.len(), offset);
    offset += 4;

    for item in value {
      write_usize(&mut self.buf, item.0, offset);
      offset += 4;
    }
  }

  /// Store the string in our string table and save the id of the string
  /// in the current field.
  pub fn write_str(&mut self, field_offset: usize, value: &str) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::String {
        panic!("Trying to write a ref into a non-string field")
      }
    }

    let id = self.str_table.insert(value);
    write_usize(&mut self.buf, id, field_offset + 2);
  }

  /// Write a bool to a field.
  pub fn write_bool(&mut self, field_offset: usize, value: bool) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Bool {
        panic!("Trying to write a ref into a non-bool field")
      }
    }

    self.buf[field_offset + 2] = if value { 1 } else { 0 };
  }

  /// Serialize all information we have into a buffer that can be sent to JS.
  /// It has the following structure:
  ///
  ///   <...ast>
  ///   <string table>
  ///   <node kind map>  <- node kind id maps to string id
  ///   <node prop map> <- node property id maps to string id
  ///   <offset kind map>
  ///   <offset prop map>
  ///   <offset str table>
  pub fn serialize(&mut self) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![];

    // The buffer starts with the serialized AST first, because that
    // contains absolute offsets. By butting this at the start of the
    // message we don't have to waste time updating any offsets.
    buf.append(&mut self.buf);

    // Next follows the string table. We'll keep track of the offset
    // in the message of where the string table begins
    let offset_str_table = buf.len();

    // Serialize string table
    buf.append(&mut self.str_table.serialize());

    // Next, serialize the mappings of kind -> string of encountered
    // nodes in the AST. We use this additional lookup table to compress
    // the message so that we can save space by using a u8 . All nodes of
    // JS, TS and JSX together are <200
    let offset_kind_map = buf.len();

    // Write the total number of entries in the kind -> str mapping table
    // TODO: make this a u8
    append_usize(&mut buf, self.kind_map.len());
    for v in &self.kind_map {
      append_usize(&mut buf, *v);
    }

    // Store offset to prop -> string map. It's the same as with node kind
    // as the total number of properties is <120 which allows us to store it
    // as u8.
    let offset_prop_map = buf.len();
    // Write the total number of entries in the kind -> str mapping table
    append_usize(&mut buf, self.prop_map.len());
    for v in &self.prop_map {
      append_usize(&mut buf, *v);
    }

    // Putting offsets of relevant parts of the buffer at the end. This
    // allows us to hop to the relevant part by merely looking at the last
    // for values in the message. Each value represents an offset into the
    // buffer.
    append_usize(&mut buf, offset_kind_map);
    append_usize(&mut buf, offset_prop_map);
    append_usize(&mut buf, offset_str_table);
    append_usize(&mut buf, self.start_buf.0);

    buf
  }
}
