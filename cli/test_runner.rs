// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fs as deno_fs;
use crate::installer::is_remote_url;
use std;
use std::path::Path;

pub fn run_tests(test_file_path: &Path, fail_fast: bool, quiet: bool) {

  // Import temporary test file and delete it immediately after importing so it's not cluttering disk.
  //
  // You may think that this will cause recompilation on each run, but this actually
  // tricks Deno to not recompile files if there's no need.
  // Eg.
  //   1. On first run of $DENO_DIR/.deno.test.ts Deno will compile and cache temporary test file and all of its imports
  //   2. Temporary test file is removed by test runner
  //   3. On next test run file is created again. If no new modules were added then temporary file contents are identical.
  //      Deno will not compile temporary test file again, but load it directly into V8.
  //   4. Deno starts loading imports one by one.
  //   5. If imported file is outdated, Deno will recompile this single file.
  //   let err;
  //   try {
  //     await import(`file://${testFilePath}`);
  //   } catch (e) {
  //     err = e;
  //   } finally {
  //     await Deno.remove(testFilePath);
  //   }

  //   if (err) {
  //     throw err;
  //   }

  //   if (!disableLog) {
  //     console.log(`Found ${moduleCount} matching test modules.`);
  //   }
}

fn find_test_modules(
  include: Vec<String>,
  exclude: Vec<String>,
) -> Vec<String> {
  let (include_paths, include_urls) = include.partition(|n| !is_remote_url(n));
  let (exclude_paths, exclude_urls) = exclude.partition(|n| !is_remote_url(n));

  let mut found = vec![];

  // const expandGlobOpts: ExpandGlobOptions = {
  //   root,
  //   exclude: excludePaths,
  //   includeDirs: true,
  //   extended: true,
  //   globstar: true
  // };

  // async function* expandDirectory(d: string): AsyncIterableIterator<string> {
  //   for (const dirGlob of DIR_GLOBS) {
  //     for await (const walkInfo of expandGlob(dirGlob, {
  //       ...expandGlobOpts,
  //       root: d,
  //       includeDirs: false
  //     })) {
  //       yield filePathToUrl(walkInfo.filename);
  //     }
  //   }
  // }

  for glob_string in include_paths {
    
  }

  for entry in WalkDir::new(".")
    .follow_links(true)
    .into_iter()
    .filter_map(|e| e.ok())
  {
    let f_name = entry.file_name().to_string_lossy();
    let sec = entry.metadata()?.modified()?;

    if f_name.ends_with(".json") && sec.elapsed()?.as_secs() < 86400 {
      println!("{}", f_name);
    }
  }

  // for (const globString of includePaths) {
  //   for await (const walkInfo of expandGlob(globString, expandGlobOpts)) {
  //     if (walkInfo.info.isDirectory()) {
  //       yield* expandDirectory(walkInfo.filename);
  //     } else {
  //       yield filePathToUrl(walkInfo.filename);
  //     }
  //   }
  // }

  // const excludeUrlPatterns = excludeUrls.map(
  //   (url: string): RegExp => RegExp(url)
  // );
  // const shouldIncludeUrl = (url: string): boolean =>
  //   !excludeUrlPatterns.some((p: RegExp): boolean => !!url.match(p));

  // yield* includeUrls.filter(shouldIncludeUrl);
}

fn render_test_file(modules: Vec<String>) -> String {
  let mut test_file = "";

  for module in modules {
    test_file += format!("import \"{}\";\n", module);
  }

  // TODO: add call to `runTests` here?

  test_file.to_string()
}

pub fn run_test_modules(
  include: Option<Vec<String>>,
  exclude: Option<Vec<String>>,
  fail_fast: bool,
  quiet: bool,
) {
  let mut module_count = 0;
  let mut test_modules = vec![];
  let allow_none = false;
  let include = include.unwrap_or_else(|| vec![]);
  let exclude = exclude.unwrap_or_else(|| vec![]);

  for test_module in find_test_modules(include, exclude) {
    test_modules.push(test_module);
    module_count += 1;
  }

  if module_count == 0 {
    println!("No matching test modules found");

    if !allow_none {
      std::process::exit(1);
    }

    return;
  }

  // Create temporary test file which contains
  // all matched modules as import statements.
  let test_file = render_test_file(test_modules);

  let cwd = std::env::current_dir().expect("No current directory");
  let test_file_path = cwd.join(".deno.test.ts");
  deno_fs::write_file(&test_file_path, test_file.as_bytes(), 0o666)
    .expect("Can't write test file");

  run_tests(test_file_path, fail_fast, quiet);
}
