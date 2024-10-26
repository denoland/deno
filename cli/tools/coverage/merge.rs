// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
//
// Forked from https://github.com/demurgos/v8-coverage/tree/d0ca18da8740198681e0bc68971b0a6cdb11db3e/rust
// Copyright 2021 Charles Samborski. All rights reserved. MIT license.

use super::range_tree::RangeTree;
use super::range_tree::RangeTreeArena;
use crate::cdp;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::iter::Peekable;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct ProcessCoverage {
  pub result: Vec<cdp::ScriptCoverage>,
}

pub fn merge_processes(
  mut processes: Vec<ProcessCoverage>,
) -> Option<ProcessCoverage> {
  if processes.len() <= 1 {
    return processes.pop();
  }
  let mut url_to_scripts: BTreeMap<String, Vec<cdp::ScriptCoverage>> =
    BTreeMap::new();
  for process_cov in processes {
    for script_cov in process_cov.result {
      url_to_scripts
        .entry(script_cov.url.clone())
        .or_default()
        .push(script_cov);
    }
  }

  let result: Vec<cdp::ScriptCoverage> = url_to_scripts
    .into_iter()
    .enumerate()
    .map(|(script_id, (_, scripts))| (script_id, scripts))
    .map(|(script_id, scripts)| {
      let mut merged: cdp::ScriptCoverage =
        merge_scripts(scripts.to_vec()).unwrap();
      merged.script_id = script_id.to_string();
      merged
    })
    .collect();

  Some(ProcessCoverage { result })
}

pub fn merge_scripts(
  mut scripts: Vec<cdp::ScriptCoverage>,
) -> Option<cdp::ScriptCoverage> {
  if scripts.len() <= 1 {
    return scripts.pop();
  }
  let (script_id, url) = {
    let first: &cdp::ScriptCoverage = &scripts[0];
    (first.script_id.clone(), first.url.clone())
  };
  let mut range_to_funcs: BTreeMap<CharRange, Vec<cdp::FunctionCoverage>> =
    BTreeMap::new();
  for script_cov in scripts {
    for func_cov in script_cov.functions {
      let root_range = {
        let root_range_cov: &cdp::CoverageRange = &func_cov.ranges[0];
        CharRange {
          start: root_range_cov.start_char_offset,
          end: root_range_cov.end_char_offset,
        }
      };
      range_to_funcs.entry(root_range).or_default().push(func_cov);
    }
  }

  let functions: Vec<cdp::FunctionCoverage> = range_to_funcs
    .into_values()
    .map(|funcs| merge_functions(funcs).unwrap())
    .collect();

  Some(cdp::ScriptCoverage {
    script_id,
    url,
    functions,
  })
}

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
struct CharRange {
  start: usize,
  end: usize,
}

impl Ord for CharRange {
  fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
    if self.start != other.start {
      self.start.cmp(&other.start)
    } else {
      other.end.cmp(&self.end)
    }
  }
}

impl PartialOrd for CharRange {
  fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

pub fn merge_functions(
  mut funcs: Vec<cdp::FunctionCoverage>,
) -> Option<cdp::FunctionCoverage> {
  if funcs.len() <= 1 {
    return funcs.pop();
  }
  let function_name = funcs[0].function_name.clone();
  let rta_capacity: usize =
    funcs.iter().fold(0, |acc, func| acc + func.ranges.len());
  let rta = RangeTreeArena::with_capacity(rta_capacity);
  let mut trees: Vec<&mut RangeTree> = Vec::new();
  for func in funcs {
    if let Some(tree) = RangeTree::from_sorted_ranges(&rta, &func.ranges) {
      trees.push(tree);
    }
  }
  let merged = RangeTree::normalize(merge_range_trees(&rta, trees).unwrap());
  let ranges = merged.to_ranges();
  let is_block_coverage: bool = !(ranges.len() == 1 && ranges[0].count == 0);

  Some(cdp::FunctionCoverage {
    function_name,
    ranges,
    is_block_coverage,
  })
}

fn merge_range_trees<'a>(
  rta: &'a RangeTreeArena<'a>,
  mut trees: Vec<&'a mut RangeTree<'a>>,
) -> Option<&'a mut RangeTree<'a>> {
  if trees.len() <= 1 {
    return trees.pop();
  }
  let (start, end) = {
    let first = &trees[0];
    (first.start, first.end)
  };
  let delta: i64 = trees.iter().fold(0, |acc, tree| acc + tree.delta);
  let children = merge_range_tree_children(rta, trees);

  Some(rta.alloc(RangeTree::new(start, end, delta, children)))
}

