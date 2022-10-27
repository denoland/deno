// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_graph::Dependency;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::Resolved;
use deno_runtime::colors;

use crate::args::Flags;
use crate::args::InfoFlags;
use crate::checksum;
use crate::display;
use crate::lsp;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;
use crate::npm::NpmResolutionPackage;
use crate::npm::NpmResolutionSnapshot;
use crate::proc_state::ProcState;

pub async fn info(flags: Flags, info_flags: InfoFlags) -> Result<(), AnyError> {
  let ps = ProcState::build(flags).await?;
  if let Some(specifier) = info_flags.file {
    let specifier = resolve_url_or_path(&specifier)?;
    let graph = ps.create_graph(vec![(specifier, ModuleKind::Esm)]).await?;

    if info_flags.json {
      display::write_json_to_stdout(&json!(graph))?;
    } else {
      let mut output = String::new();
      let formatter =
        GraphDisplayFormatter::new(&graph, &ps.npm_resolver, &mut output);
      formatter.fmt_module_graph()?;
      display::write_to_stdout_ignore_sigpipe(output.as_bytes())?;
    }
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(&ps, info_flags.json, ps.options.location_flag())?;
  }
  Ok(())
}

fn print_cache_info(
  state: &ProcState,
  json: bool,
  location: Option<&deno_core::url::Url>,
) -> Result<(), AnyError> {
  let deno_dir = &state.dir.root;
  let modules_cache = &state.file_fetcher.get_http_cache_location();
  let npm_cache = &state.npm_cache.as_readonly().get_cache_location();
  let typescript_cache = &state.dir.gen_cache.location;
  let registry_cache =
    &state.dir.root.join(lsp::language_server::REGISTRIES_PATH);
  let mut origin_dir = state.dir.root.join("location_data");

  if let Some(location) = &location {
    origin_dir =
      origin_dir.join(&checksum::gen(&[location.to_string().as_bytes()]));
  }

  let local_storage_dir = origin_dir.join("local_storage");

  if json {
    let mut output = json!({
      "denoDir": deno_dir,
      "modulesCache": modules_cache,
      "npmCache": npm_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      output["localStorage"] = serde_json::to_value(local_storage_dir)?;
    }

    display::write_json_to_stdout(&output)
  } else {
    println!(
      "{} {}",
      colors::bold("DENO_DIR location:"),
      deno_dir.display()
    );
    println!(
      "{} {}",
      colors::bold("Remote modules cache:"),
      modules_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("npm modules cache:"),
      npm_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Emitted modules cache:"),
      typescript_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Language server registries cache:"),
      registry_cache.display(),
    );
    println!(
      "{} {}",
      colors::bold("Origin storage:"),
      origin_dir.display()
    );
    if location.is_some() {
      println!(
        "{} {}",
        colors::bold("Local Storage:"),
        local_storage_dir.display(),
      );
    }
    Ok(())
  }
}

const SIBLING_CONNECTOR: char = '├';
const LAST_SIBLING_CONNECTOR: char = '└';
const CHILD_DEPS_CONNECTOR: char = '┬';
const CHILD_NO_DEPS_CONNECTOR: char = '─';
const VERTICAL_CONNECTOR: char = '│';
const EMPTY_CONNECTOR: char = ' ';

struct TreeNode {
  text: String,
  children: Vec<TreeNode>,
}

impl TreeNode {
  pub fn from_text(text: String) -> Self {
    Self {
      text,
      children: Default::default(),
    }
  }

  pub fn write<TWrite: Write>(&self, writer: &mut TWrite) -> fmt::Result {
    writeln!(writer, "{}", self.text)?;
    self.write_children_with_prefix(writer, "", &self.children)?;
    Ok(())
  }

  fn write_children_with_prefix<TWrite: Write>(
    &self,
    writer: &mut TWrite,
    prefix: &str,
    children: &Vec<TreeNode>,
  ) -> fmt::Result {
    let child_len = children.len();
    for (index, child) in children.iter().enumerate() {
      let is_last = index + 1 == child_len;
      let sibling_connector = if is_last {
        LAST_SIBLING_CONNECTOR
      } else {
        SIBLING_CONNECTOR
      };
      let child_connector = if child.children.is_empty() {
        CHILD_NO_DEPS_CONNECTOR
      } else {
        CHILD_DEPS_CONNECTOR
      };
      writeln!(
        writer,
        "{} {}",
        colors::gray(format!(
          "{}{}─{}",
          prefix, sibling_connector, child_connector
        )),
        child.text
      )?;
      let mut prefix = prefix.to_string();
      if is_last {
        prefix.push(EMPTY_CONNECTOR);
      } else {
        prefix.push(VERTICAL_CONNECTOR);
      }
      prefix.push(EMPTY_CONNECTOR);
      self.write_children_with_prefix(writer, &prefix, &child.children)?;
    }
    Ok(())
  }
}

struct GraphDisplayFormatter<'a, TWrite: Write> {
  graph: &'a ModuleGraph,
  npm_resolver: &'a NpmPackageResolver,
  writer: TWrite,
  package_sizes: HashMap<NpmPackageId, u64>,
  seen: HashSet<String>,
  resolved_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
}

