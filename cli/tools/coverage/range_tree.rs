// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
//
// Forked from https://github.com/demurgos/v8-coverage/tree/d0ca18da8740198681e0bc68971b0a6cdb11db3e/rust
// Copyright 2021 Charles Samborski. All rights reserved. MIT license.

use crate::cdp;
use std::iter::Peekable;
use typed_arena::Arena;

pub struct RangeTreeArena<'a>(Arena<RangeTree<'a>>);

impl<'a> RangeTreeArena<'a> {
  #[cfg(test)]
  pub fn new() -> Self {
    RangeTreeArena(Arena::new())
  }

  pub fn with_capacity(n: usize) -> Self {
    RangeTreeArena(Arena::with_capacity(n))
  }

  #[allow(clippy::mut_from_ref)]
  pub fn alloc(&'a self, value: RangeTree<'a>) -> &'a mut RangeTree<'a> {
    self.0.alloc(value)
  }
}

#[derive(Eq, PartialEq, Debug)]
pub struct RangeTree<'a> {
  pub start: usize,
  pub end: usize,
  pub delta: i64,
  pub children: Vec<&'a mut RangeTree<'a>>,
}

impl<'rt> RangeTree<'rt> {
  pub fn new<'a>(
    start: usize,
    end: usize,
    delta: i64,
    children: Vec<&'a mut RangeTree<'a>>,
  ) -> RangeTree<'a> {
    RangeTree {
      start,
      end,
      delta,
      children,
    }
  }

  pub fn split<'a>(
    rta: &'a RangeTreeArena<'a>,
    tree: &'a mut RangeTree<'a>,
    value: usize,
  ) -> (&'a mut RangeTree<'a>, &'a mut RangeTree<'a>) {
    let mut left_children: Vec<&'a mut RangeTree<'a>> = Vec::new();
    let mut right_children: Vec<&'a mut RangeTree<'a>> = Vec::new();
    for child in tree.children.iter_mut() {
      if child.end <= value {
        left_children.push(child);
      } else if value <= child.start {
        right_children.push(child);
      } else {
        let (left_child, right_child) = Self::split(rta, child, value);
        left_children.push(left_child);
        right_children.push(right_child);
      }
    }

    let left = RangeTree::new(tree.start, value, tree.delta, left_children);
    let right = RangeTree::new(value, tree.end, tree.delta, right_children);
    (rta.alloc(left), rta.alloc(right))
  }

  pub fn normalize<'a>(tree: &'a mut RangeTree<'a>) -> &'a mut RangeTree<'a> {
    tree.children = {
      let mut children: Vec<&'a mut RangeTree<'a>> = Vec::new();
      let mut chain: Vec<&'a mut RangeTree<'a>> = Vec::new();
      for child in tree.children.drain(..) {
        let is_chain_end: bool =
          match chain.last().map(|tree| (tree.delta, tree.end)) {
            Some((delta, chain_end)) => {
              (delta, chain_end) != (child.delta, child.start)
            }
            None => false,
          };
        if is_chain_end {
          let mut chain_iter = chain.drain(..);
          let head: &'a mut RangeTree<'a> = chain_iter.next().unwrap();
          for tree in chain_iter {
            head.end = tree.end;
            for sub_child in tree.children.drain(..) {
              sub_child.delta += tree.delta - head.delta;
              head.children.push(sub_child);
            }
          }
          children.push(RangeTree::normalize(head));
        }
        chain.push(child)
      }
      if !chain.is_empty() {
        let mut chain_iter = chain.drain(..);
        let head: &'a mut RangeTree<'a> = chain_iter.next().unwrap();
        for tree in chain_iter {
          head.end = tree.end;
          for sub_child in tree.children.drain(..) {
            sub_child.delta += tree.delta - head.delta;
            head.children.push(sub_child);
          }
        }
        children.push(RangeTree::normalize(head));
      }

      if children.len() == 1
        && children[0].start == tree.start
        && children[0].end == tree.end
      {
        let normalized = children.remove(0);
        normalized.delta += tree.delta;
        return normalized;
      }

      children
    };

    tree
  }

  pub fn to_ranges(&self) -> Vec<cdp::CoverageRange> {
    let mut ranges: Vec<cdp::CoverageRange> = Vec::new();
    let mut stack: Vec<(&RangeTree, i64)> = vec![(self, 0)];
    while let Some((cur, parent_count)) = stack.pop() {
      let count: i64 = parent_count + cur.delta;
      ranges.push(cdp::CoverageRange {
        start_char_offset: cur.start,
        end_char_offset: cur.end,
        count,
      });
      for child in cur.children.iter().rev() {
        stack.push((child, count))
      }
    }
    ranges
  }

  pub fn from_sorted_ranges<'a>(
    rta: &'a RangeTreeArena<'a>,
    ranges: &[cdp::CoverageRange],
  ) -> Option<&'a mut RangeTree<'a>> {
    Self::from_sorted_ranges_inner(
      rta,
      &mut ranges.iter().peekable(),
      usize::MAX,
      0,
    )
  }

  fn from_sorted_ranges_inner<'a, 'b, 'c: 'b>(
    rta: &'a RangeTreeArena<'a>,
    ranges: &'b mut Peekable<impl Iterator<Item = &'c cdp::CoverageRange>>,
    parent_end: usize,
    parent_count: i64,
  ) -> Option<&'a mut RangeTree<'a>> {
    let has_range: bool = match ranges.peek() {
      None => false,
      Some(range) => range.start_char_offset < parent_end,
    };
    if !has_range {
      return None;
    }
    let range = ranges.next().unwrap();
    let start: usize = range.start_char_offset;
    let end: usize = range.end_char_offset;
    let count: i64 = range.count;
    let delta: i64 = count - parent_count;
    let mut children: Vec<&mut RangeTree> = Vec::new();
    while let Some(child) =
      Self::from_sorted_ranges_inner(rta, ranges, end, count)
    {
      children.push(child);
    }
    Some(rta.alloc(RangeTree::new(start, end, delta, children)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn from_sorted_ranges_empty() {
    let rta = RangeTreeArena::new();
    let inputs: Vec<cdp::CoverageRange> = vec![cdp::CoverageRange {
      start_char_offset: 0,
      end_char_offset: 9,
      count: 1,
    }];
    let actual: Option<&mut RangeTree> =
      RangeTree::from_sorted_ranges(&rta, &inputs);
    let expected: Option<&mut RangeTree> =
      Some(rta.alloc(RangeTree::new(0, 9, 1, Vec::new())));

    assert_eq!(actual, expected);
  }
}