struct StartEvent<'a> {
  offset: usize,
  trees: Vec<(usize, &'a mut RangeTree<'a>)>,
}

fn into_start_events<'a>(trees: Vec<&'a mut RangeTree<'a>>) -> Vec<StartEvent> {
  let mut result: BTreeMap<usize, Vec<(usize, &'a mut RangeTree<'a>)>> =
    BTreeMap::new();
  for (parent_index, tree) in trees.into_iter().enumerate() {
    for child in tree.children.drain(..) {
      result
        .entry(child.start)
        .or_default()
        .push((parent_index, child));
    }
  }
  result
    .into_iter()
    .map(|(offset, trees)| StartEvent { offset, trees })
    .collect()
}

struct StartEventQueue<'a> {
  pending: Option<StartEvent<'a>>,
  queue: Peekable<::std::vec::IntoIter<StartEvent<'a>>>,
}

impl<'a> StartEventQueue<'a> {
  pub fn new(queue: Vec<StartEvent<'a>>) -> StartEventQueue<'a> {
    StartEventQueue {
      pending: None,
      queue: queue.into_iter().peekable(),
    }
  }

  pub fn set_pending_offset(&mut self, offset: usize) {
    self.pending = Some(StartEvent {
      offset,
      trees: Vec::new(),
    });
  }

  pub fn push_pending_tree(&mut self, tree: (usize, &'a mut RangeTree<'a>)) {
    self.pending = self.pending.take().map(|mut start_event| {
      start_event.trees.push(tree);
      start_event
    });
  }
}

impl<'a> Iterator for StartEventQueue<'a> {
  type Item = StartEvent<'a>;