impl<'a, TWrite: Write> Write for GraphDisplayFormatter<'a, TWrite> {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    self.writer.write_str(s)
  }
}

impl<'a, TWrite: Write> GraphDisplayFormatter<'a, TWrite> {
  pub fn new(
    graph: &'a ModuleGraph,
    npm_resolver: &'a NpmPackageResolver,
    writer: TWrite,
  ) -> Self {
    Self {
      graph,
      npm_resolver,
      writer,
      package_sizes: Default::default(),
      packages: Default::default(),
      resolved_reqs: Default::default(),
      seen: Default::default(),
    }
  }

  fn fill_for_package(
    &mut self,
    package: &NpmResolutionPackage,
    snapshot: &NpmResolutionSnapshot,
  ) {
    self.packages.insert(package.id.clone(), package.clone());
    if let Ok(size) = self.npm_resolver.package_size(&package.id) {
      self.package_sizes.insert(package.id.clone(), size);
    }
    for id in package.dependencies.values() {
      if !self.packages.contains_key(id) {
        if let Some(package) = snapshot.package_from_id(id) {
          self.fill_for_package(package, snapshot);
        }
      }
    }
  }

  pub fn fmt_module_graph(mut self) -> fmt::Result {
    if self.graph.roots.is_empty() || self.graph.roots.len() > 1 {
      return writeln!(
        self,
        "{} displaying graphs that have multiple roots is not supported.",
        colors::red("error:")
      );
    }

    let snapshot = self.npm_resolver.snapshot();
    for (specifier, _) in self.graph.specifiers() {
      if let Ok(reference) = NpmPackageReference::from_specifier(&specifier) {
        if let Ok(package) =
          snapshot.resolve_package_from_deno_module(&reference.req)
        {
          if !self.packages.contains_key(&package.id) {
            self.resolved_reqs.insert(reference.req, package.id.clone());
            self.fill_for_package(package, &snapshot);
          }
        }
      }
    }

    let root_specifier = self.graph.resolve(&self.graph.roots[0].0);
    match self.graph.try_get(&root_specifier) {
      Ok(Some(root)) => {
        if let Some(cache_info) = root.maybe_cache_info.as_ref() {
          if let Some(local) = &cache_info.local {
            writeln!(
              self,
              "{} {}",
              colors::bold("local:"),
              local.to_string_lossy()
            )?;
          }
          if let Some(emit) = &cache_info.emit {
            writeln!(
              self,
              "{} {}",
              colors::bold("emit:"),
              emit.to_string_lossy()
            )?;
          }
          if let Some(map) = &cache_info.map {
            writeln!(
              self,
              "{} {}",
              colors::bold("map:"),
              map.to_string_lossy()
            )?;
          }
        }
        writeln!(self, "{} {}", colors::bold("type:"), root.media_type)?;
        let modules = self.graph.modules();
        let total_size = modules.iter().map(|m| m.size() as f64).sum::<f64>()
          + self.package_sizes.values().map(|s| *s as f64).sum::<f64>();
        let dep_count =
          modules.len() - 1 + self.packages.len() - self.resolved_reqs.len();
        writeln!(
          self,
          "{} {} unique {}",
          colors::bold("dependencies:"),
          dep_count,
          colors::gray(format!("(total {})", display::human_size(total_size)))
        )?;
        writeln!(self, "\n")?;
        let mut root_node = TreeNode::from_text(format!(
          "{} {}",
          root_specifier,
          colors::gray(format!(
            "({})",
            display::human_size(root.size() as f64)
          ))
        ));
        for dep in root.dependencies.values() {
          root_node.children.extend(self.fmt_dep_info(dep));
        }
        root_node.write(&mut self)?;
        Ok(())
      }
      Err(ModuleGraphError::Missing(_)) => {
        writeln!(self, "{} module could not be found", colors::red("error:"))
      }
      Err(err) => {
        writeln!(self, "{} {}", colors::red("error:"), err)
      }
      Ok(None) => {
        writeln!(self, "{} an internal error occurred", colors::red("error:"))
      }
    }
  }

