// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::TextChange;
use deno_config::FmtOptionsConfig;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_node;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use jsonc_parser::ast::ObjectProp;
use jsonc_parser::ast::Value;

use crate::args::AddFlags;
use crate::args::CacheSetting;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

enum DenoConfigFormat {
  Json,
  Jsonc,
}

impl DenoConfigFormat {
  fn from_specifier(spec: &ModuleSpecifier) -> Result<Self, AnyError> {
    let file_name = spec
      .path_segments()
      .ok_or_else(|| anyhow!("Empty path in deno config specifier: {spec}"))?
      .last()
      .unwrap();
    match file_name {
      "deno.json" => Ok(Self::Json),
      "deno.jsonc" => Ok(Self::Jsonc),
      _ => bail!("Unsupported deno config file: {file_name}"),
    }
  }
}

enum DenoOrPackageJson {
  Deno(Arc<deno_config::ConfigFile>, DenoConfigFormat),
  Npm(Arc<deno_node::PackageJson>, Option<FmtOptionsConfig>),
}

impl DenoOrPackageJson {
  fn specifier(&self) -> Cow<ModuleSpecifier> {
    match self {
      Self::Deno(d, ..) => Cow::Borrowed(&d.specifier),
      Self::Npm(n, ..) => Cow::Owned(n.specifier()),
    }
  }

  /// Returns the existing imports/dependencies from the config.
  fn existing_imports(&self) -> Result<IndexMap<String, String>, AnyError> {
    match self {
      DenoOrPackageJson::Deno(deno, ..) => {
        if let Some(imports) = deno.json.imports.clone() {
          match serde_json::from_value(imports) {
            Ok(map) => Ok(map),
            Err(err) => {
              bail!("Malformed \"imports\" configuration: {err}")
            }
          }
        } else {
          Ok(Default::default())
        }
      }
      DenoOrPackageJson::Npm(npm, ..) => {
        Ok(npm.dependencies.clone().unwrap_or_default())
      }
    }
  }

  fn fmt_options(&self) -> FmtOptionsConfig {
    match self {
      DenoOrPackageJson::Deno(deno, ..) => deno
        .to_fmt_config()
        .ok()
        .map(|f| f.options)
        .unwrap_or_default(),
      DenoOrPackageJson::Npm(_, config) => config.clone().unwrap_or_default(),
    }
  }

  fn imports_key(&self) -> &'static str {
    match self {
      DenoOrPackageJson::Deno(..) => "imports",
      DenoOrPackageJson::Npm(..) => "dependencies",
    }
  }

  fn file_name(&self) -> &'static str {
    match self {
      DenoOrPackageJson::Deno(_, format) => match format {
        DenoConfigFormat::Json => "deno.json",
        DenoConfigFormat::Jsonc => "deno.jsonc",
      },
      DenoOrPackageJson::Npm(..) => "package.json",
    }
  }

  fn is_npm(&self) -> bool {
    matches!(self, Self::Npm(..))
  }

  /// Get the preferred config file to operate on
  /// given the flags. If no config file is present,
  /// creates a `deno.json` file - in this case
  /// we also return a new `CliFactory` that knows about
  /// the new config
  fn from_flags(flags: Flags) -> Result<(Self, CliFactory), AnyError> {
    let factory = CliFactory::from_flags(flags.clone())?;
    let options = factory.cli_options();
    let start_ctx = options.workspace.resolve_start_ctx();

    match (start_ctx.maybe_deno_json(), start_ctx.maybe_pkg_json()) {
      // when both are present, for now,
      // default to deno.json
      (Some(deno), Some(_) | None) => Ok((
        DenoOrPackageJson::Deno(
          deno.clone(),
          DenoConfigFormat::from_specifier(&deno.specifier)?,
        ),
        factory,
      )),
      (None, Some(package_json)) if options.enable_future_features() => {
        Ok((DenoOrPackageJson::Npm(package_json.clone(), None), factory))
      }
      (None, Some(_) | None) => {
        std::fs::write(options.initial_cwd().join("deno.json"), "{}\n")
          .context("Failed to create deno.json file")?;
        log::info!("Created deno.json configuration file.");
        let factory = CliFactory::from_flags(flags.clone())?;
        let options = factory.cli_options().clone();
        let start_ctx = options.workspace.resolve_start_ctx();
        Ok((
          DenoOrPackageJson::Deno(
            start_ctx.maybe_deno_json().cloned().ok_or_else(|| {
              anyhow!("config not found, but it was just created")
            })?,
            DenoConfigFormat::Json,
          ),
          factory,
        ))
      }
    }
  }
}

