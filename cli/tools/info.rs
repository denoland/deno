// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_graph::Dependency;
use deno_graph::GraphKind;
use deno_graph::Module;
use deno_graph::ModuleError;
use deno_graph::ModuleGraph;
use deno_graph::Resolution;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_terminal::colors;

use crate::args::Flags;
use crate::args::InfoFlags;
use crate::display;
use crate::factory::CliFactory;
use crate::graph_util::graph_exit_integrity_errors;
use crate::npm::CliNpmResolver;
use crate::npm::ManagedCliNpmResolver;
use crate::util::checksum;

const JSON_SCHEMA_VERSION: u8 = 1;

pub async fn info(
  flags: Arc<Flags>,
  info_flags: InfoFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  if let Some(specifier) = info_flags.file {
    let module_graph_builder = factory.module_graph_builder().await?;
    let module_graph_creator = factory.module_graph_creator().await?;
    let npm_resolver = factory.npm_resolver().await?;
    let maybe_lockfile = cli_options.maybe_lockfile();
    let resolver = factory.workspace_resolver().await?;

    let maybe_import_specifier =
      if let Some(import_map) = resolver.maybe_import_map() {
        if let Ok(imports_specifier) =
          import_map.resolve(&specifier, import_map.base_url())
        {
          Some(imports_specifier)
        } else {
          None
        }
      } else {
        None
      };

    let specifier = match maybe_import_specifier {
      Some(specifier) => specifier,
      None => resolve_url_or_path(&specifier, cli_options.initial_cwd())?,
    };

    let mut loader = module_graph_builder.create_graph_loader();
    loader.enable_loading_cache_info(); // for displaying the cache information
    let graph = module_graph_creator
      .create_graph_with_loader(GraphKind::All, vec![specifier], &mut loader)
      .await?;

    // write out the lockfile if there is one
    if let Some(lockfile) = &maybe_lockfile {
      graph_exit_integrity_errors(&graph);
      lockfile.write_if_changed()?;
    }

    if info_flags.json {
      let mut json_graph = serde_json::json!(graph);
      if let Some(output) = json_graph.as_object_mut() {
        output.shift_insert(
          0,
          "version".to_string(),
          JSON_SCHEMA_VERSION.into(),
        );
      }
      add_npm_packages_to_json(&mut json_graph, npm_resolver.as_ref());
      display::write_json_to_stdout(&json_graph)?;
    } else {
      let mut output = String::new();
      GraphDisplayContext::write(&graph, npm_resolver.as_ref(), &mut output)?;
      display::write_to_stdout_ignore_sigpipe(output.as_bytes())?;
    }
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(
      &factory,
      info_flags.json,
      cli_options.location_flag().as_ref(),
    )?;
  }
  Ok(())
}

