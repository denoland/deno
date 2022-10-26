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
      fmt_module_graph(&graph, &mut output)?;
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

/// A function that converts a float to a string the represents a human
/// readable version of that number.
fn human_size(size: f64) -> String {
  let negative = if size.is_sign_positive() { "" } else { "-" };
  let size = size.abs();
  let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  if size < 1_f64 {
    return format!("{}{}{}", negative, size, "B");
  }
  let delimiter = 1024_f64;
  let exponent = std::cmp::min(
    (size.ln() / delimiter.ln()).floor() as i32,
    (units.len() - 1) as i32,
  );
  let pretty_bytes = format!("{:.2}", size / delimiter.powi(exponent))
    .parse::<f64>()
    .unwrap()
    * 1_f64;
  let unit = units[exponent as usize];
  format!("{}{}{}", negative, pretty_bytes, unit)
}

fn fmt_module_graph(graph: &ModuleGraph, f: &mut impl Write) -> fmt::Result {
  if graph.roots.is_empty() || graph.roots.len() > 1 {
    return writeln!(
      f,
      "{} displaying graphs that have multiple roots is not supported.",
      colors::red("error:")
    );
  }
  let root_specifier = graph.resolve(&graph.roots[0].0);
  match graph.try_get(&root_specifier) {
    Ok(Some(root)) => {
      if let Some(cache_info) = root.maybe_cache_info.as_ref() {
        if let Some(local) = &cache_info.local {
          writeln!(
            f,
            "{} {}",
            colors::bold("local:"),
            local.to_string_lossy()
          )?;
        }
        if let Some(emit) = &cache_info.emit {
          writeln!(f, "{} {}", colors::bold("emit:"), emit.to_string_lossy())?;
        }
        if let Some(map) = &cache_info.map {
          writeln!(f, "{} {}", colors::bold("map:"), map.to_string_lossy())?;
        }
      }
      writeln!(f, "{} {}", colors::bold("type:"), root.media_type)?;
      let modules = graph.modules();
      let total_size: f64 = modules.iter().map(|m| m.size() as f64).sum();
      let dep_count = modules.len() - 1;
      writeln!(
        f,
        "{} {} unique {}",
        colors::bold("dependencies:"),
        dep_count,
        colors::gray(format!("(total {})", human_size(total_size)))
      )?;
      writeln!(
        f,
        "\n{} {}",
        root_specifier,
        colors::gray(format!("({})", human_size(root.size() as f64)))
      )?;
      let mut seen = HashSet::new();
      let dep_len = root.dependencies.len();
      for (idx, (_, dep)) in root.dependencies.iter().enumerate() {
        fmt_dep_info(
          dep,
          f,
          "",
          idx == dep_len - 1 && root.maybe_types_dependency.is_none(),
          graph,
          &mut seen,
        )?;
      }
      Ok(())
    }
    Err(ModuleGraphError::Missing(_)) => {
      writeln!(f, "{} module could not be found", colors::red("error:"))
    }
    Err(err) => {
      writeln!(f, "{} {}", colors::red("error:"), err)
    }
    Ok(None) => {
      writeln!(f, "{} an internal error occurred", colors::red("error:"))
    }
  }
}

fn fmt_dep_info<S: AsRef<str> + fmt::Display + Clone>(
  dep: &Dependency,
  f: &mut impl Write,
  prefix: S,
  last: bool,
  graph: &ModuleGraph,
  seen: &mut HashSet<ModuleSpecifier>,
) -> fmt::Result {
  if !dep.maybe_code.is_none() {
    fmt_resolved_info(
      &dep.maybe_code,
      f,
      prefix.clone(),
      dep.maybe_type.is_none() && last,
      graph,
      false,
      seen,
    )?;
  }
  if !dep.maybe_type.is_none() {
    fmt_resolved_info(&dep.maybe_type, f, prefix, last, graph, true, seen)?;
  }
  Ok(())
}

