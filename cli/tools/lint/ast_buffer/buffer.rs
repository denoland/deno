// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Display;

use deno_ast::swc::common::Span;
use deno_ast::swc::common::DUMMY_SP;
use indexmap::IndexMap;

use crate::util::text_encoding::Utf16Map;

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

pub type Index = u32;

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

fn append_usize(result: &mut Vec<u8>, value: usize) {
  let raw = u32::try_from(value).unwrap();
  append_u32(result, raw);
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
pub struct NodeRef(pub Index);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingRef(pub Index);

pub trait AstBufSerializer {
  fn serialize(&mut self) -> Vec<u8>;
}

/// <type u8>
/// <prop offset u32>
/// <child idx u32>
/// <next idx u32>
/// <parent idx u32>
#[derive(Debug)]
struct Node {
  kind: u8,
  prop_offset: u32,
  child: u32,
  next: u32,
  parent: u32,
}

#[derive(Debug)]
pub struct SerializeCtx {
  root_idx: Index,

  nodes: Vec<Node>,
  prop_stack: Vec<Vec<u8>>,
  field_count: Vec<usize>,
  field_buf: Vec<u8>,
  prev_sibling_stack: Vec<Index>,

  /// Vec of spans
  spans: Vec<u32>,

  /// Maps string id to the actual string
  str_table: StringTable,
  /// Maps kind id to string id
  kind_name_map: Vec<usize>,
  /// Maps prop id to string id
  prop_name_map: Vec<usize>,
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
      spans: Vec::with_capacity(512),
      root_idx: 0,
      nodes: Vec::with_capacity(512),
      prop_stack: vec![vec![]],
      prev_sibling_stack: vec![0],
      field_count: vec![0],
      field_buf: Vec::with_capacity(1024),
      str_table: StringTable::new(),
      kind_name_map: vec![0; kind_size],
      prop_name_map: vec![0; prop_size],
    };

    let empty_str = ctx.str_table.insert("");

    // Placeholder node is always 0
    ctx.append_node(0, &DUMMY_SP);
    ctx.kind_name_map[0] = empty_str;
    ctx.kind_name_map[1] = empty_str;

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

  pub fn set_root_idx(&mut self, idx: Index) {
    self.root_idx = idx;
  }

  pub fn map_utf8_spans_to_utf16(&mut self, map: &Utf16Map) {
    for value in &mut self.spans {
      *value = map
        .utf8_to_utf16_offset((*value).into())
        .unwrap_or_else(|| panic!("Failed converting '{value}' to utf16."))
        .into();
    }
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

    // Increment field counter
    let idx = self.field_count.len() - 1;
    let count = self.field_count[idx];
    self.field_count[idx] = count + 1;

    let buf = self.prop_stack.last_mut().unwrap();
    buf.push(n);
    buf.push(flags);
  }

  fn get_node(&mut self, id: Index) -> &mut Node {
    self.nodes.get_mut(id as usize).unwrap()
  }

  fn set_parent(&mut self, child_id: Index, parent_id: Index) {
    let child = self.get_node(child_id);
    child.parent = parent_id;
  }

  fn set_child(&mut self, parent_id: Index, child_id: Index) {
    let parent = self.get_node(parent_id);
    parent.child = child_id;
  }

  fn set_next(&mut self, node_id: Index, next_id: Index) {
    let node = self.get_node(node_id);
    node.next = next_id;
  }

  fn update_ref_links(&mut self, parent_id: Index, child_id: Index) {
    let last_idx = self.prev_sibling_stack.len() - 1;
    let parent = self.get_node(parent_id);
    if parent.child == 0 {
      parent.child = child_id;
    } else {
      let prev_id = self.prev_sibling_stack[last_idx];
      self.set_next(prev_id, child_id);
    }

    self.prev_sibling_stack[last_idx] = child_id;
    self.set_parent(child_id, parent_id);
  }

  pub fn append_node<K>(&mut self, kind: K, span: &Span) -> PendingRef
  where
    K: Into<u8> + Display + Clone,
  {
    let (start, end) = if *span == DUMMY_SP {
      (0, 0)
    } else {
      // -1 is because swc stores spans 1-indexed
      (span.lo.0 - 1, span.hi.0 - 1)
    };
    self.append_inner(kind, start, end)
  }

  pub fn append_inner<K>(
    &mut self,
    kind: K,
    span_lo: u32,
    span_hi: u32,
  ) -> PendingRef
  where
    K: Into<u8> + Display + Clone,
  {
    let kind_u8: u8 = kind.clone().into();

    let id: Index = self.nodes.len() as u32;

    self.nodes.push(Node {
      kind: kind_u8,
      prop_offset: 0,
      child: 0,
      next: 0,
      parent: 0,
    });

    if let Some(v) = self.kind_name_map.get::<usize>(kind_u8.into()) {
      if *v == 0 {
        let s_id = self.str_table.insert(&format!("{kind}"));
        self.kind_name_map[kind_u8 as usize] = s_id;
      }
    }

    self.field_count.push(0);
    self.prop_stack.push(vec![]);
    self.prev_sibling_stack.push(0);

    // write spans
    self.spans.push(span_lo);
    self.spans.push(span_hi);

    PendingRef(id)
  }

  pub fn commit_node(&mut self, id: PendingRef) -> NodeRef {
    let mut buf = self.prop_stack.pop().unwrap();
    let count = self.field_count.pop().unwrap();
    let offset = self.field_buf.len();

    // All nodes have <10 fields
    self.field_buf.push(count as u8);
    self.field_buf.append(&mut buf);

    let node = self.nodes.get_mut(id.0 as usize).unwrap();
    node.prop_offset = offset as u32;

    self.prev_sibling_stack.pop();

    NodeRef(id.0)
  }

  // Allocate an object field
  pub fn open_obj(&mut self) {
    self.field_count.push(0);
    self.prop_stack.push(vec![]);
  }

  pub fn commit_obj<P>(&mut self, prop: P)
  where
    P: Into<u8> + Display + Clone,
  {
    let mut buf = self.prop_stack.pop().unwrap();
    let count = self.field_count.pop().unwrap();
    let offset = self.field_buf.len();
    append_usize(&mut self.field_buf, count);
    self.field_buf.append(&mut buf);

    self.field_header(prop, PropFlags::Object);
    let buf = self.prop_stack.last_mut().unwrap();
    append_usize(buf, offset);
  }

  /// Allocate an null field
  pub fn write_null<P>(&mut self, prop: P)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Null);

    let buf = self.prop_stack.last_mut().unwrap();
    append_u32(buf, 0);
  }

  /// Allocate an null field
  pub fn write_undefined<P>(&mut self, prop: P)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Undefined);

    let buf = self.prop_stack.last_mut().unwrap();
    append_u32(buf, 0);
  }

  /// Allocate a number field
  pub fn write_num<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Number);

    let id = self.str_table.insert(value);
    let buf = self.prop_stack.last_mut().unwrap();
    append_usize(buf, id);
  }

  /// Allocate a bigint field
  pub fn write_bigint<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::BigInt);

    let id = self.str_table.insert(value);
    let buf = self.prop_stack.last_mut().unwrap();
    append_usize(buf, id);
  }

  /// Allocate a RegExp field
  pub fn write_regex<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Regex);

    let id = self.str_table.insert(value);
    let buf = self.prop_stack.last_mut().unwrap();
    append_usize(buf, id);
  }

  /// Store the string in our string table and save the id of the string
  /// in the current field.
  pub fn write_str<P>(&mut self, prop: P, value: &str)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::String);

    let id = self.str_table.insert(value);
    let buf = self.prop_stack.last_mut().unwrap();
    append_usize(buf, id);
  }

  /// Write a bool to a field.
  pub fn write_bool<P>(&mut self, prop: P, value: bool)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Bool);

    let n = if value { 1 } else { 0 };
    let buf = self.prop_stack.last_mut().unwrap();
    append_u32(buf, n);
  }

  /// Replace the placeholder of a reference field with the actual offset
  /// to the node we want to point to.
  pub fn write_ref<P>(&mut self, prop: P, parent: &PendingRef, value: NodeRef)
  where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::Ref);
    let buf = self.prop_stack.last_mut().unwrap();
    append_u32(buf, value.0);

    if parent.0 > 0 {
      self.update_ref_links(parent.0, value.0);
    }
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
    if let Some(v) = value {
      self.write_ref(prop, parent, v);
    } else {
      self.write_null(prop);
    };
  }

  /// Helper for writing optional node offsets with undefined as empty value
  pub fn write_maybe_undef_ref<P>(
    &mut self,
    prop: P,
    parent: &PendingRef,
    value: Option<NodeRef>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    if let Some(v) = value {
      self.write_ref(prop, parent, v);
    } else {
      self.write_undefined(prop);
    };
  }

  /// Write a vec of node offsets into the property. The necessary space
  /// has been reserved earlier.
  pub fn write_ref_vec<P>(
    &mut self,
    prop: P,
    parent_ref: &PendingRef,
    value: Vec<NodeRef>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    self.field_header(prop, PropFlags::RefArr);
    let group_id = self.append_node(GROUP_KIND, &DUMMY_SP);
    let group_id = self.commit_node(group_id).0;

    let buf = self.prop_stack.last_mut().unwrap();
    append_u32(buf, group_id);

    self.update_ref_links(parent_ref.0, group_id);

    let mut prev_id = 0;
    for (i, item) in value.iter().enumerate() {
      self.set_parent(item.0, group_id);

      if i == 0 {
        self.set_child(group_id, item.0);
      } else {
        self.set_next(prev_id, item.0);
      }

      prev_id = item.0;
    }
  }

  pub fn write_maybe_ref_vec_skip<P>(
    &mut self,
    prop: P,
    parent_ref: &PendingRef,
    value: Option<Vec<NodeRef>>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    if let Some(value) = value {
      self.write_ref_vec(prop, parent_ref, value);
    }
  }

  pub fn write_ref_vec_or_empty<P>(
    &mut self,
    prop: P,
    parent_ref: &PendingRef,
    value: Option<Vec<NodeRef>>,
  ) where
    P: Into<u8> + Display + Clone,
  {
    let actual = value.unwrap_or_default();
    self.write_ref_vec(prop, parent_ref, actual)
  }

  /// Serialize all information we have into a buffer that can be sent to JS.
  /// It has the following structure:
  ///
  ///   <...ast>
  ///   <string table>
  ///   <node kind map>  <- node kind id maps to string id
  ///   <node prop map> <- node property id maps to string id
  ///   <spans> <- List of spans, rarely needed
  ///   <offset spans>
  ///   <offset kind map>
  ///   <offset prop map>
  ///   <offset str table>
  pub fn serialize(&mut self) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![];

    // The buffer starts with the serialized AST first, because that
    // contains absolute offsets. By butting this at the start of the
    // message we don't have to waste time updating any offsets.
    for node in &self.nodes {
      buf.push(node.kind);
      append_u32(&mut buf, node.prop_offset);
      append_u32(&mut buf, node.child);
      append_u32(&mut buf, node.next);
      append_u32(&mut buf, node.parent);
    }

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
    for v in &self.spans {
      append_u32(&mut buf, *v);
    }

    // The field value table. They're detached from nodes as they're not
    // as frequently needed as the nodes themselves. The most common
    // operation is traversal and we can traverse nodes without knowing
    // about the fields.
    let offset_props = buf.len();
    buf.append(&mut self.field_buf);

    // Putting offsets of relevant parts of the buffer at the end. This
    // allows us to hop to the relevant part by merely looking at the last
    // for values in the message. Each value represents an offset into the
    // buffer.
    append_usize(&mut buf, offset_props);
    append_usize(&mut buf, offset_spans);
    append_usize(&mut buf, offset_kind_map);
    append_usize(&mut buf, offset_prop_map);
    append_usize(&mut buf, offset_str_table);
    append_u32(&mut buf, self.root_idx);

    buf
  }
}