#[allow(clippy::print_stdout)]
fn print_cache_info(
  factory: &CliFactory,
  json: bool,
  location: Option<&deno_core::url::Url>,
) -> Result<(), AnyError> {
  let dir = factory.deno_dir()?;
  #[allow(deprecated)]
  let modules_cache = factory.global_http_cache()?.get_global_cache_location();
  let npm_cache = factory.deno_dir()?.npm_folder_path();
  let typescript_cache = &dir.gen_cache.location;
  let registry_cache = dir.registries_folder_path();
  let mut origin_dir = dir.origin_data_folder_path();
  let deno_dir = dir.root_path_for_display().to_string();

  if let Some(location) = &location {
    origin_dir =
      origin_dir.join(checksum::gen(&[location.to_string().as_bytes()]));
  }

  let local_storage_dir = origin_dir.join("local_storage");

  if json {
    let mut json_output = serde_json::json!({
      "version": JSON_SCHEMA_VERSION,
      "denoDir": deno_dir,
      "modulesCache": modules_cache,
      "npmCache": npm_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      json_output["localStorage"] = serde_json::to_value(local_storage_dir)?;
    }

    display::write_json_to_stdout(&json_output)
  } else {
    println!("{} {}", colors::bold("DENO_DIR location:"), deno_dir);
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

fn add_npm_packages_to_json(
  json: &mut serde_json::Value,
  npm_resolver: &dyn CliNpmResolver,
) {
  let Some(npm_resolver) = npm_resolver.as_managed() else {
    return; // does not include byonm to deno info's output
  };

  // ideally deno_graph could handle this, but for now we just modify the json here
  let snapshot = npm_resolver.snapshot();
  let json = json.as_object_mut().unwrap();
  let modules = json.get_mut("modules").and_then(|m| m.as_array_mut());
  if let Some(modules) = modules {
    if modules.len() == 1
      && modules[0].get("kind").and_then(|k| k.as_str()) == Some("npm")
    {
      // If there is only one module and it's "external", then that means
      // someone provided an npm specifier as a cli argument. In this case,
      // we want to show which npm package the cli argument resolved to.
      let module = &mut modules[0];
      let maybe_package = module
        .get("specifier")
        .and_then(|k| k.as_str())
        .and_then(|specifier| NpmPackageNvReference::from_str(specifier).ok())
        .and_then(|package_ref| {
          snapshot
            .resolve_package_from_deno_module(package_ref.nv())
            .ok()
        });
      if let Some(pkg) = maybe_package {
        if let Some(module) = module.as_object_mut() {
          module
            .insert("npmPackage".to_string(), pkg.id.as_serialized().into());
        }
      }
    } else {
      // Filter out npm package references from the modules and instead
      // have them only listed as dependencies. This is done because various
      // npm specifiers modules in the graph are really just unresolved
      // references. So there could be listed multiple npm specifiers
      // that would resolve to a single npm package.
      for i in (0..modules.len()).rev() {
        if matches!(
          modules[i].get("kind").and_then(|k| k.as_str()),
          Some("npm") | Some("external")
        ) {
          modules.remove(i);
        }
      }
    }

    for module in modules.iter_mut() {
      let dependencies = module
        .get_mut("dependencies")
        .and_then(|d| d.as_array_mut());
      if let Some(dependencies) = dependencies {
        for dep in dependencies.iter_mut() {
          if let serde_json::Value::Object(dep) = dep {
            let specifier = dep.get("specifier").and_then(|s| s.as_str());
            if let Some(specifier) = specifier {
              if let Ok(npm_ref) = NpmPackageReqReference::from_str(specifier) {
                if let Ok(pkg) =
                  snapshot.resolve_pkg_from_pkg_req(npm_ref.req())
                {
                  dep.insert(
                    "npmPackage".to_string(),
                    pkg.id.as_serialized().into(),
                  );
                }
              }
            }
          }
        }
      }
    }
  }

  let mut sorted_packages =
    snapshot.all_packages_for_every_system().collect::<Vec<_>>();
  sorted_packages.sort_by(|a, b| a.id.cmp(&b.id));
  let mut json_packages = serde_json::Map::with_capacity(sorted_packages.len());
  for pkg in sorted_packages {
    let mut kv = serde_json::Map::new();
    kv.insert("name".to_string(), pkg.id.nv.name.to_string().into());
    kv.insert("version".to_string(), pkg.id.nv.version.to_string().into());
    let mut deps = pkg.dependencies.values().collect::<Vec<_>>();
    deps.sort();
    let deps = deps
      .into_iter()
      .map(|id| serde_json::Value::String(id.as_serialized()))
      .collect::<Vec<_>>();
    kv.insert("dependencies".to_string(), deps.into());

    json_packages.insert(pkg.id.as_serialized(), kv.into());
  }

  json.insert("npmPackages".to_string(), json_packages.into());
}

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
}

fn print_tree_node<TWrite: Write>(
  tree_node: &TreeNode,
  writer: &mut TWrite,
) -> fmt::Result {
  fn print_children<TWrite: Write>(
    writer: &mut TWrite,
    prefix: &str,
    children: &[TreeNode],
  ) -> fmt::Result {
    const SIBLING_CONNECTOR: char = '├';
    const LAST_SIBLING_CONNECTOR: char = '└';
    const CHILD_DEPS_CONNECTOR: char = '┬';
    const CHILD_NO_DEPS_CONNECTOR: char = '─';
    const VERTICAL_CONNECTOR: char = '│';
    const EMPTY_CONNECTOR: char = ' ';

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
        colors::gray(format!("{prefix}{sibling_connector}─{child_connector}")),
        child.text
      )?;
      let child_prefix = format!(
        "{}{}{}",
        prefix,
        if is_last {
          EMPTY_CONNECTOR
        } else {
          VERTICAL_CONNECTOR
        },
        EMPTY_CONNECTOR
      );
      print_children(writer, &child_prefix, &child.children)?;
    }

    Ok(())
  }

  writeln!(writer, "{}", tree_node.text)?;
  print_children(writer, "", &tree_node.children)?;
  Ok(())
}