  fn next(&mut self) -> Option<<Self as Iterator>::Item> {
    let pending_offset: Option<usize> = match &self.pending {
      Some(ref start_event) if !start_event.trees.is_empty() => {
        Some(start_event.offset)
      }
      _ => None,
    };

    match pending_offset {
      Some(pending_offset) => {
        let queue_offset =
          self.queue.peek().map(|start_event| start_event.offset);
        match queue_offset {
          None => self.pending.take(),
          Some(queue_offset) => {
            if pending_offset < queue_offset {
              self.pending.take()
            } else {
              let mut result = self.queue.next().unwrap();
              if pending_offset == queue_offset {
                let pending_trees = self.pending.take().unwrap().trees;
                result.trees.extend(pending_trees)
              }
              Some(result)
            }
          }
        }
      }
      None => self.queue.next(),
    }
  }
}

fn merge_range_tree_children<'a>(
  rta: &'a RangeTreeArena<'a>,
  parent_trees: Vec<&'a mut RangeTree<'a>>,
) -> Vec<&'a mut RangeTree<'a>> {
  let mut flat_children: Vec<Vec<&'a mut RangeTree<'a>>> =
    Vec::with_capacity(parent_trees.len());
  let mut wrapped_children: Vec<Vec<&'a mut RangeTree<'a>>> =
    Vec::with_capacity(parent_trees.len());
  let mut open_range: Option<CharRange> = None;

  for _parent_tree in parent_trees.iter() {
    flat_children.push(Vec::new());
    wrapped_children.push(Vec::new());
  }

  let mut start_event_queue =
    StartEventQueue::new(into_start_events(parent_trees));

  let mut parent_to_nested: HashMap<usize, Vec<&'a mut RangeTree<'a>>> =
    HashMap::new();

  while let Some(event) = start_event_queue.next() {
    open_range = if let Some(open_range) = open_range {
      if open_range.end <= event.offset {
        for (parent_index, nested) in parent_to_nested {
          wrapped_children[parent_index].push(rta.alloc(RangeTree::new(
            open_range.start,
            open_range.end,
            0,
            nested,
          )));
        }
        parent_to_nested = HashMap::new();
        None
      } else {
        Some(open_range)
      }
    } else {
      None
    };

    match open_range {
      Some(open_range) => {
        for (parent_index, tree) in event.trees {
          let child = if tree.end > open_range.end {
            let (left, right) = RangeTree::split(rta, tree, open_range.end);
            start_event_queue.push_pending_tree((parent_index, right));
            left
          } else {
            tree
          };
          parent_to_nested
            .entry(parent_index)
            .or_default()
            .push(child);
        }
      }
      None => {
        let mut open_range_end: usize = event.offset + 1;
        for (_, ref tree) in &event.trees {
          open_range_end = if tree.end > open_range_end {
            tree.end
          } else {
            open_range_end
          };
        }
        for (parent_index, tree) in event.trees {
          if tree.end == open_range_end {
            flat_children[parent_index].push(tree);
            continue;
          }
          parent_to_nested.entry(parent_index).or_default().push(tree);
        }
        start_event_queue.set_pending_offset(open_range_end);
        open_range = Some(CharRange {
          start: event.offset,
          end: open_range_end,
        });
      }
    }
  }
  if let Some(open_range) = open_range {
    for (parent_index, nested) in parent_to_nested {
      wrapped_children[parent_index].push(rta.alloc(RangeTree::new(
        open_range.start,
        open_range.end,
        0,
        nested,
      )));
    }
  }

  let child_forests: Vec<Vec<&'a mut RangeTree<'a>>> = flat_children
    .into_iter()
    .zip(wrapped_children)
    .map(|(flat, wrapped)| merge_children_lists(flat, wrapped))
    .collect();

  let events = get_child_events_from_forests(&child_forests);

  let mut child_forests: Vec<
    Peekable<::std::vec::IntoIter<&'a mut RangeTree<'a>>>,
  > = child_forests
    .into_iter()
    .map(|forest| forest.into_iter().peekable())
    .collect();

  let mut result: Vec<&'a mut RangeTree<'a>> = Vec::new();
  for event in events.iter() {
    let mut matching_trees: Vec<&'a mut RangeTree<'a>> = Vec::new();
    for children in child_forests.iter_mut() {
      let next_tree: Option<&'a mut RangeTree<'a>> = {
        if children
          .peek()
          .map(|tree| tree.start == *event)
          .unwrap_or(false)
        {
          children.next()
        } else {
          None
        }
      };
      if let Some(next_tree) = next_tree {
        matching_trees.push(next_tree);
      }
    }
    if let Some(merged) = merge_range_trees(rta, matching_trees) {
      result.push(merged);
    }
  }

  result
}

fn get_child_events_from_forests<'a>(
  forests: &[Vec<&'a mut RangeTree<'a>>],
) -> BTreeSet<usize> {
  let mut event_set: BTreeSet<usize> = BTreeSet::new();
  for forest in forests {
    for tree in forest {
      event_set.insert(tree.start);
      event_set.insert(tree.end);
    }
  }
  event_set
}