fn fmt_module_info<S: AsRef<str> + fmt::Display + Clone>(
  module: &Module,
  f: &mut impl Write,
  prefix: S,
  last: bool,
  graph: &ModuleGraph,
  type_dep: bool,
  seen: &mut HashSet<ModuleSpecifier>,
) -> fmt::Result {
  let was_seen = seen.contains(&module.specifier);
  let children = !((module.dependencies.is_empty()
    && module.maybe_types_dependency.is_none())
    || was_seen);
  let (specifier_str, size_str) = if was_seen {
    let specifier_str = if type_dep {
      colors::italic_gray(&module.specifier).to_string()
    } else {
      colors::gray(&module.specifier).to_string()
    };
    (specifier_str, colors::gray(" *").to_string())
  } else {
    let specifier_str = if type_dep {
      colors::italic(&module.specifier).to_string()
    } else {
      module.specifier.to_string()
    };
    let size_str =
      colors::gray(format!(" ({})", human_size(module.size() as f64)))
        .to_string();
    (specifier_str, size_str)
  };

  seen.insert(module.specifier.clone());

  fmt_info_msg(
    f,
    prefix.clone(),
    last,
    children,
    format!("{}{}", specifier_str, size_str),
  )?;

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
      fmt_resolved_info(type_dep, f, &prefix, dep_len == 0, graph, true, seen)?;
    }
    for (idx, (_, dep)) in module.dependencies.iter().enumerate() {
      fmt_dep_info(
        dep,
        f,
        &prefix,
        idx == dep_len - 1 && module.maybe_types_dependency.is_none(),
        graph,
        seen,
      )?;
    }
  }
  Ok(())
}

fn fmt_error_info<S: AsRef<str> + fmt::Display + Clone>(
  err: &ModuleGraphError,
  f: &mut impl Write,
  prefix: S,
  last: bool,
  specifier: &ModuleSpecifier,
  seen: &mut HashSet<ModuleSpecifier>,
) -> fmt::Result {
  seen.insert(specifier.clone());
  match err {
    ModuleGraphError::InvalidSource(_, _) => {
      fmt_error_msg(f, prefix, last, specifier, "(invalid source)")
    }
    ModuleGraphError::InvalidTypeAssertion { .. } => {
      fmt_error_msg(f, prefix, last, specifier, "(invalid import assertion)")
    }
    ModuleGraphError::LoadingErr(_, _) => {
      fmt_error_msg(f, prefix, last, specifier, "(loading error)")
    }
    ModuleGraphError::ParseErr(_, _) => {
      fmt_error_msg(f, prefix, last, specifier, "(parsing error)")
    }
    ModuleGraphError::ResolutionError(_) => {
      fmt_error_msg(f, prefix, last, specifier, "(resolution error)")
    }
    ModuleGraphError::UnsupportedImportAssertionType(_, _) => fmt_error_msg(
      f,
      prefix,
      last,
      specifier,
      "(unsupported import assertion)",
    ),
    ModuleGraphError::UnsupportedMediaType(_, _) => {
      fmt_error_msg(f, prefix, last, specifier, "(unsupported)")
    }
    ModuleGraphError::Missing(_) => {
      fmt_error_msg(f, prefix, last, specifier, "(missing)")
    }
  }
}

fn fmt_info_msg<S, M>(
  f: &mut impl Write,
  prefix: S,
  last: bool,
  children: bool,
  msg: M,
) -> fmt::Result
where
  S: AsRef<str> + fmt::Display + Clone,
  M: AsRef<str> + fmt::Display,
{
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
    f,
    "{} {}",
    colors::gray(format!(
      "{}{}─{}",
      prefix, sibling_connector, child_connector
    )),
    msg
  )
}