/// Precached information about npm packages that are used in deno info.
#[derive(Default)]
struct NpmInfo {
  package_sizes: HashMap<NpmPackageId, u64>,
  resolved_ids: HashMap<PackageNv, NpmPackageId>,
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
}

impl NpmInfo {
  pub fn build<'a>(
    graph: &'a ModuleGraph,
    npm_resolver: &'a ManagedCliNpmResolver,
    npm_snapshot: &'a NpmResolutionSnapshot,
  ) -> Self {
    let mut info = NpmInfo::default();
    if graph.npm_packages.is_empty() {
      return info; // skip going over the modules if there's no npm packages
    }

    for module in graph.modules() {
      if let Module::Npm(module) = module {
        let nv = module.nv_reference.nv();
        if let Ok(package) = npm_snapshot.resolve_package_from_deno_module(nv) {
          info.resolved_ids.insert(nv.clone(), package.id.clone());
          if !info.packages.contains_key(&package.id) {
            info.fill_package_info(package, npm_resolver, npm_snapshot);
          }
        }
      }
    }

    info
  }

  fn fill_package_info<'a>(
    &mut self,
    package: &NpmResolutionPackage,
    npm_resolver: &'a ManagedCliNpmResolver,
    npm_snapshot: &'a NpmResolutionSnapshot,
  ) {
    self.packages.insert(package.id.clone(), package.clone());
    if let Ok(size) = npm_resolver.package_size(&package.id) {
      self.package_sizes.insert(package.id.clone(), size);
    }
    for id in package.dependencies.values() {
      if !self.packages.contains_key(id) {
        if let Some(package) = npm_snapshot.package_from_id(id) {
          self.fill_package_info(package, npm_resolver, npm_snapshot);
        }
      }
    }
  }

  pub fn resolve_package(
    &self,
    nv: &PackageNv,
  ) -> Option<&NpmResolutionPackage> {
    let id = self.resolved_ids.get(nv)?;
    self.packages.get(id)
  }
}

struct GraphDisplayContext<'a> {
  graph: &'a ModuleGraph,
  npm_info: NpmInfo,
  seen: HashSet<String>,
}

impl<'a> GraphDisplayContext<'a> {
  pub fn write<TWrite: Write>(
    graph: &'a ModuleGraph,
    npm_resolver: &'a dyn CliNpmResolver,
    writer: &mut TWrite,
  ) -> Result<(), AnyError> {
    let npm_info = match npm_resolver.as_managed() {
      Some(npm_resolver) => {
        let npm_snapshot = npm_resolver.snapshot();
        NpmInfo::build(graph, npm_resolver, &npm_snapshot)
      }
      None => NpmInfo::default(),
    };
    Self {
      graph,
      npm_info,
      seen: Default::default(),
    }
    .into_writer(writer)
  }

