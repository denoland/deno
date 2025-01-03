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
  Obj,
  Regex,
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
      7 => Ok(PropFlags::Obj),
      8 => Ok(PropFlags::Regex),
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
pub struct NumPos(pub usize);
#[derive(Debug)]
pub struct ObjPos(pub usize);
#[derive(Debug)]
pub struct RegexPos(pub usize);

pub struct AllocNode(pub usize, pub usize);

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
  #[allow(dead_code)]
  Num(NumPos),
  #[allow(dead_code)]
  Obj(ObjPos),
  #[allow(dead_code)]
  Regex(RegexPos),
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
  fn obj_field(&mut self, prop: P, len: usize) -> ObjPos;
  fn str_field(&mut self, prop: P) -> StrPos;
  fn num_field(&mut self, prop: P) -> NumPos;
  fn bool_field(&mut self, prop: P) -> BoolPos;
  fn undefined_field(&mut self, prop: P) -> UndefPos;
  fn null_field(&mut self, prop: P) -> NullPos;
  fn regex_field(&mut self, prop: P) -> RegexPos;
  fn commit_schema(&mut self, offset: PendingNodeRef) -> NodeRef;

  fn write_ref(&mut self, pos: FieldPos, value: NodeRef);
  fn write_maybe_ref(&mut self, pos: FieldPos, value: Option<NodeRef>);
  fn write_refs(&mut self, pos: FieldArrPos, value: Vec<NodeRef>);
  fn write_str(&mut self, pos: StrPos, value: &str);
  fn write_bool(&mut self, pos: BoolPos, value: bool);
  fn write_num(&mut self, pos: NumPos, value: &str);
  fn write_regex(&mut self, pos: RegexPos, value: &str);

  fn serialize(&mut self) -> Vec<u8>;
}

#[derive(Debug)]
pub struct SerializeCtx {
  id: u32,

  start_buf: NodeRef,

  buf: Vec<u8>,
  field_buf: Vec<u8>,
  schema_map: Vec<usize>,
  spans: Vec<u8>,
  str_table: StringTable,
  kind_name_map: Vec<usize>,
  prop_name_map: Vec<usize>,

