#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import {
  buildMode,
  dirname,
  getPrebuilt,
  getSources,
  gitLsFiles,
  join,
  parseJSONC,
  ROOT_PATH,
  SEPARATOR,
  walk,
} from "./util.js";
import { assertEquals } from "@std/assert";
import { checkCopyright } from "./copyright_checker.js";

const promises = [];

let js = Deno.args.includes("--js");
let rs = Deno.args.includes("--rs");
if (!js && !rs) {
  js = true;
  rs = true;
}

if (rs) {
  promises.push(clippy());
  promises.push(ensureNoNonPermissionCapitalLetterShortFlags());
  promises.push(ensureDisallowedMethodsEnforced());
}

if (js) {
  promises.push(dlint());
  promises.push(dlintPreferPrimordials());
  promises.push(lintNodePolyfillDenoApis());
  promises.push(ensureWorkflowYmlsUpToDate());
  promises.push(ensureNoUnusedOutFiles());
  promises.push(ensureNoNewTopLevelEntries());

  if (rs) {
    promises.push(checkCopyright());
  }
}

const results = await Promise.allSettled(promises);
for (const result of results) {
  if (result.status === "rejected") {
    console.error(result.reason);
    Deno.exit(1);
  }
}

async function dlint() {
  const configFile = join(ROOT_PATH, ".dlint.json");
  const execPath = await getPrebuilt("dlint");

  const sourceFiles = await getSources(ROOT_PATH, [
    "*.js",
    "*.ts",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/tools/coverage/script.js",
    ":!:runtime/cpu_profiler/flamegraph.js",
    ":!:cli/tools/doc/prism.css",
    ":!:cli/tools/doc/prism.js",
    ":!:cli/tsc/dts/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:ext/node/polyfills/deps/**",
    ":!:runtime/examples/",
    ":!:libs/eszip/testdata/**",
    ":!:target/",
    ":!:tests/bench/testdata/npm/*",
    ":!:tests/bench/testdata/express-router.js",
    ":!:tests/bench/testdata/react-dom.js",
    ":!:tests/ffi/testdata/test.js",
    ":!:tests/registry/**",
    ":!:tests/specs/**",
    ":!:tests/testdata/**",
    ":!:tests/unit_node/testdata/**",
    ":!:tests/wpt/runner/**",
    ":!:tests/wpt/suite/**",
    ":!:libs/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  const pending = [];
  for (const chunk of chunks) {
    const cmd = new Deno.Command(execPath, {
      cwd: ROOT_PATH,
      args: ["run", "--config=" + configFile, ...chunk],
      // capture to not conflict with clippy output
      stderr: "piped",
    });
    pending.push(
      cmd.output().then(({ stderr, code }) => {
        if (code > 0) {
          const decoder = new TextDecoder();
          console.log("\n------ dlint ------");
          console.log(decoder.decode(stderr));
          throw new Error("dlint failed");
        }
      }),
    );
  }
  const results = await Promise.allSettled(pending);
  for (const result of results) {
    if (result.status === "rejected") {
      throw new Error(result.reason);
    }
  }
}

// `prefer-primordials` has to apply only to files related to bootstrapping,
// which is different from other lint rules. This is why this dedicated function
// is needed.
async function dlintPreferPrimordials() {
  const execPath = await getPrebuilt("dlint");
  const sourceFiles = await getSources(ROOT_PATH, [
    "runtime/**/*.js",
    "runtime/**/*.ts",
    "ext/**/*.js",
    "ext/**/*.ts",
    ":!:ext/**/*.d.ts",
    "ext/node/polyfills/*.mjs",
    ":!:ext/node/polyfills/deps/**",
    ":!:runtime/cpu_profiler/flamegraph.js",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  for (const chunk of chunks) {
    const cmd = new Deno.Command(execPath, {
      cwd: ROOT_PATH,
      args: ["run", "--rule", "prefer-primordials", ...chunk],
      stdout: "inherit",
      stderr: "inherit",
    });
    const { code } = await cmd.output();

    if (code > 0) {
      throw new Error("prefer-primordials failed");
    }
  }
}

// Lint ext/node/polyfills for Deno.* API usage. These should be migrated
// to internal ops or ext: imports. The expected violation counts are tracked
// per file in no_deno_api_in_polyfills.ts -- any mismatch is a hard error.
async function lintNodePolyfillDenoApis() {
  const pluginPath = import.meta.resolve(
    "./lint_plugins/no_deno_api_in_polyfills.ts",
  );

  const { EXPECTED_VIOLATIONS } = await import(pluginPath);

  // Create a temp deno.json config that only enables our plugin.
  const configPath = await Deno.makeTempFile({ suffix: ".json" });
  try {
    await Deno.writeTextFile(
      configPath,
      JSON.stringify({
        lint: {
          plugins: [pluginPath],
        },
      }),
    );

    const sourceFiles = await getSources(ROOT_PATH, [
      "ext/node/polyfills/*.ts",
      "ext/node/polyfills/*.js",
      "ext/node/polyfills/*.mjs",
      "ext/node/polyfills/**/*.ts",
      "ext/node/polyfills/**/*.js",
      "ext/node/polyfills/**/*.mjs",
      ":!:ext/node/polyfills/deps/**",
    ]);

    if (!sourceFiles.length) {
      return;
    }

    const cmd = new Deno.Command(Deno.execPath(), {
      cwd: ROOT_PATH,
      args: [
        "lint",
        "--config=" + configPath,
        ...sourceFiles,
      ],
      stdout: "piped",
      stderr: "piped",
    });

    const { stdout, stderr } = await cmd.output();
    const output = new TextDecoder().decode(stdout) +
      new TextDecoder().decode(stderr);

    // Strip ANSI codes for reliable parsing.
    // deno-lint-ignore no-control-regex
    const clean = output.replace(/\x1b\[[0-9;]*m/g, "");

    // Count actual violations per file.
    const actualCounts = {};
    const lineRegex = /--> (.+):(\d+):(\d+)/g;
    const allMatches = [...clean.matchAll(lineRegex)];
    for (const match of allMatches) {
      const absPath = match[1];
      const relPath = absPath.replace(ROOT_PATH + "/", "").replace(
        ROOT_PATH + "\\",
        "",
      ).replaceAll("\\", "/");
      actualCounts[relPath] = (actualCounts[relPath] || 0) + 1;
    }

    // Debug: show what we parsed.
    console.log(
      `[no-deno-api] ROOT_PATH: ${ROOT_PATH}`,
    );
    console.log(
      `[no-deno-api] deno lint output length: stdout=${
        new TextDecoder().decode(stdout).length
      } stderr=${new TextDecoder().decode(stderr).length}`,
    );
    console.log(
      `[no-deno-api] regex matched ${allMatches.length} violation(s)`,
    );
    if (allMatches.length === 0 && clean.length > 0) {
      // Show first few lines with --> to debug format mismatch
      const arrowLines = clean.split("\n").filter((l) => l.includes("-->"));
      console.log(
        `[no-deno-api] lines containing "-->": ${arrowLines.length}`,
      );
      for (const line of arrowLines.slice(0, 5)) {
        console.log(`[no-deno-api]   ${JSON.stringify(line)}`);
      }
    } else if (allMatches.length > 0) {
      // Show a sample match
      const m = allMatches[0];
      console.log(
        `[no-deno-api] sample match: path=${JSON.stringify(m[1])} -> relPath=${
          JSON.stringify(
            m[1].replace(ROOT_PATH + "/", "").replace(ROOT_PATH + "\\", "")
              .replaceAll("\\", "/"),
          )
        }`,
      );
    }
    console.log(
      `[no-deno-api] actualCounts has ${
        Object.keys(actualCounts).length
      } file(s)`,
    );

    // Compare actual vs expected.
    const errors = [];
    const allFiles = new Set([
      ...Object.keys(EXPECTED_VIOLATIONS),
      ...Object.keys(actualCounts),
    ]);

    for (const file of [...allFiles].sort()) {
      const expected = EXPECTED_VIOLATIONS[file] || 0;
      const actual = actualCounts[file] || 0;

      if (actual > expected) {
        errors.push(
          `${file}: expected ${expected} Deno.* violations but found ${actual} (+${
            actual - expected
          }). ` +
            "New Deno.* API usage is not allowed in node polyfills.",
        );
      } else if (actual < expected) {
        errors.push(
          `${file}: expected ${expected} Deno.* violations but found ${actual} (-${
            expected - actual
          }). ` +
            "Please update EXPECTED_VIOLATIONS in tools/lint_plugins/no_deno_api_in_polyfills.ts.",
        );
      }
    }

    if (errors.length > 0) {
      console.log("\n------ node polyfills Deno API usage ------");
      for (const err of errors) {
        console.error(err);
      }
      throw new Error(
        `${errors.length} file(s) have mismatched Deno.* API violation counts`,
      );
    }
  } finally {
    await Deno.remove(configPath);
  }
}

function splitToChunks(paths, initCmdLen) {
  let cmdLen = initCmdLen;
  const MAX_COMMAND_LEN = 30000;
  const chunks = [[]];
  for (const p of paths) {
    if (cmdLen + p.length > MAX_COMMAND_LEN) {
      chunks.push([p]);
      cmdLen = initCmdLen;
    } else {
      chunks[chunks.length - 1].push(p);
      cmdLen += p.length;
    }
  }
  return chunks;
}

async function clippy() {
  const currentBuildMode = buildMode();

  const clippyDenyFlags = [
    "--",
    "-D",
    "warnings",
    "--deny",
    "clippy::unused_async",
    // generally prefer the `log` crate, but ignore
    // these print_* rules if necessary
    "--deny",
    "clippy::print_stderr",
    "--deny",
    "clippy::print_stdout",
    "--deny",
    "clippy::large_futures",
    "--deny",
    "clippy::allow_attributes_without_reason",
  ];

  // Run clippy for the whole workspace except deno_core with --all-features.
  // deno_core is excluded because --all-features enables
  // v8_enable_pointer_compression which is not available on all platforms.
  {
    const cmd = [
      "clippy",
      "--all-targets",
      "--all-features",
      "--locked",
      "--workspace",
      "--exclude",
      "deno_core",
    ];

    if (currentBuildMode != "debug") {
      cmd.push("--release");
    }

    const cargoCmd = new Deno.Command("cargo", {
      cwd: ROOT_PATH,
      args: [...cmd, ...clippyDenyFlags],
      stdout: "inherit",
      stderr: "inherit",
    });
    const { code } = await cargoCmd.output();

    if (code > 0) {
      throw new Error("clippy failed");
    }
  }

  // Run clippy for deno_core with specific features, matching the invocation
  // from https://github.com/denoland/deno_core/blob/main/tools/lint.ts
  {
    const DENO_CORE_CLIPPY_FEATURES = [
      "default",
      "include_js_files_for_snapshotting",
      "unsafe_runtime_options",
      "unsafe_use_unprotected_platform",
    ].join(",");

    const cmd = [
      "clippy",
      "-p",
      "deno_core",
      "--all-targets",
      "--locked",
      "--features",
      DENO_CORE_CLIPPY_FEATURES,
    ];

    if (currentBuildMode != "debug") {
      cmd.push("--release");
    }

    const cargoCmd = new Deno.Command("cargo", {
      cwd: ROOT_PATH,
      args: [...cmd, ...clippyDenyFlags],
      stdout: "inherit",
      stderr: "inherit",
    });
    const { code } = await cargoCmd.output();

    if (code > 0) {
      throw new Error("clippy failed for deno_core");
    }
  }
}

async function ensureWorkflowYmlsUpToDate() {
  const generators = [
    ".github/workflows/ci.ts",
    ".github/workflows/create_prerelease_tag.ts",
    ".github/workflows/pr.ts",
    ".github/workflows/cargo_publish.ts",
    ".github/workflows/ecosystem_compat_test.ts",
    ".github/workflows/node_compat_test.ts",
    ".github/workflows/npm_publish.ts",
    ".github/workflows/post_publish.ts",
    ".github/workflows/promote_to_release.ts",
    ".github/workflows/start_release.ts",
    ".github/workflows/version_bump.ts",
  ];

  const pending = generators.map(async (gen) => {
    const cmd = new Deno.Command("deno", {
      cwd: ROOT_PATH,
      args: ["run", "--allow-read=.", gen, "--lint"],
      stderr: "piped",
      stdout: "piped",
    });
    const { code, stderr } = await cmd.output();
    if (code !== 0) {
      const ymlFile = gen.replace(".ts", ".generated.yml");
      const decoder = new TextDecoder();
      throw new Error(
        `${ymlFile} is out of date. Run: ${gen}\n${decoder.decode(stderr)}`,
      );
    }
  });

  await Promise.all(pending);
}

/**
 * When short permission flags were being proposed, a concern that was raised was that
 * it would degrade the permission system by making the flags obscure. To address this
 * concern, we decided to make uppercase short flags ONLY relate to permissions. That
 * way if someone specifies something like `-E`, the user can scrutinize the command
 * a bit more than if it were `-e`. This custom lint rule attempts to try to maintain
 * this convention.
 */
async function ensureNoNonPermissionCapitalLetterShortFlags() {
  const text = await Deno.readTextFile(join(ROOT_PATH, "cli/args/flags.rs"));
  const shortFlags = text.matchAll(/\.short\('([A-Z])'\)/g);
  const values = Array.from(shortFlags.map((flag) => flag[1])).sort();
  // DO NOT update this list with a non-permission short flag without
  // discussion--there needs to be precedence to add to this list.
  const expected = [
    // --allow-all
    "A",
    // --dev flag for `deno install` (precedence: `npm install -D <package>`)
    "D",
    // --allow-env
    "E",
    // --allow-import
    "I",
    // log level (precedence: legacy)
    "L",
    // --allow-net
    "N",
    // --permission-set
    "P",
    // --allow-read
    "R",
    // --allow-sys
    "S",
    // version flag (precedence: legacy)
    "V",
    // --allow-write
    "W",
  ];
  assertEquals(values, expected);
}

async function ensureNoUnusedOutFiles() {
  const specsDir = join(ROOT_PATH, "tests", "specs");
  const outFilePaths = new Set(
    (await Array.fromAsync(
      walk(specsDir, { exts: [".out"] }),
    )).map((entry) => entry.path),
  );
  const testFiles = (await Array.fromAsync(
    walk(specsDir, { exts: [".jsonc"] }),
  )).filter((entry) => {
    return entry.path.endsWith("__test__.jsonc");
  });

  function checkObject(baseDirPath, obj, substsInit = {}) {
    const substs = { ...substsInit };

    if ("variants" in obj) {
      for (const variantValue of Object.values(obj.variants)) {
        for (const [substKey, substValue] of Object.entries(variantValue)) {
          const subst = `\$\{${substKey}\}`;
          if (subst in substs) {
            substs[subst].push(substValue);
          } else {
            substs[subst] = [substValue];
          }
        }
      }
    }
    for (const [key, value] of Object.entries(obj)) {
      if (typeof value === "object") {
        checkObject(baseDirPath, value, substs);
      } else if (key === "output" && typeof value === "string") {
        for (const [subst, substValues] of Object.entries(substs)) {
          if (value.includes(subst)) {
            for (const substValue of substValues) {
              const substitutedValue = value.replaceAll(subst, substValue);
              const substitutedOutFilePath = join(
                baseDirPath,
                substitutedValue,
              );
              outFilePaths.delete(substitutedOutFilePath);
            }
          }
        }
        const outFilePath = join(baseDirPath, value);
        outFilePaths.delete(outFilePath);
      }
    }
  }

  for (const testFile of testFiles) {
    try {
      const text = await Deno.readTextFile(testFile.path);
      const data = parseJSONC(text);
      checkObject(dirname(testFile.path), data);
    } catch (err) {
      throw new Error("Failed reading: " + testFile.path, {
        cause: err,
      });
    }
  }

  const notFoundPaths = Array.from(outFilePaths);
  if (notFoundPaths.length > 0) {
    notFoundPaths.sort(); // be deterministic
    for (const file of notFoundPaths) {
      console.error(`Unreferenced .out file: ${file}`);
    }
    throw new Error(`${notFoundPaths.length} unreferenced .out files`);
  }
}

async function listTopLevelEntries() {
  const files = await gitLsFiles(ROOT_PATH, []);
  const rootPrefix = ROOT_PATH.replace(new RegExp(SEPARATOR + "$"), "") +
    SEPARATOR;
  return [
    ...new Set(
      files.map((f) => f.replace(rootPrefix, ""))
        .map((file) => {
          const sepIndex = file.indexOf(SEPARATOR);
          // top-level file or first path component (directory)
          return sepIndex === -1 ? file : file.substring(0, sepIndex);
        }),
    ),
  ].sort();
}

// every ext/ and libs/ crate must have a clippy.toml with the correct
// disallowed methods
async function ensureDisallowedMethodsEnforced() {
  // methods that must be banned in both ext and libs crates
  const COMMON_METHODS = [
    "std::path::Path::canonicalize",
    "std::path::Path::is_dir",
    "std::path::Path::is_file",
    "std::path::Path::is_symlink",
    "std::path::Path::metadata",
    "std::path::Path::read_dir",
    "std::path::Path::read_link",
    "std::path::Path::symlink_metadata",
    "std::path::Path::try_exists",
    "std::path::Path::exists",
    "std::fs::canonicalize",
    "std::fs::copy",
    "std::fs::create_dir_all",
    "std::fs::create_dir",
    "std::fs::DirBuilder::new",
    "std::fs::hard_link",
    "std::fs::metadata",
    "std::fs::OpenOptions::new",
    "std::fs::read_dir",
    "std::fs::read_link",
    "std::fs::read_to_string",
    "std::fs::read",
    "std::fs::remove_dir_all",
    "std::fs::remove_dir",
    "std::fs::remove_file",
    "std::fs::rename",
    "std::fs::set_permissions",
    "std::fs::symlink_metadata",
    "std::fs::write",
    "url::Url::to_file_path",
    "url::Url::from_file_path",
    "url::Url::from_directory_path",
  ];

  // additional methods that must be banned in libs crates
  const LIBS_EXTRA_METHODS = [
    "std::path::absolute",
    "std::env::var",
    "std::env::var_os",
    "std::env::current_dir",
    "std::env::set_current_dir",
    "std::env::temp_dir",
    "std::time::SystemTime::now",
    "chrono::Utc::now",
  ];

  const errors = [];

  async function checkCrateDir(crateDir, kind) {
    const clippyToml = join(crateDir, "clippy.toml");
    let clippyContent;
    try {
      clippyContent = await Deno.readTextFile(clippyToml);
    } catch {
      errors.push(`Missing clippy.toml: ${clippyToml}`);
      return;
    }

    const requiredMethods = kind === "libs"
      ? [...COMMON_METHODS, ...LIBS_EXTRA_METHODS]
      : COMMON_METHODS;
    for (const method of requiredMethods) {
      if (!clippyContent.includes(`"${method}"`)) {
        errors.push(`Missing disallowed method "${method}" in: ${clippyToml}`);
      }
    }
  }

  // check ext crates
  for await (
    const entry of Deno.readDir(join(ROOT_PATH, "ext"))
  ) {
    if (!entry.isDirectory) continue;
    const crateDir = join(ROOT_PATH, "ext", entry.name);
    try {
      await Deno.stat(join(crateDir, "Cargo.toml"));
    } catch {
      continue;
    }
    await checkCrateDir(crateDir, "ext");
  }

  // check libs crates
  for await (
    const entry of Deno.readDir(join(ROOT_PATH, "libs"))
  ) {
    if (entry.name === "core_testing") {
      continue; // skip only test crates
    }
    if (!entry.isDirectory) continue;
    const crateDir = join(ROOT_PATH, "libs", entry.name);
    try {
      await Deno.stat(join(crateDir, "Cargo.toml"));
    } catch {
      continue;
    }
    await checkCrateDir(crateDir, "libs");
  }

  // check runtime crate (treated like ext - no env/time restrictions)
  await checkCrateDir(join(ROOT_PATH, "runtime"), "ext");
  // check runtime/permissions (treated like libs)
  await checkCrateDir(join(ROOT_PATH, "runtime", "permissions"), "libs");

  if (errors.length > 0) {
    errors.sort();
    for (const msg of errors) {
      console.error(msg);
    }
    throw new Error(
      `${errors.length} disallowed-methods enforcement error(s)`,
    );
  }
}

async function ensureNoNewTopLevelEntries() {
  const currentEntries = await listTopLevelEntries();

  // WARNING: When adding anything to this list it must be discussed!
  // Keep the root of the repository clean.
  const allowed = new Set([
    ".cargo",
    ".devcontainer",
    ".github",
    "x",
    "cli",
    "ext",
    "libs",
    "runtime",
    "tests",
    "tools",
    ".dlint.json",
    ".dprint.json",
    ".editorconfig",
    ".gitattributes",
    // WARNING! See Notice above before adding anything here
    ".gitignore",
    ".gitmodules",
    ".rustfmt.toml",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE.md",
    "README.md",
    "Releases.md",
    "import_map.json",
    "rust-toolchain.toml",
    "flake.nix",
    "flake.lock",
  ]);

  const newEntries = currentEntries.filter((e) => !allowed.has(e));
  if (newEntries.length > 0) {
    throw new Error(
      `New top-level entries detected: ${newEntries.join(", ")}. ` +
        `Only the following top-level entries are allowed: ${
          Array.from(allowed).join(", ")
        }`,
    );
  }
}
