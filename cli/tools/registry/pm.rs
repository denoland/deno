// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;

use deno_ast::TextChange;
use deno_config::FmtOptionsConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_runtime::deno_fetch::reqwest;
use deno_semver::jsr::JsrPackageReqReference;
use jsonc_parser::ast::ObjectProp;
use jsonc_parser::ast::Value;

use super::api;
use crate::args::jsr_api_url;
use crate::args::AddFlags;
use crate::args::Flags;
use crate::factory::CliFactory;

async fn find_packages_latest_version(
  client: &reqwest::Client,
  registry_api_url: &str,
  package_name: String,
) -> Result<(String, Option<String>), AnyError> {
  let Some(name_no_at) = package_name.strip_prefix('@') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };
  let Some((scope, name_no_scope)) = name_no_at.split_once('/') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };

  let response =
    api::get_package(client, registry_api_url, scope, name_no_scope).await?;
  if response.status() == 404 {
    return Ok((package_name, None));
  }
  let package = api::parse_response::<api::Package>(response).await?;
  Ok((package_name, package.latest_version))
}

pub async fn add(flags: Flags, add_flags: AddFlags) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags.clone()).await?;
  let cli_options = cli_factory.cli_options();

  let Some(config_file) = cli_options.maybe_config_file() else {
    tokio::fs::write(cli_options.initial_cwd().join("deno.json"), "{}")
      .await
      .context("Failed to create deno.json file")?;
    eprintln!("Created deno.json configuration file.");
    return add(flags, add_flags).boxed_local().await;
  };

  if config_file.specifier.scheme() != "file" {
    bail!("Can't add dependencies to a remote configuration file");
  }
  let config_file_path = config_file.specifier.to_file_path().unwrap();

  let http_client = cli_factory.http_client();
  let client = http_client.client()?;
  let registry_api_url = jsr_api_url().to_string();

  let mut packages_to_version = Vec::with_capacity(add_flags.packages.len());

  // TODO(bartlomieju): parse as PackageReq - if there's `npm:` prefix, try on npm,
  // otherwise query JSR. Need to do semver as well - @luca/flag@^0.14 should use to
  // highest possible `0.14.x` version.
  // for package_name in add_flags.packages.iter() {
  //   if package_name.starts_with("npm:") {
  //     eprintln!("TODO: trying to add npm specifier, not handled yet");
  //   } else {
  //     let ref_ =
  //       JsrPackageReqReference::from_str(&format!("jsr:{}", package_name));
  //     eprintln!("ref_ {:?}", ref_);
  //   }
  // }

  let package_futures = add_flags
    .packages
    .into_iter()
    .map(|package_name| {
      find_packages_latest_version(client, &registry_api_url, package_name)
        .boxed_local()
    })
    .collect::<Vec<_>>();

  let stream_of_futures = deno_core::futures::stream::iter(package_futures);
  let mut buffered = stream_of_futures.buffer_unordered(10);

  while let Some(latest_version_result) = buffered.next().await {
    let (package_name, maybe_last_version) = latest_version_result?;
    if let Some(last_version) = maybe_last_version {
      packages_to_version.push((package_name, last_version));
    } else {
      bail!("{} was not found.", crate::colors::red(package_name));
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

  for (package, version) in packages_to_version {
    eprintln!(
      "Add {} - {}",
      crate::colors::green(format!("{}", package)),
      version
    );
    existing_imports.insert(package, version);
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

  tokio::fs::write(&config_file_path, new_text).await.unwrap();

  Ok(())
}

fn generate_imports(packages_to_version: Vec<(String, String)>) -> String {
  let mut contents = vec![];
  let len = packages_to_version.len();
  for (index, (package, version)) in packages_to_version.iter().enumerate() {
    // TODO(bartlomieju): fix it, once we start support specifying version on the cli
    contents.push(format!("\"{}\": \"jsr:{}@^{}\"", package, package, version));
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