  fn fmt_dep_info(&mut self, dep: &Dependency) -> Vec<TreeNode> {
    let mut children = Vec::with_capacity(2);
    if !dep.maybe_code.is_none() {
      if let Some(child) = self.fmt_resolved_info(&dep.maybe_code, false) {
        children.push(child);
      }
    }
    if !dep.maybe_type.is_none() {
      if let Some(child) = self.fmt_resolved_info(&dep.maybe_type, true) {
        children.push(child);
      }
    }
    children
  }

  fn fmt_module_info(&mut self, module: &Module, type_dep: bool) -> TreeNode {
    enum PackageOrSpecifier {
      Package(NpmResolutionPackage),
      Specifier(ModuleSpecifier),
    }
    use PackageOrSpecifier::*;

    let package_or_specifier =
      match NpmPackageReference::from_specifier(&module.specifier)
        .ok()
        .and_then(|package_ref| self.resolved_reqs.get(&package_ref.req))
        .and_then(|id| self.packages.get(id))
      {
        Some(package) => Package(package.clone()),
        None => Specifier(module.specifier.clone()),
      };
    let seen_key = match &package_or_specifier {
      Package(package) => package.id.to_string(),
      Specifier(specifier) => specifier.to_string(),
    };
    let was_seen = self.seen.contains(&seen_key);
    let specifier_str = if was_seen {
      let specifier_str = if type_dep {
        colors::italic_gray(&module.specifier).to_string()
      } else {
        colors::gray(&module.specifier).to_string()
      };
      format!("{} {}", specifier_str, colors::gray("*"))
    } else {
      let specifier_str = if type_dep {
        colors::italic(&module.specifier).to_string()
      } else {
        module.specifier.to_string()
      };
      let specifier_str = match &package_or_specifier {
        Package(package) => {
          format!("{} - {}", specifier_str, package.id.version)
        }
        Specifier(_) => specifier_str,
      };
      let maybe_size = match &package_or_specifier {
        Package(package) => {
          self.package_sizes.get(&package.id).map(|s| *s as usize)
        }
        Specifier(_) => {
          module.maybe_source.as_ref().map(|s| s.as_bytes().len())
        }
      };
      let size_str = colors::gray(format!(
        "({})",
        match maybe_size {
          Some(size) => display::human_size(size as f64),
          None => "unknown".to_string(),
        }
      ));
      format!("{} {}", specifier_str, size_str)
    };

    self.seen.insert(seen_key);

    let mut tree_node = TreeNode::from_text(specifier_str);

    if !was_seen {
      if let Some((_, type_dep)) = &module.maybe_types_dependency {
        if let Some(child) = self.fmt_resolved_info(type_dep, true) {
          tree_node.children.push(child);
        }
      }
      match &package_or_specifier {
        Package(package) => {
          tree_node.children.extend(self.fmt_npm_deps(package));
        }
        Specifier(_) => {
          for dep in module.dependencies.values() {
            tree_node.children.extend(self.fmt_dep_info(dep));
          }
        }
      }
    }
    tree_node
  }