  fn into_writer<TWrite: Write>(
    mut self,
    writer: &mut TWrite,
  ) -> Result<(), AnyError> {
    if self.graph.roots.is_empty() || self.graph.roots.len() > 1 {
      bail!("displaying graphs that have multiple roots is not supported.");
    }

    let root_specifier = self.graph.resolve(&self.graph.roots[0]);
    match self.graph.try_get(root_specifier) {
      Ok(Some(root)) => {
        let maybe_cache_info = match root {
          Module::Js(module) => module.maybe_cache_info.as_ref(),
          Module::Json(module) => module.maybe_cache_info.as_ref(),
          Module::Node(_) | Module::Npm(_) | Module::External(_) => None,
        };
        if let Some(cache_info) = maybe_cache_info {
          if let Some(local) = &cache_info.local {
            writeln!(
              writer,
              "{} {}",
              colors::bold("local:"),
              local.to_string_lossy()
            )?;
          }
        }
        if let Some(module) = root.js() {
          writeln!(writer, "{} {}", colors::bold("type:"), module.media_type)?;
        }
        let total_modules_size = self
          .graph
          .modules()
          .map(|m| {
            let size = match m {
              Module::Js(module) => module.size(),
              Module::Json(module) => module.size(),
              Module::Node(_) | Module::Npm(_) | Module::External(_) => 0,
            };
            size as f64
          })
          .sum::<f64>();
        let total_npm_package_size = self
          .npm_info
          .package_sizes
          .values()
          .map(|s| *s as f64)
          .sum::<f64>();
        let total_size = total_modules_size + total_npm_package_size;
        let dep_count = self.graph.modules().count() - 1 // -1 for the root module
          + self.npm_info.packages.len()
          - self.npm_info.resolved_ids.len();
        writeln!(
          writer,
          "{} {} unique",
          colors::bold("dependencies:"),
          dep_count,
        )?;
        writeln!(
          writer,
          "{} {}",
          colors::bold("size:"),
          display::human_size(total_size),
        )?;
        writeln!(writer)?;
        let root_node = self.build_module_info(root, false);
        print_tree_node(&root_node, writer)?;
        Ok(())
      }
      Err(err) => {
        if let ModuleError::Missing(_, _) = *err {
          bail!("module could not be found");
        } else {
          bail!("{:#}", err);
        }
      }
      Ok(None) => {
        bail!("an internal error occurred");
      }
    }
  }

  fn build_dep_info(&mut self, dep: &Dependency) -> Vec<TreeNode> {
    let mut children = Vec::with_capacity(2);
    if !dep.maybe_code.is_none() {
      if let Some(child) = self.build_resolved_info(&dep.maybe_code, false) {
        children.push(child);
      }
    }
    if !dep.maybe_type.is_none() {
      if let Some(child) = self.build_resolved_info(&dep.maybe_type, true) {
        children.push(child);
      }
    }
    children
  }

  fn build_module_info(&mut self, module: &Module, type_dep: bool) -> TreeNode {
    enum PackageOrSpecifier {
      Package(NpmResolutionPackage),
      Specifier(ModuleSpecifier),
    }

    use PackageOrSpecifier::*;

    let package_or_specifier = match module.npm() {
      Some(npm) => match self.npm_info.resolve_package(npm.nv_reference.nv()) {
        Some(package) => Package(package.clone()),
        None => Specifier(module.specifier().clone()), // should never happen
      },
      None => Specifier(module.specifier().clone()),
    };
    let was_seen = !self.seen.insert(match &package_or_specifier {
      Package(package) => package.id.as_serialized(),
      Specifier(specifier) => specifier.to_string(),
    });
    let header_text = if was_seen {
      let specifier_str = if type_dep {
        colors::italic_gray(module.specifier()).to_string()
      } else {
        colors::gray(module.specifier()).to_string()
      };
      format!("{} {}", specifier_str, colors::gray("*"))
    } else {
      let header_text = if type_dep {
        colors::italic(module.specifier()).to_string()
      } else {
        module.specifier().to_string()
      };
      let maybe_size = match &package_or_specifier {
        Package(package) => {
          self.npm_info.package_sizes.get(&package.id).copied()
        }
        Specifier(_) => match module {
          Module::Js(module) => Some(module.size() as u64),
          Module::Json(module) => Some(module.size() as u64),
          Module::Node(_) | Module::Npm(_) | Module::External(_) => None,
        },
      };
      format!("{} {}", header_text, maybe_size_to_text(maybe_size))
    };

    let mut tree_node = TreeNode::from_text(header_text);

    if !was_seen {
      match &package_or_specifier {
        Package(package) => {
          tree_node.children.extend(self.build_npm_deps(package));
        }
        Specifier(_) => {
          if let Some(module) = module.js() {
            if let Some(types_dep) = &module.maybe_types_dependency {
              if let Some(child) =
                self.build_resolved_info(&types_dep.dependency, true)
              {
                tree_node.children.push(child);
              }
            }
            for dep in module.dependencies.values() {
              tree_node.children.extend(self.build_dep_info(dep));
            }
          }
        }
      }
    }
    tree_node
  }

