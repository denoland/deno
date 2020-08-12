// #[macro_use]
use crate::colors;
use crate::file_fetcher::SourceFile;
use crate::global_state::GlobalState;
use crate::human_size;
use crate::msg;
use crate::MainWorker;
use crate::ModuleGraph;
use crate::ModuleGraphLoader;
use crate::ModuleSpecifier;
use crate::Permissions;
//use crate::tsc::TargetLib;
use deno_core::ErrBox;
use serde::ser::SerializeStruct;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Struct containing a module's dependency information.
#[derive(Serialize)]
pub struct ModuleDepInfo {
  local: String,
  file_type: String,
  compiled: Option<String>,
  map: Option<String>,
  dep_count: usize,
  deps: FileInfoDepTree,
}

impl ModuleDepInfo {
  /// Creates a new `ModuleDepInfo` struct for the module with the provided `ModuleSpecifier`.
  pub async fn new(
    worker: &MainWorker,
    module_specifier: ModuleSpecifier,
  ) -> Result<ModuleDepInfo, ErrBox> {
    let global_state = worker.state.borrow().global_state.clone();
    let ts_compiler = &global_state.ts_compiler;
    let file_fetcher = &global_state.file_fetcher;
    let out = file_fetcher
      .fetch_source_file(&module_specifier, None, Permissions::allow_all())
      .await?;
    let local = out.filename.to_str().unwrap().to_string();
    let file_type = msg::enum_name_media_type(out.media_type).to_string();
    let compiled: Option<String> = ts_compiler
      .get_compiled_source_file(&out.url)
      .ok()
      .and_then(get_source_filename);
    let map: Option<String> = ts_compiler
      .get_source_map_file(&module_specifier)
      .ok()
      .and_then(get_source_filename);
    let module_graph =
      get_module_graph(&global_state, &module_specifier).await?;
    let deps = FileInfoDepTree::new(&module_graph, &module_specifier);

    let info = Self {
      local,
      file_type,
      compiled,
      map,
      dep_count: module_graph.len() - 1,
      deps,
    };

    Ok(info)
  }

  /// Prints the module info to the console.
  pub fn print(self) {
    println!("{} {}", colors::bold("local:"), self.local);
    println!("{} {}", colors::bold("type:"), self.file_type);
    if let Some(compiled) = self.compiled {
      println!("{} {}", colors::bold("compiled:"), compiled);
    }
    if let Some(map) = self.map {
      println!("{} {}", colors::bold("map:"), map);
    }

    println!("{} {} unique", colors::bold("deps:"), self.dep_count);
    println!(
      "{} ({}, total = {})",
      self.deps.name,
      human_size(self.deps.size as f64),
      human_size(self.deps.total_size.unwrap() as f64)
    );

    for (idx, dep) in self.deps.deps.iter().enumerate() {
      print_file_dep_info(&dep, "  ", idx == self.deps.deps.len() - 1);
    }
  }
}

/// A dependency tree of the basic module information.
///
/// Constructed from a `ModuleGraph` and `ModuleSpecifier` that
/// acts as the root of the tree.
struct FileInfoDepTree {
  name: String,
  size: usize,
  total_size: Option<usize>,
  deps: Vec<FileInfoDepTree>,
}

impl serde::Serialize for FileInfoDepTree {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let mut state = serializer.serialize_struct("FileInfoDepTree", 4)?;

    state.serialize_field("name", &self.name)?;
    if let Some(total_size) = self.total_size {
      state.serialize_field("size", &self.size)?;
      state.serialize_field("total_size", &total_size)?;
    }
    state.serialize_field("deps", &self.deps)?;
    state.end()
  }
}

impl FileInfoDepTree {
  /// Create a `FileInfoDepTree` tree from a `ModuleGraph` and the root `ModuleSpecifier`.
  pub fn new(
    module_graph: &ModuleGraph,
    root_specifier: &ModuleSpecifier,
  ) -> Self {
    let mut seen = HashSet::new();
    let mut total_sizes = HashMap::new();

    Self::cstr_helper(&mut seen, &mut total_sizes, module_graph, root_specifier)
  }