fn package_json_dependency_entry(
  selected: SelectedPackage,
) -> (String, String) {
  if let Some(npm_package) = selected.package_name.strip_prefix("npm:") {
    (npm_package.into(), selected.version_req)
  } else if let Some(jsr_package) = selected.package_name.strip_prefix("jsr:") {
    let jsr_package = jsr_package.strip_prefix('@').unwrap_or(jsr_package);
    let scope_replaced = jsr_package.replace('/', "__");
    let version_req =
      format!("npm:@jsr/{scope_replaced}@{}", selected.version_req);
    (selected.import_name, version_req)
  } else {
    (selected.package_name, selected.version_req)
  }
}

pub async fn add(flags: Flags, add_flags: AddFlags) -> Result<(), AnyError> {
  let (config_file, cli_factory) =
    DenoOrPackageJson::from_flags(flags.clone())?;

  let config_specifier = config_file.specifier();
  if config_specifier.scheme() != "file" {
    bail!("Can't add dependencies to a remote configuration file");
  }
  let config_file_path = config_specifier.to_file_path().unwrap();

  let http_client = cli_factory.http_client_provider();

  let mut selected_packages = Vec::with_capacity(add_flags.packages.len());
  let mut package_reqs = Vec::with_capacity(add_flags.packages.len());

  for entry_text in add_flags.packages.iter() {
    let req = AddPackageReq::parse(entry_text).with_context(|| {
      format!("Failed to parse package required: {}", entry_text)
    })?;

    package_reqs.push(req);
  }

  let deps_http_cache = cli_factory.global_http_cache()?;
  let mut deps_file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::ReloadAll,
    true,
    http_client.clone(),
    Default::default(),
    None,
  );
  deps_file_fetcher.set_download_log_level(log::Level::Trace);
  let deps_file_fetcher = Arc::new(deps_file_fetcher);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(deps_file_fetcher.clone()));
  let npm_resolver = Arc::new(NpmFetchResolver::new(deps_file_fetcher));

  let package_futures = package_reqs
    .into_iter()
    .map(move |package_req| {
      find_package_and_select_version_for_req(
        jsr_resolver.clone(),
        npm_resolver.clone(),
        package_req,
      )
      .boxed_local()
    })
    .collect::<Vec<_>>();

  let stream_of_futures = deno_core::futures::stream::iter(package_futures);
  let mut buffered = stream_of_futures.buffer_unordered(10);

  while let Some(package_and_version_result) = buffered.next().await {
    let package_and_version = package_and_version_result?;

    match package_and_version {
      PackageAndVersion::NotFound(package_name) => {
        bail!("{} was not found.", crate::colors::red(package_name));
      }
      PackageAndVersion::Selected(selected) => {
        selected_packages.push(selected);
      }
    }
  }

  let config_file_contents = {
    let contents = tokio::fs::read_to_string(&config_file_path).await.unwrap();
    if contents.trim().is_empty() {
      "{}\n".into()
    } else {
      contents
    }
  };
  let ast = jsonc_parser::parse_to_ast(
    &config_file_contents,
    &Default::default(),
    &Default::default(),
  )?;

  let obj = match ast.value {
    Some(Value::Object(obj)) => obj,
    _ => bail!("Failed updating config file due to no object."),
  };

  let mut existing_imports = config_file.existing_imports()?;

  let is_npm = config_file.is_npm();
  for selected_package in selected_packages {
    log::info!(
      "Add {} - {}@{}",
      crate::colors::green(&selected_package.import_name),
      selected_package.package_name,
      selected_package.version_req
    );

    if is_npm {
      let (name, version) = package_json_dependency_entry(selected_package);
      existing_imports.insert(name, version)
    } else {
      existing_imports.insert(
        selected_package.import_name,
        format!(
          "{}@{}",
          selected_package.package_name, selected_package.version_req
        ),
      )
    };
  }
  let mut import_list: Vec<(String, String)> =
    existing_imports.into_iter().collect();

  import_list.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
  let generated_imports = generate_imports(import_list);

  let fmt_config_options = config_file.fmt_options();

  let new_text = update_config_file_content(
    obj,
    &config_file_contents,
    generated_imports,
    fmt_config_options,
    config_file.imports_key(),
    config_file.file_name(),
  );

  tokio::fs::write(&config_file_path, new_text)
    .await
    .context("Failed to update configuration file")?;

  // clear the previously cached package.json from memory before reloading it
  deno_node::PackageJsonThreadLocalCache::clear();
  // make a new CliFactory to pick up the updated config file
  let cli_factory = CliFactory::from_flags(flags)?;
  // cache deps
  if cli_factory.cli_options().enable_future_features() {
    crate::module_loader::load_top_level_deps(&cli_factory).await?;
  }

  Ok(())
}