  fn build_npm_deps(
    &mut self,
    package: &NpmResolutionPackage,
  ) -> Vec<TreeNode> {
    let mut deps = package.dependencies.values().collect::<Vec<_>>();
    deps.sort();
    let mut children = Vec::with_capacity(deps.len());
    for dep_id in deps.into_iter() {
      let maybe_size = self.npm_info.package_sizes.get(dep_id).cloned();
      let size_str = maybe_size_to_text(maybe_size);
      let mut child = TreeNode::from_text(format!(
        "npm:/{} {}",
        dep_id.as_serialized(),
        size_str
      ));
      if let Some(package) = self.npm_info.packages.get(dep_id) {
        if !package.dependencies.is_empty() {
          let was_seen = !self.seen.insert(package.id.as_serialized());
          if was_seen {
            child.text = format!("{} {}", child.text, colors::gray("*"));
          } else {
            let package = package.clone();
            child.children.extend(self.build_npm_deps(&package));
          }
        }
      }
      children.push(child);
    }
    children
  }

  fn build_error_info(
    &mut self,
    err: &ModuleError,
    specifier: &ModuleSpecifier,
  ) -> TreeNode {
    self.seen.insert(specifier.to_string());
    match err {
      ModuleError::InvalidTypeAssertion { .. } => {
        self.build_error_msg(specifier, "(invalid import attribute)")
      }
      ModuleError::LoadingErr(_, _, err) => {
        use deno_graph::ModuleLoadError::*;
        let message = match err {
          HttpsChecksumIntegrity(_) => "(checksum integrity error)",
          Decode(_) => "(loading decode error)",
          Loader(err) => match deno_core::error::get_custom_error_class(err) {
            Some("NotCapable") => "(not capable, requires --allow-import)",
            _ => "(loading error)",
          },
          Jsr(_) => "(loading error)",
          NodeUnknownBuiltinModule(_) => "(unknown node built-in error)",
          Npm(_) => "(npm loading error)",
          TooManyRedirects => "(too many redirects error)",
        };
        self.build_error_msg(specifier, message.as_ref())
      }
      ModuleError::ParseErr(_, _) => {
        self.build_error_msg(specifier, "(parsing error)")
      }
      ModuleError::UnsupportedImportAttributeType { .. } => {
        self.build_error_msg(specifier, "(unsupported import attribute)")
      }
      ModuleError::UnsupportedMediaType { .. } => {
        self.build_error_msg(specifier, "(unsupported)")
      }
      ModuleError::Missing(_, _) | ModuleError::MissingDynamic(_, _) => {
        self.build_error_msg(specifier, "(missing)")
      }
    }
  }

  fn build_error_msg(
    &self,
    specifier: &ModuleSpecifier,
    error_msg: &str,
  ) -> TreeNode {
    TreeNode::from_text(format!(
      "{} {}",
      colors::red(specifier),
      colors::red_bold(error_msg)
    ))
  }

  fn build_resolved_info(
    &mut self,
    resolution: &Resolution,
    type_dep: bool,
  ) -> Option<TreeNode> {
    match resolution {
      Resolution::Ok(resolved) => {
        let specifier = &resolved.specifier;
        let resolved_specifier = self.graph.resolve(specifier);
        Some(match self.graph.try_get(resolved_specifier) {
          Ok(Some(module)) => self.build_module_info(module, type_dep),
          Err(err) => self.build_error_info(err, resolved_specifier),
          Ok(None) => TreeNode::from_text(format!(
            "{} {}",
            colors::red(specifier),
            colors::red_bold("(missing)")
          )),
        })
      }
      Resolution::Err(err) => Some(TreeNode::from_text(format!(
        "{} {}",
        colors::italic(err.to_string()),
        colors::red_bold("(resolve error)")
      ))),
      _ => None,
    }
  }
}

fn maybe_size_to_text(maybe_size: Option<u64>) -> String {
  colors::gray(format!(
    "({})",
    match maybe_size {
      Some(size) => display::human_size(size as f64),
      None => "unknown".to_string(),
    }
  ))
  .to_string()
}