  fn cstr_helper(
    seen: &mut HashSet<String>,
    total_sizes: &mut HashMap<String, usize>,
    graph: &ModuleGraph,
    root: &ModuleSpecifier,
  ) -> Self {
    let name = root.to_string();
    let never_seen = seen.insert(name.clone());
    let file = graph.get(&name).unwrap();
    let size = file.size();
    let deps = if never_seen {
      file
        .imports
        .iter()
        .map(|import| import.resolved_specifier.clone())
        .map(|spec| Self::cstr_helper(seen, total_sizes, graph, &spec))
        .collect::<Vec<_>>()
    } else {
      vec![]
    };

    let total_size = if never_seen {
      if let Some(total_size) = total_sizes.get(&name) {
        Some(total_size.to_owned())
      } else {
        let total: usize = deps
          .iter()
          .map(|dep| {
            if let Some(total_size) = dep.total_size {
              total_size
            } else {
              0
            }
          })
          .sum();
        let total = size + total;

        total_sizes.insert(name.clone(), total);

        Some(total)
      }
    } else {
      None
    };

    Self {
      name,
      size,
      total_size,
      deps,
    }
  }
}

/// Prints the `FileInfoDepTree` tree to stdout.
fn print_file_dep_info(info: &FileInfoDepTree, prefix: &str, is_last: bool) {
  print_dep(prefix, is_last, info);

  let prefix = &get_new_prefix(prefix, is_last);
  let child_count = info.deps.len();
  for (idx, dep) in info.deps.iter().enumerate() {
    print_file_dep_info(dep, prefix, idx == child_count - 1);
  }
}

/// Prints a single `FileInfoDepTree` to stdout.
fn print_dep(prefix: &str, is_last: bool, info: &FileInfoDepTree) {
  let has_children = !info.deps.is_empty();

  println!(
    "{}{}─{} {}{}",
    prefix,
    get_sibling_connector(is_last),
    get_child_connector(has_children),
    info.name,
    get_formatted_totals(info)
  );
}

/// Gets the formatted totals for the provided `FileInfoDepTree`.
///
/// If the total size is reported as 0 then an empty string is returned.
fn get_formatted_totals(info: &FileInfoDepTree) -> String {
  if let Some(total_size) = info.total_size {
    format!(
      " ({}, total = {})",
      human_size(info.size as f64),
      human_size(total_size as f64)
    )
  } else {
    "".to_string()
  }
}

/// Gets the sibling portion of the tree branch.
fn get_sibling_connector(is_last: bool) -> char {
  if is_last {
    '└'
  } else {
    '├'
  }
}

/// Gets the child connector for the branch.
fn get_child_connector(has_children: bool) -> char {
  if has_children {
    '┬'
  } else {
    '─'
  }
}

/// Creates a new prefix for a dependency tree item.
fn get_new_prefix(prefix: &str, is_last: bool) -> String {
  let mut prefix = prefix.to_string();
  if is_last {
    prefix.push(' ');
  } else {
    prefix.push('│');
  }

  prefix.push(' ');
  prefix
}

/// Gets the full filename of a `SourceFile`.
fn get_source_filename(file: SourceFile) -> Option<String> {
  file.filename.to_str().map(|s| s.to_owned())
}

/// Constructs a ModuleGraph for the module with the provided name.
async fn get_module_graph(
  global_state: &GlobalState,
  module_specifier: &ModuleSpecifier,
) -> Result<ModuleGraph, ErrBox> {
    /*global_state
    .prepare_module_load(
      module_specifier.clone(),
      None,
      TargetLib::Main,
      Permissions::allow_all(),
      false,
      global_state.maybe_import_map.clone(),
    )
    .await?;
  global_state
    .clone()
    .fetch_compiled_module(module_specifier.clone(), None)
    .await?;
*/
  let mut module_graph_loader = ModuleGraphLoader::new(
    global_state.file_fetcher.clone(),
    global_state.maybe_import_map.clone(),
    Permissions::allow_all(),
    false,
    true,
  );
  module_graph_loader
    .add_to_graph(&module_specifier, None)
    .await?;
  Ok(module_graph_loader.get_graph())
}

#[test]
fn get_new_prefix_adds_spaces_if_is_last() {
  let prefix = get_new_prefix("", true);

  assert_eq!(prefix, "  ".to_string());
}

#[test]
fn get_new_prefix_adds_a_vertial_bar_if_not_is_last() {
  let prefix = get_new_prefix("", false);

  assert_eq!(prefix, "│ ".to_string());
}