// TODO: itertools?
// https://play.integer32.com/?gist=ad2cd20d628e647a5dbdd82e68a15cb6&version=stable&mode=debug&edition=2015
fn merge_children_lists<'a>(
  a: Vec<&'a mut RangeTree<'a>>,
  b: Vec<&'a mut RangeTree<'a>>,
) -> Vec<&'a mut RangeTree<'a>> {
  let mut merged: Vec<&'a mut RangeTree<'a>> = Vec::new();
  let mut a = a.into_iter();
  let mut b = b.into_iter();
  let mut next_a = a.next();
  let mut next_b = b.next();
  loop {
    match (next_a, next_b) {
      (Some(tree_a), Some(tree_b)) => {
        if tree_a.start < tree_b.start {
          merged.push(tree_a);
          next_a = a.next();
          next_b = Some(tree_b);
        } else {
          merged.push(tree_b);
          next_a = Some(tree_a);
          next_b = b.next();
        }
      }
      (Some(tree_a), None) => {
        merged.push(tree_a);
        merged.extend(a);
        break;
      }
      (None, Some(tree_b)) => {
        merged.push(tree_b);
        merged.extend(b);
        break;
      }
      (None, None) => break,
    }
  }

  merged
}

#[cfg(test)]
mod tests {
  use super::*;
  //   use test_generator::test_resources;

  #[test]
  fn empty() {
    let inputs: Vec<ProcessCoverage> = Vec::new();
    let expected: Option<ProcessCoverage> = None;

    assert_eq!(merge_processes(inputs), expected);
  }

