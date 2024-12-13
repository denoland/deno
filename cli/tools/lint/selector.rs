use regex::Regex;

use super::ast_buf::{AstNode, AstProp};

#[derive(Debug)]
pub enum Op {
  Equal,
  NotEqual,
  Greater,
  Less,
  GreaterEqual,
  LessEqual,
}

#[derive(Debug)]
pub enum PropSelector {
  Exists(AstProp),
  Bin(Op, AstProp, String),
  Regex(AstProp, Regex),
}

#[derive(Debug)]
pub enum Pseudo {
  NthChild(usize, usize),
  Has(Selector),
  Not(Selector),
  Matches(Vec<Selector>),
}

#[derive(Debug)]
pub enum RelationOp {
  /// node ~ sibling
  Following(SelectorItem, SelectorItem),
  /// node + sibling
  Adjacent(),
  /// node > child
  Child,
  /// node child
  Descendant,
}

#[derive(Debug)]
pub struct Relation {
  op: RelationOp,
  left: Selector,
  right: Selector,
}

#[derive(Debug)]
pub enum Selector {
  Item(SelectorItem),
  Relation(Box<Selector>),
}

#[derive(Debug)]
pub struct SelectorItem {
  kind: Option<AstNode>,
  attrs: Vec<PropSelector>,
  pseudo: Vec<Pseudo>,
}