  fn fmt_npm_deps(&mut self, package: &NpmResolutionPackage) -> Vec<TreeNode> {
    let mut deps = package.dependencies.values().collect::<Vec<_>>();
    deps.sort();
    let mut children = Vec::with_capacity(deps.len());
    for dep_id in deps.into_iter() {
      let maybe_size = self.package_sizes.get(dep_id).cloned();
      // todo: re-use
      let size_str = colors::gray(format!(
        "({})",
        match maybe_size {
          Some(size) => display::human_size(size as f64),
          None => "unknown".to_string(),
        }
      ));

      let mut child =
        TreeNode::from_text(format!("npm:{} {}", dep_id, size_str));
      if let Some(package) = self.packages.get(dep_id) {
        if !package.dependencies.is_empty() {
          if self.seen.contains(&package.id.to_string()) {
            child.text = format!("{} {}", child.text, colors::gray("*"));
          } else {
            let package = package.clone();
            child.children.extend(self.fmt_npm_deps(&package));
          }
        }
      }
      children.push(child);
    }
    children
  }

  fn fmt_error_info(
    &mut self,
    err: &ModuleGraphError,
    specifier: &ModuleSpecifier,
  ) -> TreeNode {
    self.seen.insert(specifier.to_string());
    match err {
      ModuleGraphError::InvalidSource(_, _) => {
        self.fmt_error_msg(specifier, "(invalid source)")
      }
      ModuleGraphError::InvalidTypeAssertion { .. } => {
        self.fmt_error_msg(specifier, "(invalid import assertion)")
      }
      ModuleGraphError::LoadingErr(_, _) => {
        self.fmt_error_msg(specifier, "(loading error)")
      }
      ModuleGraphError::ParseErr(_, _) => {
        self.fmt_error_msg(specifier, "(parsing error)")
      }
      ModuleGraphError::ResolutionError(_) => {
        self.fmt_error_msg(specifier, "(resolution error)")
      }
      ModuleGraphError::UnsupportedImportAssertionType(_, _) => {
        self.fmt_error_msg(specifier, "(unsupported import assertion)")
      }
      ModuleGraphError::UnsupportedMediaType(_, _) => {
        self.fmt_error_msg(specifier, "(unsupported)")
      }
      ModuleGraphError::Missing(_) => {
        self.fmt_error_msg(specifier, "(missing)")
      }
    }
  }

  fn fmt_error_msg(
    &mut self,
    specifier: &ModuleSpecifier,
    error_msg: &str,
  ) -> TreeNode {
    TreeNode::from_text(format!(
      "{} {}",
      colors::red(specifier),
      colors::red_bold(error_msg)
    ))
  }

  fn fmt_resolved_info(
    &mut self,
    resolved: &Resolved,
    type_dep: bool,
  ) -> Option<TreeNode> {
    match resolved {
      Resolved::Ok { specifier, .. } => {
        let resolved_specifier = self.graph.resolve(specifier);
        Some(match self.graph.try_get(&resolved_specifier) {
          Ok(Some(module)) => self.fmt_module_info(module, type_dep),
          Err(err) => self.fmt_error_info(&err, &resolved_specifier),
          Ok(None) => TreeNode::from_text(format!(
            "{} {}",
            colors::red(specifier),
            colors::red_bold("(missing)")
          )),
        })
      }
      Resolved::Err(err) => Some(TreeNode::from_text(format!(
        "{} {}",
        colors::italic(err.to_string()),
        colors::red_bold("(resolve error)")
      ))),
      _ => None,
    }
  }
}
