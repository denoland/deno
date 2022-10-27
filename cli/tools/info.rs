// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
use crate::npm::NpmPackageResolver;
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
      let formatter = GraphDisplayFormatter {
        graph: &graph,
        npm_resolver: &ps.npm_resolver,
        seen: Default::default(),
        writer: &mut output,
      };
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

struct GraphDisplayFormatter<'a, TWrite: Write> {
  graph: &'a ModuleGraph,
  npm_resolver: &'a NpmPackageResolver,
  seen: HashSet<ModuleSpecifier>,
  writer: TWrite,
}

impl<'a, TWrite: Write> Write for GraphDisplayFormatter<'a, TWrite> {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    self.writer.write_str(s)
  }
}

impl<'a, TWrite: Write> GraphDisplayFormatter<'a, TWrite> {
  pub fn fmt_module_graph(mut self) -> fmt::Result {
    if self.graph.roots.is_empty() || self.graph.roots.len() > 1 {
      return writeln!(
        self,
        "{} displaying graphs that have multiple roots is not supported.",
        colors::red("error:")
      );
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
        let total_size: f64 = modules.iter().map(|m| m.size() as f64).sum();
        let dep_count = modules.len() - 1;
        writeln!(
          self,
          "{} {} unique {}",
          colors::bold("dependencies:"),
          dep_count,
          colors::gray(format!("(total {})", display::human_size(total_size)))
        )?;
        writeln!(
          self,
          "\n{} {}",
          root_specifier,
          colors::gray(format!(
            "({})",
            display::human_size(root.size() as f64)
          ))
        )?;
        let dep_len = root.dependencies.len();
        for (idx, (_, dep)) in root.dependencies.iter().enumerate() {
          self.fmt_dep_info(
            dep,
            "",
            idx == dep_len - 1 && root.maybe_types_dependency.is_none(),
          )?;
        }
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

  fn fmt_dep_info(
    &mut self,
    dep: &Dependency,
    prefix: &str,
    last: bool,
  ) -> fmt::Result {
    if !dep.maybe_code.is_none() {
      self.fmt_resolved_info(
        &dep.maybe_code,
        prefix,
        dep.maybe_type.is_none() && last,
        false,
      )?;
    }
    if !dep.maybe_type.is_none() {
      self.fmt_resolved_info(&dep.maybe_type, prefix, last, true)?;
    }
    Ok(())
  }

  fn fmt_module_info(
    &mut self,
    module: &Module,
    prefix: &str,
    last: bool,
    type_dep: bool,
  ) -> fmt::Result {
    let was_seen = self.seen.contains(&module.specifier);
    let children = !((module.dependencies.is_empty()
      && module.maybe_types_dependency.is_none())
      || was_seen);
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
      let size_str = colors::gray(format!(
        "({})",
        display::human_size(module.size() as f64)
      ));
      format!("{} {}", specifier_str, size_str)
    };

    self.seen.insert(module.specifier.clone());

    self.fmt_info_msg(prefix, last, children, &specifier_str)?;

    if !was_seen {
      let mut prefix = prefix.to_string();
      if last {
        prefix.push(EMPTY_CONNECTOR);
      } else {
        prefix.push(VERTICAL_CONNECTOR);
      }
      prefix.push(EMPTY_CONNECTOR);
      let dep_len = module.dependencies.len();
      if let Some((_, type_dep)) = &module.maybe_types_dependency {
        self.fmt_resolved_info(type_dep, &prefix, dep_len == 0, true)?;
      }
      for (idx, (_, dep)) in module.dependencies.iter().enumerate() {
        self.fmt_dep_info(
          dep,
          &prefix,
          idx == dep_len - 1 && module.maybe_types_dependency.is_none(),
        )?;
      }
    }
    Ok(())
  }

  fn fmt_error_info(
    &mut self,
    err: &ModuleGraphError,
    prefix: &str,
    last: bool,
    specifier: &ModuleSpecifier,
  ) -> fmt::Result {
    self.seen.insert(specifier.clone());
    match err {
      ModuleGraphError::InvalidSource(_, _) => {
        self.fmt_error_msg(prefix, last, specifier, "(invalid source)")
      }
      ModuleGraphError::InvalidTypeAssertion { .. } => self.fmt_error_msg(
        prefix,
        last,
        specifier,
        "(invalid import assertion)",
      ),
      ModuleGraphError::LoadingErr(_, _) => {
        self.fmt_error_msg(prefix, last, specifier, "(loading error)")
      }
      ModuleGraphError::ParseErr(_, _) => {
        self.fmt_error_msg(prefix, last, specifier, "(parsing error)")
      }
      ModuleGraphError::ResolutionError(_) => {
        self.fmt_error_msg(prefix, last, specifier, "(resolution error)")
      }
      ModuleGraphError::UnsupportedImportAssertionType(_, _) => self
        .fmt_error_msg(
          prefix,
          last,
          specifier,
          "(unsupported import assertion)",
        ),
      ModuleGraphError::UnsupportedMediaType(_, _) => {
        self.fmt_error_msg(prefix, last, specifier, "(unsupported)")
      }
      ModuleGraphError::Missing(_) => {
        self.fmt_error_msg(prefix, last, specifier, "(missing)")
      }
    }
  }

  fn fmt_info_msg(
    &mut self,
    prefix: &str,
    last: bool,
    children: bool,
    msg: &str,
  ) -> fmt::Result {
    let sibling_connector = if last {
      LAST_SIBLING_CONNECTOR
    } else {
      SIBLING_CONNECTOR
    };
    let child_connector = if children {
      CHILD_DEPS_CONNECTOR
    } else {
      CHILD_NO_DEPS_CONNECTOR
    };
    writeln!(
      self,
      "{} {}",
      colors::gray(format!(
        "{}{}─{}",
        prefix, sibling_connector, child_connector
      )),
      msg
    )
  }

  fn fmt_error_msg(
    &mut self,
    prefix: &str,
    last: bool,
    specifier: &ModuleSpecifier,
    error_msg: &str,
  ) -> fmt::Result {
    self.fmt_info_msg(
      prefix,
      last,
      false,
      &format!("{} {}", colors::red(specifier), colors::red_bold(error_msg)),
    )
  }

  fn fmt_resolved_info(
    &mut self,
    resolved: &Resolved,
    prefix: &str,
    last: bool,
    type_dep: bool,
  ) -> fmt::Result {
    match resolved {
      Resolved::Ok { specifier, .. } => {
        let resolved_specifier = self.graph.resolve(specifier);
        match self.graph.try_get(&resolved_specifier) {
          Ok(Some(module)) => {
            self.fmt_module_info(module, prefix, last, type_dep)
          }
          Err(err) => {
            self.fmt_error_info(&err, prefix, last, &resolved_specifier)
          }
          Ok(None) => self.fmt_info_msg(
            prefix,
            last,
            false,
            &format!(
              "{} {}",
              colors::red(specifier),
              colors::red_bold("(missing)")
            ),
          ),
        }
      }
      Resolved::Err(err) => self.fmt_info_msg(
        prefix,
        last,
        false,
        &format!(
          "{} {}",
          colors::italic(err.to_string()),
          colors::red_bold("(resolve error)")
        ),
      ),
      _ => Ok(()),
    }
  }
}
