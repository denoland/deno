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
  Number,
  Bool,
  Null,
  Undefined,
  Object,
  Regex,
  BigInt,
  Array,
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
      3 => Ok(PropFlags::Number),
      4 => Ok(PropFlags::Bool),
      5 => Ok(PropFlags::Null),
      6 => Ok(PropFlags::Undefined),
      7 => Ok(PropFlags::Object),
      8 => Ok(PropFlags::Regex),
      9 => Ok(PropFlags::BigInt),
      10 => Ok(PropFlags::Array),
      _ => Err("Unknown Prop flag"),
    }
  }
}

const GROUP_KIND: u8 = 1;
const MASK_U32_1: u32 = 0b11111111_00000000_00000000_00000000;
const MASK_U32_2: u32 = 0b00000000_11111111_00000000_00000000;
const MASK_U32_3: u32 = 0b00000000_00000000_11111111_00000000;
const MASK_U32_4: u32 = 0b00000000_00000000_00000000_11111111;

#[inline]
fn append_u32(result: &mut Vec<u8>, value: u32) {
  let v1: u8 = ((value & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((value & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((value & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (value & MASK_U32_4) as u8;

  result.push(v1);
  result.push(v2);
  result.push(v3);
  result.push(v4);
}

#[inline]
fn write_u32(result: &mut [u8], value: u32, offset: usize) {
  let v1: u8 = ((value & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((value & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((value & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (value & MASK_U32_4) as u8;

  result[offset] = v1;
  result[offset + 1] = v2;
  result[offset + 2] = v3;
  result[offset + 3] = v4;
}

fn append_usize(result: &mut Vec<u8>, value: usize) {
  let raw = u32::try_from(value).unwrap();
  append_u32(result, raw);
}

fn write_usize(result: &mut [u8], value: usize, offset: usize) {
  let raw = u32::try_from(value).unwrap();
  write_u32(result, raw, offset);
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
pub struct NodeRef(pub u32);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingRef(pub u32);

pub trait AstBufSerializer {
  fn serialize(&mut self) -> Vec<u8>;
}

#[derive(Debug)]
pub struct SerializeCtx {
  id: u32,

  start_buf: NodeRef,

  /// Node buffer for traversal
  buf: Vec<u8>,
  field_buf: Vec<u8>,

  /// Vec of spans
  spans: Vec<u32>,

  /// Maps string id to the actual string
  str_table: StringTable,
  /// Maps kind id to string id
  kind_name_map: Vec<usize>,
  /// Maps prop id to string id
  prop_name_map: Vec<usize>,

  /// Internal, used for creating schemas
  field_count: u8,
  field_offset: usize,
  prev_sibling_node: Option<u32>,
}

/// <type u8>
/// <prop offset u32>
/// <child idx u32>
/// <next idx u32>
/// <parent idx u32>
const NODE_SIZE: u32 = 1 + 4 + 4 + 4 + 4;

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
      id: 0,
      spans: vec![],
      start_buf: NodeRef(0),
      buf: vec![],
      field_buf: vec![],
      str_table: StringTable::new(),
      kind_name_map: vec![0; kind_size],
      prop_name_map: vec![0; prop_size],
      field_count: 0,
      field_offset: 0,
      prev_sibling_node: None,
    };

    let empty_str = ctx.str_table.insert("");

    // Placeholder node is always 0
    ctx.append_node(0, &DUMMY_SP);
    ctx.kind_name_map[0] = empty_str;
    ctx.kind_name_map[1] = empty_str;
    ctx.start_buf = NodeRef(ctx.buf.len() as u32);

    // Insert default props that are always present
    let type_str = ctx.str_table.insert("type");
    let parent_str = ctx.str_table.insert("parent");
    let range_str = ctx.str_table.insert("range");
    let length_str = ctx.str_table.insert("length");

    // These values are expected to be in this order on the JS side
    ctx.prop_name_map[0] = empty_str;
    ctx.prop_name_map[1] = type_str;
    ctx.prop_name_map[2] = parent_str;
    ctx.prop_name_map[3] = range_str;
    ctx.prop_name_map[4] = length_str;

    ctx
  }

  /// Allocate a node's header
  fn field_header<P>(&mut self, prop: P, prop_flags: PropFlags)
  where
    P: Into<u8> + Display + Clone,
  {
    let flags: u8 = prop_flags.into();
    let n: u8 = prop.clone().into();

    if let Some(v) = self.prop_name_map.get::<usize>(n.into()) {
      if *v == 0 {
        let id = self.str_table.insert(&format!("{prop}"));
        self.prop_name_map[n as usize] = id;
      }
    }

    self.field_count += 1;
    self.field_buf.push(n);
    self.field_buf.push(flags);
  }

  fn get_id(&mut self) -> u32 {
    let id = self.id;
    self.id += 1;
    id
  }

  fn update_parent_link(&mut self, parent_id: u32, ref_id: u32) {
    let offset = ref_id * NODE_SIZE;

    // Update parent id of written ref
    let parent_offset = offset + 1 + 4 + 4;
    write_u32(&mut self.buf, parent_id, parent_offset.try_into().unwrap());
  }

  fn update_ref_links(&mut self, parent_id: u32, ref_id: u32) {
    self.update_parent_link(parent_id, ref_id);

    // Update next pointer of previous sibling
    if let Some(prev_id) = self.prev_sibling_node {
      let prev_offset = prev_id * NODE_SIZE;

      let prev_next = prev_offset + 1 + 4;
      write_u32(&mut self.buf, ref_id, prev_next.try_into().unwrap());
    } else {
      // Update parent child pointer
      let parent_offset = parent_id * NODE_SIZE;

      let child_offset = parent_offset + 1;
      write_u32(&mut self.buf, ref_id, child_offset.try_into().unwrap());
    }

    self.prev_sibling_node = Some(ref_id)
  }

  fn append_inner(
    &mut self,
    kind: u8,
    field_offset: usize,
    span_lo: u32,
    span_hi: u32,
  ) {
    // type
    self.buf.push(kind);

    if let Some(v) = self.kind_name_map.get::<usize>(kind.into()) {
      if *v == 0 {
        let s_id = self.str_table.insert(&format!("{kind}"));
        self.kind_name_map[kind as usize] = s_id;
      }
    }

    // field offset + child idx + next idx + parent idx
    append_usize(&mut self.buf, field_offset);
    append_usize(&mut self.buf, 0);
    append_usize(&mut self.buf, 0);
    append_usize(&mut self.buf, 0);

    // write spans
    self.spans.push(span_lo);
    self.spans.push(span_hi);
  }

  /// The node buffer contains enough information for traversal
  pub fn append_node<N>(&mut self, kind: N, span: &Span) -> PendingRef
  where
    N: Into<u8> + Display + Clone,
  {
    let id = self.get_id();

    self.field_offset = self.field_buf.len();
    self.field_buf.push(0);

    // type
    let kind_u8 = kind.clone().into();
    self.append_inner(kind_u8, self.field_offset, span.lo.0, span.hi.0);

    PendingRef(id)
  }

  pub fn commit_node(&mut self, id: PendingRef) -> NodeRef {
    self.field_buf[self.field_offset] = self.field_count;
    self.field_count = 0;
    self.prev_sibling_node = None;

    NodeRef(id.0)
  }

  // Allocate an object field
  pub fn write_obj<P>(&mut self, prop: P, len: usize)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Object);
    append_usize(&mut self.field_buf, len);
  }

  /// Allocate an null field
  pub fn write_null<P>(&mut self, prop: P)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Null);
    append_u32(&mut self.field_buf, 0);
  }

  /// Allocate a number field
  pub fn write_num<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Number);

    let id = self.str_table.insert(value);
    append_usize(&mut self.field_buf, id);
  }

  /// Allocate a bigint field
  pub fn write_bigint<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::BigInt);

    let id = self.str_table.insert(value);
    append_usize(&mut self.field_buf, id);
  }

  /// Allocate a RegExp field
  pub fn write_regex<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Regex);

    let id = self.str_table.insert(value);
    append_usize(&mut self.field_buf, id);
  }

  /// Store the string in our string table and save the id of the string
  /// in the current field.
  pub fn write_str<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::String);

    let id = self.str_table.insert(value);
    append_usize(&mut self.field_buf, id);
  }

  /// Write a bool to a field.
  pub fn write_bool<P>(&mut self, prop: P, value: bool)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Bool);

    let n = if value { 1 } else { 0 };
    append_u32(&mut self.field_buf, n);
  }

  /// Replace the placeholder of a reference field with the actual offset
  /// to the node we want to point to.
  pub fn write_ref<P>(&mut self, prop: P, parent: &PendingRef, value: NodeRef)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Ref);
    append_u32(&mut self.field_buf, value.0);

    self.update_ref_links(parent.0, value.0);
  }

  /// Helper for writing optional node offsets
  pub fn write_maybe_ref<P>(
    &mut self,
    prop: P,
    parent: &PendingRef,
    value: Option<NodeRef>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    let ref_value = if let Some(v) = value { v.0 } else { 0 };

    self.field_header(prop, PropFlags::Ref);
    append_u32(&mut self.field_buf, ref_value);

    self.update_ref_links(parent.0, ref_value);
  }

  /// Write a vec of node offsets into the property. The necessary space
  /// has been reserved earlier.
  pub fn write_ref_vec<P>(
    &mut self,
    prop: P,
    parent: &PendingRef,
    value: Vec<NodeRef>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::RefArr);

    let group_id = self.get_id();

    append_u32(&mut self.field_buf, group_id);

    // TODO(@marvinhagemeister) This is wrong
    self.append_inner(GROUP_KIND, 0, 0, 0);
    self.update_parent_link(parent.0, group_id);

    for item in value {
      self.update_parent_link(group_id, item.0);
    }
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
    append_usize(&mut buf, self.kind_name_map.len());
    for v in &self.kind_name_map {
      append_usize(&mut buf, *v);
    }

    // Store offset to prop -> string map. It's the same as with node kind
    // as the total number of properties is <120 which allows us to store it
    // as u8.
    let offset_prop_map = buf.len();
    // Write the total number of entries in the kind -> str mapping table
    append_usize(&mut buf, self.prop_name_map.len());
    for v in &self.prop_name_map {
      append_usize(&mut buf, *v);
    }

    // Spans are rarely needed, so they're stored in a separate array.
    // They're indexed by the node id.
    let offset_spans = buf.len();
    append_usize(&mut buf, self.spans.len());
    for v in &self.spans {
      append_u32(&mut buf, *v);
    }

    // Putting offsets of relevant parts of the buffer at the end. This
    // allows us to hop to the relevant part by merely looking at the last
    // for values in the message. Each value represents an offset into the
    // buffer.
    append_usize(&mut buf, offset_spans);
    append_usize(&mut buf, offset_kind_map);
    append_usize(&mut buf, offset_prop_map);
    append_usize(&mut buf, offset_str_table);
    append_u32(&mut buf, self.start_buf.0);

    buf
  }
}