  // Internal, used for creating schemas
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
      id: 0,
      spans: vec![],
      start_buf: NodeRef(0),
      buf: vec![],
      field_buf: vec![],
      schema_map: vec![0; kind_size],
      str_table: StringTable::new(),
      kind_name_map: vec![0; kind_size],
      prop_name_map: vec![0; prop_size],
      field_count: 0,
    };

    let empty_str = ctx.str_table.insert("");

    // Placeholder node is always 0
    ctx.reserve_props(0, NodeRef(0), &DUMMY_SP, 0);
    ctx.kind_name_map[0] = empty_str;
    ctx.start_buf = NodeRef(ctx.buf.len());

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

  pub fn has_schema<N>(&self, kind: &N) -> bool
  where
    N: Into<u8> + Display + Clone,
  {
    let n: u8 = kind.clone().into();
    self.schema_map.get(n).is_none()
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

    if let Some(v) = self.prop_name_map.get::<usize>(n.into()) {
      if *v == 0 {
        let id = self.str_table.insert(&format!("{prop}"));
        self.prop_name_map[n as usize] = id;
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

  fn get_id(&mut self) -> u32 {
    let id = self.id;
    self.id += 1;
    id
  }

  fn reserve_props(&mut self, prop_count: usize) -> PendingNodeRef {
    let offset = self.buf.len();

    // No node has more than <10 properties
    debug_assert!(prop_count < 10);
    self.buf.push(prop_count as u8);

    PendingNodeRef(NodeRef(offset))
  }

  /// The node buffer contains enough information for traversal
  ///   <type u8>
  ///   <id u32>
  ///   <parent offset u32>
  ///   <child offset u32>
  ///   <next offset u32>
  pub fn append_node<N>(
    &mut self,
    kind: &N,
    parent: NodeRef,
    span: &Span,
  ) -> NodeRef
  where
    N: Into<u8> + Display + Clone,
  {
    let offset = NodeRef(self.buf.len());
    let n = kind.clone().into();
    self.buf.push(n);

    let id = self.get_id();
    append_u32(&mut self.buf, id);

    // Offset to the parent node. Will be 0 if none exists
    append_usize(&mut self.buf, parent.0);

    // Reserve child
    append_usize(&mut self.buf, 0);

    // Reserve next
    append_usize(&mut self.buf, 0);

    // Append span
    append_u32(&mut self.spans, span.lo.0);
    append_u32(&mut self.spans, span.hi.0);

    offset
  }

  pub fn get_alloc_pos(&self) -> AllocNode {
    AllocNode(self.buf.len(), self.field_buf.len())
  }

  pub fn begin_schema<N>(&mut self, kind: &N) -> usize
  where
    N: Into<u8> + Display + Clone,
  {
    #[cfg(debug_assertions)]
    {
      if self.field_count > 0 {
        panic!("Uncommitted schema");
      }
    }

    let offset = self.field_buf.len();

    let n = usize::try_from(kind.clone().into());
    self.schema_map[n] = offset;

    // prop count
    self.field_buf.push(0);

    offset
  }

  pub fn commit_schema(&mut self, offset: usize) {
    self.field_buf[offset] = self.field_count;
    self.field_count = 0;
  }

  /// Allocate the node header. It's always the same for every node.
  pub fn header<N>(
    &mut self,
    kind: N,
    parent: NodeRef,
    span: &Span,
  ) -> PendingNodeRef
  where
    N: Into<u8> + Display + Clone,
  {
    let kind_u8: u8 = kind.clone().into();

    if let Some(v) = self.kind_name_map.get::<usize>(kind_u8.into()) {
      if *v == 0 {
        let id = self.str_table.insert(&format!("{kind}"));
        self.kind_name_map[kind_u8 as usize] = id;
      }
    }

    self.append_node(kind_u8, parent);
    self.append_span(span);

    // Prop count will be filled with the actual value when the
    // schema is committed.
    self.reserve_props(kind_u8, 0)
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
  pub fn ref_vec_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field(prop, PropFlags::RefArr)
  }

  // Allocate a number field. Numbers are internally represented as strings
  pub fn obj_field<P>(&mut self, prop: P, len: usize) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    let offset = self.field(prop, PropFlags::Obj);
    append_usize(&mut self.buf, len);

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

  // Allocate a number field. Numbers are internally represented as strings
  pub fn num_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field(prop, PropFlags::Number)
  }

  // Allocate a regex field. Regexes are internally represented as strings
  pub fn regex_field<P>(&mut self, prop: P) -> usize
  where
    P: Into<u8> + Display + Clone,
  {
    self.field(prop, PropFlags::Regex)
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

  pub fn begin_write(&mut self, offset: &NodeRef) {
    //
  }

  /// Replace the placeholder of a reference field with the actual offset
  /// to the node we want to point to.
  pub fn write_ref(&mut self, value: NodeRef) {
    append_usize(&mut self.buf, value.0);
  }

  /// Helper for writing optional node offsets
  pub fn write_maybe_ref(&mut self, value: Option<NodeRef>) {
    let ref_value = if let Some(v) = value { v } else { NodeRef(0) };
    append_usize(&mut self.buf, ref_value.0);
  }

  /// Write a vec of node offsets into the property. The necessary space
  /// has been reserved earlier.
  pub fn write_ref_vec(&mut self, value: Vec<NodeRef>) {
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
  pub fn write_str(&mut self, value: &str) {
    let id = self.str_table.insert(value);
    append_usize(&mut self.field_buf, id);
  }

  /// Write a bool to a field.
  pub fn write_bool(&mut self, value: bool) {
    let n = if value { 1 } else { 0 };
    self.field_buf.push(n);
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
