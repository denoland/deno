// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::TextChange;
use deno_config::FmtOptionsConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use jsonc_parser::ast::ObjectProp;
use jsonc_parser::ast::Value;

use crate::args::AddFlags;
use crate::args::CacheSetting;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

pub async fn add(flags: Flags, add_flags: AddFlags) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags.clone()).await?;
  let cli_options = cli_factory.cli_options();

  let Some(config_file) = cli_options.maybe_config_file() else {
    tokio::fs::write(cli_options.initial_cwd().join("deno.json"), "{}\n")
      .await
      .context("Failed to create deno.json file")?;
    log::info!("Created deno.json configuration file.");
    return add(flags, add_flags).boxed_local().await;
  };

  if config_file.specifier.scheme() != "file" {
    bail!("Can't add dependencies to a remote configuration file");
  }
  let config_file_path = config_file.specifier.to_file_path().unwrap();

  let http_client = cli_factory.http_client();

  let mut selected_packages = Vec::with_capacity(add_flags.packages.len());
  let mut package_reqs = Vec::with_capacity(add_flags.packages.len());

  for package_name in add_flags.packages.iter() {
    let req = if package_name.starts_with("npm:") {
      let pkg_req = NpmPackageReqReference::from_str(package_name)
        .with_context(|| {
          format!("Failed to parse package required: {}", package_name)
        })?;
      AddPackageReq::Npm(pkg_req)
    } else {
      let pkg_req = JsrPackageReqReference::from_str(&format!(
        "jsr:{}",
        package_name.strip_prefix("jsr:").unwrap_or(package_name)
      ))
      .with_context(|| {
        format!("Failed to parse package required: {}", package_name)
      })?;
      AddPackageReq::Jsr(pkg_req)
    };

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

  let config_file_contents =
    tokio::fs::read_to_string(&config_file_path).await.unwrap();
  let ast = jsonc_parser::parse_to_ast(
    &config_file_contents,
    &Default::default(),
    &Default::default(),
  )?;

  let obj = match ast.value {
    Some(Value::Object(obj)) => obj,
    _ => bail!("Failed updating config file due to no object."),
  };

  let mut existing_imports =
    if let Some(imports) = config_file.json.imports.clone() {
      match serde_json::from_value::<HashMap<String, String>>(imports) {
        Ok(i) => i,
        Err(_) => bail!("Malformed \"imports\" configuration"),
      }
    } else {
      HashMap::default()
    };

  for selected_package in selected_packages {
    log::info!(
      "Add {} - {}@{}",
      crate::colors::green(&selected_package.import_name),
      selected_package.package_name,
      selected_package.version_req
    );
    existing_imports.insert(
      selected_package.import_name,
      format!(
        "{}@{}",
        selected_package.package_name, selected_package.version_req
      ),
    );
  }
  let mut import_list: Vec<(String, String)> =
    existing_imports.into_iter().collect();

  import_list.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
  let generated_imports = generate_imports(import_list);

  let fmt_config_options = config_file
    .to_fmt_config()
    .ok()
    .flatten()
    .map(|config| config.options)
    .unwrap_or_default();

  let new_text = update_config_file_content(
    obj,
    &config_file_contents,
    generated_imports,
    fmt_config_options,
  );

  tokio::fs::write(&config_file_path, new_text)
    .await
    .context("Failed to update configuration file")?;

  // TODO(bartlomieju): we should now cache the imports from the config file.

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
  match add_package_req {
    AddPackageReq::Jsr(pkg_ref) => {
      let req = pkg_ref.req();
      let jsr_prefixed_name = format!("jsr:{}", &req.name);
      let Some(nv) = jsr_resolver.req_to_nv(req).await else {
        return Ok(PackageAndVersion::NotFound(jsr_prefixed_name));
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: req.name.to_string(),
        package_name: jsr_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
      }))
    }
    AddPackageReq::Npm(pkg_ref) => {
      let req = pkg_ref.req();
      let npm_prefixed_name = format!("npm:{}", &req.name);
      let Some(nv) = npm_resolver.req_to_nv(req).await else {
        return Ok(PackageAndVersion::NotFound(npm_prefixed_name));
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: req.name.to_string(),
        package_name: npm_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
      }))
    }
  }
}

enum AddPackageReq {
  Jsr(JsrPackageReqReference),
  Npm(NpmPackageReqReference),
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
) -> String {
  let mut text_changes = vec![];

  match obj.get("imports") {
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
        new_text: format!("\"imports\": {{ {} }}", generated_imports),
      })
    }
    // we verified the shape of `imports` above
    Some(_) => unreachable!(),
  }

  let new_text =
    deno_ast::apply_text_changes(config_file_contents, text_changes);

  crate::tools::fmt::format_json(
    &PathBuf::from("deno.json"),
    &new_text,
    &fmt_options,
  )
  .ok()
  .map(|formatted_text| formatted_text.unwrap_or_else(|| new_text.clone()))
  .unwrap_or(new_text)
}