fn fmt_error_msg<S, M>(
  f: &mut impl Write,
  prefix: S,
  last: bool,
  specifier: &ModuleSpecifier,
  error_msg: M,
) -> fmt::Result
where
  S: AsRef<str> + fmt::Display + Clone,
  M: AsRef<str> + fmt::Display,
{
  fmt_info_msg(
    f,
    prefix,
    last,
    false,
    format!("{} {}", colors::red(specifier), colors::red_bold(error_msg)),
  )
}

fn fmt_resolved_info<S: AsRef<str> + fmt::Display + Clone>(
  resolved: &Resolved,
  f: &mut impl Write,
  prefix: S,
  last: bool,
  graph: &ModuleGraph,
  type_dep: bool,
  seen: &mut HashSet<ModuleSpecifier>,
) -> fmt::Result {
  match resolved {
    Resolved::Ok { specifier, .. } => {
      let resolved_specifier = graph.resolve(specifier);
      match graph.try_get(&resolved_specifier) {
        Ok(Some(module)) => {
          fmt_module_info(module, f, prefix, last, graph, type_dep, seen)
        }
        Err(err) => {
          fmt_error_info(&err, f, prefix, last, &resolved_specifier, seen)
        }
        Ok(None) => fmt_info_msg(
          f,
          prefix,
          last,
          false,
          format!(
            "{} {}",
            colors::red(specifier),
            colors::red_bold("(missing)")
          ),
        ),
      }
    }
    Resolved::Err(err) => fmt_info_msg(
      f,
      prefix,
      last,
      false,
      format!(
        "{} {}",
        colors::italic(err.to_string()),
        colors::red_bold("(resolve error)")
      ),
    ),
    _ => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use deno_graph::source::CacheInfo;
  use deno_graph::source::MemoryLoader;
  use deno_graph::source::Source;
  use deno_graph::DefaultModuleAnalyzer;
  use test_util::strip_ansi_codes;

  use super::*;
  use std::path::PathBuf;

  #[tokio::test]
  async fn test_info_graph() {
    let mut loader = MemoryLoader::new(
      vec![
        (
          "https://deno.land/x/example/a.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/a.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"import * as b from "./b.ts";
            import type { F } from "./f.d.ts";
            import * as g from "./g.js";
            "#,
          },
        ),
        (
          "https://deno.land/x/example/b.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/b.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"
            // @deno-types="./c.d.ts"
            import * as c from "./c.js";
            import * as d from "./d.ts";"#,
          },
        ),
        (
          "https://deno.land/x/example/c.js",
          Source::Module {
            specifier: "https://deno.land/x/example/c.js",
            maybe_headers: Some(vec![(
              "content-type",
              "application/javascript",
            )]),
            content: r#"export const c = "c";"#,
          },
        ),
        (
          "https://deno.land/x/example/c.d.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/c.d.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"export const c: "c";"#,
          },
        ),
        (
          "https://deno.land/x/example/d.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/d.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"import * as e from "./e.ts";
            export const d = "d";"#,
          },
        ),
        (
          "https://deno.land/x/example/e.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/e.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"import * as b from "./b.ts";
            export const e = "e";"#,
          },
        ),
        (
          "https://deno.land/x/example/f.d.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/f.d.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"export interface F { }"#,
          },
        ),
        (
          "https://deno.land/x/example/g.js",
          Source::Module {
            specifier: "https://deno.land/x/example/g.js",
            maybe_headers: Some(vec![
              ("content-type", "application/javascript"),
              ("x-typescript-types", "./g.d.ts"),
            ]),
            content: r#"export const g = "g";"#,
          },
        ),
        (
          "https://deno.land/x/example/g.d.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/g.d.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"export const g: "g";"#,
          },
        ),
      ],
      vec![(
        "https://deno.land/x/example/a.ts",
        CacheInfo {
          local: Some(PathBuf::from(
            "/cache/deps/https/deno.land/x/example/a.ts",
          )),
          emit: Some(PathBuf::from(
            "/cache/deps/https/deno.land/x/example/a.js",
          )),
          ..Default::default()
        },
      )],
    );
    let root_specifier =
      ModuleSpecifier::parse("https://deno.land/x/example/a.ts").unwrap();
    let module_analyzer = DefaultModuleAnalyzer::default();
    let builder = Builder::new(
      vec![(root_specifier, ModuleKind::Esm)],
      false,
      &mut loader,
      None,
      None,
      &module_analyzer,
      None,
    );
    let graph = builder.build(BuildKind::All, None).await;
    let mut output = String::new();
    fmt_module_graph(&graph, &mut output).unwrap();
    assert_eq!(
      strip_ansi_codes(&output),
      r#"local: /cache/deps/https/deno.land/x/example/a.ts
emit: /cache/deps/https/deno.land/x/example/a.js
type: TypeScript
dependencies: 8 unique (total 477B)

https://deno.land/x/example/a.ts (129B)
├─┬ https://deno.land/x/example/b.ts (120B)
│ ├── https://deno.land/x/example/c.js (21B)
│ ├── https://deno.land/x/example/c.d.ts (20B)
│ └─┬ https://deno.land/x/example/d.ts (62B)
│   └─┬ https://deno.land/x/example/e.ts (62B)
│     └── https://deno.land/x/example/b.ts *
├── https://deno.land/x/example/f.d.ts (22B)
└─┬ https://deno.land/x/example/g.js (21B)
  └── https://deno.land/x/example/g.d.ts (20B)
"#
    );
  }

  #[tokio::test]
  async fn test_info_graph_import_assertion() {
    let mut loader = MemoryLoader::new(
      vec![
        (
          "https://deno.land/x/example/a.ts",
          Source::Module {
            specifier: "https://deno.land/x/example/a.ts",
            maybe_headers: Some(vec![(
              "content-type",
              "application/typescript",
            )]),
            content: r#"import b from "./b.json" assert { type: "json" };
            const c = await import("./c.json", { assert: { type: "json" } });
            "#,
          },
        ),
        (
          "https://deno.land/x/example/b.json",
          Source::Module {
            specifier: "https://deno.land/x/example/b.json",
            maybe_headers: Some(vec![("content-type", "application/json")]),
            content: r#"{"b":"c"}"#,
          },
        ),
        (
          "https://deno.land/x/example/c.json",
          Source::Module {
            specifier: "https://deno.land/x/example/c.json",
            maybe_headers: Some(vec![("content-type", "application/json")]),
            content: r#"{"c":1}"#,
          },
        ),
      ],
      vec![(
        "https://deno.land/x/example/a.ts",
        CacheInfo {
          local: Some(PathBuf::from(
            "/cache/deps/https/deno.land/x/example/a.ts",
          )),
          emit: Some(PathBuf::from(
            "/cache/deps/https/deno.land/x/example/a.js",
          )),
          ..Default::default()
        },
      )],
    );
    let root_specifier =
      ModuleSpecifier::parse("https://deno.land/x/example/a.ts").unwrap();
    let module_analyzer = DefaultModuleAnalyzer::default();
    let builder = Builder::new(
      vec![(root_specifier, ModuleKind::Esm)],
      false,
      &mut loader,
      None,
      None,
      &module_analyzer,
      None,
    );
    let graph = builder.build(BuildKind::All, None).await;
    let mut output = String::new();
    fmt_module_graph(&graph, &mut output).unwrap();
    assert_eq!(
      strip_ansi_codes(&output),
      r#"local: /cache/deps/https/deno.land/x/example/a.ts
emit: /cache/deps/https/deno.land/x/example/a.js
type: TypeScript
dependencies: 2 unique (total 156B)

https://deno.land/x/example/a.ts (140B)
├── https://deno.land/x/example/b.json (9B)
└── https://deno.land/x/example/c.json (7B)
"#
    );
  }
}
