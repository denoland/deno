// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CompileFlags;
use crate::args::Flags;
use crate::graph_util::error_for_any_npm_specifier;
use crate::standalone::DenoCompileBinaryBuilder;
use crate::util::path::path_has_trailing_slash;
use crate::ProcState;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_runtime::colors;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::installer::infer_name_from_url;

pub async fn compile(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::from_flags(flags).await?;
  let binary_builder = DenoCompileBinaryBuilder::new(
    ps.file_fetcher.clone(),
    ps.http_client.clone(),
    ps.dir.clone(),
  );
  let module_specifier = ps.options.resolve_main_module()?;
  let module_roots = {
    let mut vec = Vec::with_capacity(compile_flags.include.len() + 1);
    vec.push(module_specifier.clone());
    for side_module in &compile_flags.include {
      vec.push(resolve_url_or_path(side_module, ps.options.initial_cwd())?);
    }
    vec
  };

  let output_path = resolve_compile_executable_output_path(
    &compile_flags,
    ps.options.initial_cwd(),
  )
  .await?;

  let graph = Arc::try_unwrap(
    ps.module_graph_builder
      .create_graph_and_maybe_check(module_roots)
      .await?,
  )
  .unwrap();

  // at the moment, we don't support npm specifiers in deno_compile, so show an error
  error_for_any_npm_specifier(&graph)?;

  let parser = ps.parsed_source_cache.as_capturing_parser();
  let eszip = eszip::EszipV2::from_graph(graph, &parser, Default::default())?;

  log::info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );
  let final_bin = binary_builder
    .build_bin(eszip, &module_specifier, &compile_flags, &ps.options)
    .await?;

  log::info!("{} {}", colors::green("Emit"), output_path.display());
  binary_builder.write(output_path, final_bin)?;

  Ok(())
}

async fn resolve_compile_executable_output_path(
  compile_flags: &CompileFlags,
  current_dir: &Path,
) -> Result<PathBuf, AnyError> {
  let module_specifier =
    resolve_url_or_path(&compile_flags.source_file, current_dir)?;

  let mut output = compile_flags.output.clone();

  if let Some(out) = output.as_ref() {
    if path_has_trailing_slash(out) {
      if let Some(infer_file_name) = infer_name_from_url(&module_specifier)
        .await
        .map(PathBuf::from)
      {
        output = Some(out.join(infer_file_name));
      }
    } else {
      output = Some(out.to_path_buf());
    }
  }

  if output.is_none() {
    output = infer_name_from_url(&module_specifier)
      .await
      .map(PathBuf::from)
  }

  output.ok_or_else(|| generic_error(
    "An executable name was not provided. One could not be inferred from the URL. Aborting.",
  )).map(|output| {
    get_os_specific_filepath(output, &compile_flags.target)
  })
}

fn get_os_specific_filepath(
  output: PathBuf,
  target: &Option<String>,
) -> PathBuf {
  let is_windows = match target {
    Some(target) => target.contains("windows"),
    None => cfg!(windows),
  };
  if is_windows && output.extension().unwrap_or_default() != "exe" {
    if let Some(ext) = output.extension() {
      // keep version in my-exe-0.1.0 -> my-exe-0.1.0.exe
      output.with_extension(format!("{}.exe", ext.to_string_lossy()))
    } else {
      output.with_extension("exe")
    }
  } else {
    output
  }
}

#[cfg(test)]
mod test {
  pub use super::*;

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_linux() {
    let path = resolve_compile_executable_output_path(
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(PathBuf::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
        include: vec![],
      },
      &std::env::current_dir().unwrap(),
    )
    .await
    .unwrap();

    // no extension, no matter what the operating system is
    // because the target was specified as linux
    // https://github.com/denoland/deno/issues/9667
    assert_eq!(path.file_name().unwrap(), "file");
  }

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_windows() {
    let path = resolve_compile_executable_output_path(
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(PathBuf::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-pc-windows-msvc".to_string()),
        include: vec![],
      },
      &std::env::current_dir().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "file.exe");
  }

  #[test]
  fn test_os_specific_file_path() {
    fn run_test(path: &str, target: Option<&str>, expected: &str) {
      assert_eq!(
        get_os_specific_filepath(
          PathBuf::from(path),
          &target.map(|s| s.to_string())
        ),
        PathBuf::from(expected)
      );
    }

    if cfg!(windows) {
      run_test("C:\\my-exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe.exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe-0.1.2", None, "C:\\my-exe-0.1.2.exe");
    } else {
      run_test("my-exe", Some("linux"), "my-exe");
      run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
    }

    run_test("C:\\my-exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.0.1.2", Some("windows"), "C:\\my-exe.0.1.2.exe");
    run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
  }
}