struct SelectedPackage {
  import_name: String,
  package_name: String,
  version_req: String,
}

enum PackageAndVersion {
  NotFound(String),
  Selected(SelectedPackage),
}

async fn find_package_and_select_version_for_req(
  jsr_resolver: Arc<JsrFetchResolver>,
  npm_resolver: Arc<NpmFetchResolver>,
  add_package_req: AddPackageReq,
) -> Result<PackageAndVersion, AnyError> {
  match add_package_req.value {
    AddPackageReqValue::Jsr(req) => {
      let jsr_prefixed_name = format!("jsr:{}", &req.name);
      let Some(nv) = jsr_resolver.req_to_nv(&req).await else {
        return Ok(PackageAndVersion::NotFound(jsr_prefixed_name));
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: add_package_req.alias,
        package_name: jsr_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
      }))
    }
    AddPackageReqValue::Npm(req) => {
      let npm_prefixed_name = format!("npm:{}", &req.name);
      let Some(nv) = npm_resolver.req_to_nv(&req).await else {
        return Ok(PackageAndVersion::NotFound(npm_prefixed_name));
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: add_package_req.alias,
        package_name: npm_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
      }))
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum AddPackageReqValue {
  Jsr(PackageReq),
  Npm(PackageReq),
}

#[derive(Debug, PartialEq, Eq)]
struct AddPackageReq {
  alias: String,
  value: AddPackageReqValue,
}

impl AddPackageReq {
  pub fn parse(entry_text: &str) -> Result<Self, AnyError> {
    enum Prefix {
      Jsr,
      Npm,
    }

    fn parse_prefix(text: &str) -> (Option<Prefix>, &str) {
      if let Some(text) = text.strip_prefix("jsr:") {
        (Some(Prefix::Jsr), text)
      } else if let Some(text) = text.strip_prefix("npm:") {
        (Some(Prefix::Npm), text)
      } else {
        (None, text)
      }
    }

    // parse the following:
    // - alias@npm:<package_name>
    // - other_alias@npm:<package_name>
    // - @alias/other@jsr:<package_name>
    fn parse_alias(entry_text: &str) -> Option<(&str, &str)> {
      for prefix in ["npm:", "jsr:"] {
        let Some(location) = entry_text.find(prefix) else {
          continue;
        };
        let prefix = &entry_text[..location];
        if let Some(alias) = prefix.strip_suffix('@') {
          return Some((alias, &entry_text[location..]));
        }
      }
      None
    }

    let (maybe_prefix, entry_text) = parse_prefix(entry_text);
    let (prefix, maybe_alias, entry_text) = match maybe_prefix {
      Some(prefix) => (prefix, None, entry_text),
      None => match parse_alias(entry_text) {
        Some((alias, text)) => {
          let (maybe_prefix, entry_text) = parse_prefix(text);
          (
            maybe_prefix.unwrap_or(Prefix::Jsr),
            Some(alias.to_string()),
            entry_text,
          )
        }
        None => (Prefix::Jsr, None, entry_text),
      },
    };

    match prefix {
      Prefix::Jsr => {
        let package_req = PackageReq::from_str(entry_text)?;
        Ok(AddPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.to_string()),
          value: AddPackageReqValue::Jsr(package_req),
        })
      }
      Prefix::Npm => {
        let package_req = PackageReq::from_str(entry_text)?;
        Ok(AddPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.to_string()),
          value: AddPackageReqValue::Npm(package_req),
        })
      }
    }
  }
}