  #[test]
  fn two_flat_trees() {
    let inputs: Vec<ProcessCoverage> = vec![
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 9,
              count: 1,
            }],
          }],
        }],
      },
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 9,
              count: 2,
            }],
          }],
        }],
      },
    ];
    let expected: Option<ProcessCoverage> = Some(ProcessCoverage {
      result: vec![cdp::ScriptCoverage {
        script_id: String::from("0"),
        url: String::from("/lib.js"),
        functions: vec![cdp::FunctionCoverage {
          function_name: String::from("lib"),
          is_block_coverage: true,
          ranges: vec![cdp::CoverageRange {
            start_char_offset: 0,
            end_char_offset: 9,
            count: 3,
          }],
        }],
      }],
    });

    assert_eq!(merge_processes(inputs), expected);
  }

  #[test]
  fn two_trees_with_matching_children() {
    let inputs: Vec<ProcessCoverage> = vec![
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 10,
              },
              cdp::CoverageRange {
                start_char_offset: 3,
                end_char_offset: 6,
                count: 1,
              },
            ],
          }],
        }],
      },
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 20,
              },
              cdp::CoverageRange {
                start_char_offset: 3,
                end_char_offset: 6,
                count: 2,
              },
            ],
          }],
        }],
      },
    ];
    let expected: Option<ProcessCoverage> = Some(ProcessCoverage {
      result: vec![cdp::ScriptCoverage {
        script_id: String::from("0"),
        url: String::from("/lib.js"),
        functions: vec![cdp::FunctionCoverage {
          function_name: String::from("lib"),
          is_block_coverage: true,
          ranges: vec![
            cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 9,
              count: 30,
            },
            cdp::CoverageRange {
              start_char_offset: 3,
              end_char_offset: 6,
              count: 3,
            },
          ],
        }],
      }],
    });

    assert_eq!(merge_processes(inputs), expected);
  }

  #[test]
  fn two_trees_with_partially_overlapping_children() {
    let inputs: Vec<ProcessCoverage> = vec![
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 10,
              },
              cdp::CoverageRange {
                start_char_offset: 2,
                end_char_offset: 5,
                count: 1,
              },
            ],
          }],
        }],
      },
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 20,
              },
              cdp::CoverageRange {
                start_char_offset: 4,
                end_char_offset: 7,
                count: 2,
              },
            ],
          }],
        }],
      },
    ];
    let expected: Option<ProcessCoverage> = Some(ProcessCoverage {
      result: vec![cdp::ScriptCoverage {
        script_id: String::from("0"),
        url: String::from("/lib.js"),
        functions: vec![cdp::FunctionCoverage {
          function_name: String::from("lib"),
          is_block_coverage: true,
          ranges: vec![
            cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 9,
              count: 30,
            },
            cdp::CoverageRange {
              start_char_offset: 2,
              end_char_offset: 5,
              count: 21,
            },
            cdp::CoverageRange {
              start_char_offset: 4,
              end_char_offset: 5,
              count: 3,
            },
            cdp::CoverageRange {
              start_char_offset: 5,
              end_char_offset: 7,
              count: 12,
            },
          ],
        }],
      }],
    });

    assert_eq!(merge_processes(inputs), expected);
  }

  #[test]
  fn two_trees_with_with_complementary_children_summing_to_the_same_count() {
    let inputs: Vec<ProcessCoverage> = vec![
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 1,
              },
              cdp::CoverageRange {
                start_char_offset: 1,
                end_char_offset: 8,
                count: 6,
              },
              cdp::CoverageRange {
                start_char_offset: 1,
                end_char_offset: 5,
                count: 5,
              },
              cdp::CoverageRange {
                start_char_offset: 5,
                end_char_offset: 8,
                count: 7,
              },
            ],
          }],
        }],
      },
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 9,
                count: 4,
              },
              cdp::CoverageRange {
                start_char_offset: 1,
                end_char_offset: 8,
                count: 8,
              },
              cdp::CoverageRange {
                start_char_offset: 1,
                end_char_offset: 5,
                count: 9,
              },
              cdp::CoverageRange {
                start_char_offset: 5,
                end_char_offset: 8,
                count: 7,
              },
            ],
          }],
        }],
      },
    ];
    let expected: Option<ProcessCoverage> = Some(ProcessCoverage {
      result: vec![cdp::ScriptCoverage {
        script_id: String::from("0"),
        url: String::from("/lib.js"),
        functions: vec![cdp::FunctionCoverage {
          function_name: String::from("lib"),
          is_block_coverage: true,
          ranges: vec![
            cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 9,
              count: 5,
            },
            cdp::CoverageRange {
              start_char_offset: 1,
              end_char_offset: 8,
              count: 14,
            },
          ],
        }],
      }],
    });

    assert_eq!(merge_processes(inputs), expected);
  }

  #[test]
  fn merges_a_similar_sliding_chain_a_bc() {
    let inputs: Vec<ProcessCoverage> = vec![
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 7,
                count: 10,
              },
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 4,
                count: 1,
              },
            ],
          }],
        }],
      },
      ProcessCoverage {
        result: vec![cdp::ScriptCoverage {
          script_id: String::from("0"),
          url: String::from("/lib.js"),
          functions: vec![cdp::FunctionCoverage {
            function_name: String::from("lib"),
            is_block_coverage: true,
            ranges: vec![
              cdp::CoverageRange {
                start_char_offset: 0,
                end_char_offset: 7,
                count: 20,
              },
              cdp::CoverageRange {
                start_char_offset: 1,
                end_char_offset: 6,
                count: 11,
              },
              cdp::CoverageRange {
                start_char_offset: 2,
                end_char_offset: 5,
                count: 2,
              },
            ],
          }],
        }],
      },
    ];
    let expected: Option<ProcessCoverage> = Some(ProcessCoverage {
      result: vec![cdp::ScriptCoverage {
        script_id: String::from("0"),
        url: String::from("/lib.js"),
        functions: vec![cdp::FunctionCoverage {
          function_name: String::from("lib"),
          is_block_coverage: true,
          ranges: vec![
            cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 7,
              count: 30,
            },
            cdp::CoverageRange {
              start_char_offset: 0,
              end_char_offset: 6,
              count: 21,
            },
            cdp::CoverageRange {
              start_char_offset: 1,
              end_char_offset: 5,
              count: 12,
            },
            cdp::CoverageRange {
              start_char_offset: 2,
              end_char_offset: 4,
              count: 3,
            },
          ],
        }],
      }],
    });

    assert_eq!(merge_processes(inputs), expected);
  }
}