fn generate_imports(packages_to_version: Vec<(String, String)>) -> String {
  let mut contents = vec![];
  let len = packages_to_version.len();
  for (index, (package, version)) in packages_to_version.iter().enumerate() {
    // TODO(bartlomieju): fix it, once we start support specifying version on the cli
    contents.push(format!("\"{}\": \"{}\"", package, version));
    if index != len - 1 {
      contents.push(",".to_string());
    }
  }
  contents.join("\n")
}

fn update_config_file_content(
  obj: jsonc_parser::ast::Object,
  config_file_contents: &str,
  generated_imports: String,
  fmt_options: FmtOptionsConfig,
  imports_key: &str,
  file_name: &str,
) -> String {
  let mut text_changes = vec![];

  match obj.get(imports_key) {
    Some(ObjectProp {
      value: Value::Object(lit),
      ..
    }) => text_changes.push(TextChange {
      range: (lit.range.start + 1)..(lit.range.end - 1),
      new_text: generated_imports,
    }),
    None => {
      let insert_position = obj.range.end - 1;
      text_changes.push(TextChange {
        range: insert_position..insert_position,
        // NOTE(bartlomieju): adding `\n` here to force the formatter to always
        // produce a config file that is multline, like so:
        // ```
        // {
        //   "imports": {
        //     "<package_name>": "<registry>:<package_name>@<semver>"
        //   }
        // }
        new_text: format!("\"{imports_key}\": {{\n {generated_imports} }}"),
      })
    }
    // we verified the shape of `imports`/`dependencies` above
    Some(_) => unreachable!(),
  }

  let new_text =
    deno_ast::apply_text_changes(config_file_contents, text_changes);

  crate::tools::fmt::format_json(
    &PathBuf::from(file_name),
    &new_text,
    &fmt_options,
  )
  .ok()
  .map(|formatted_text| formatted_text.unwrap_or_else(|| new_text.clone()))
  .unwrap_or(new_text)
}

#[cfg(test)]
mod test {
  use deno_semver::VersionReq;

  use super::*;

  #[test]
  fn test_parse_add_package_req() {
    assert_eq!(
      AddPackageReq::parse("jsr:foo").unwrap(),
      AddPackageReq {
        alias: "foo".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("alias@jsr:foo").unwrap(),
      AddPackageReq {
        alias: "alias".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("@alias/pkg@npm:foo").unwrap(),
      AddPackageReq {
        alias: "@alias/pkg".to_string(),
        value: AddPackageReqValue::Npm(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("@alias/pkg@jsr:foo").unwrap(),
      AddPackageReq {
        alias: "@alias/pkg".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("alias@jsr:foo@^1.5.0").unwrap(),
      AddPackageReq {
        alias: "alias".to_string(),
        value: AddPackageReqValue::Jsr(
          PackageReq::from_str("foo@^1.5.0").unwrap()
        )
      }
    );
    assert_eq!(
      AddPackageReq::parse("@scope/pkg@tag").unwrap(),
      AddPackageReq {
        alias: "@scope/pkg".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq {
          name: "@scope/pkg".to_string(),
          // this is a tag
          version_req: VersionReq::parse_from_specifier("tag").unwrap(),
        }),
      }
    );
  }
}
