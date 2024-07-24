#!/usr/bin/env -S deno run --allow-write --allow-read --allow-run --allow-net --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { buildMode, getPrebuilt, getSources, join, ROOT_PATH } from "./util.js";
import { checkCopyright } from "./copyright_checker.js";
import * as ciFile from "../.github/workflows/ci.generate.ts";

const promises = [];

let js = Deno.args.includes("--js");
let rs = Deno.args.includes("--rs");
if (!js && !rs) {
  js = true;
  rs = true;
}

if (rs) {
  promises.push(clippy());
}

if (js) {
  promises.push(dlint());
  promises.push(dlintPreferPrimordials());
  promises.push(ensureCiYmlUpToDate());
  promises.push(ensureNoNewITests());

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
    ":!:tests/testdata/swc_syntax_error.ts",
    ":!:tests/testdata/error_008_checkjs.js",
    ":!:cli/bench/testdata/npm/*",
    ":!:cli/bench/testdata/express-router.js",
    ":!:cli/bench/testdata/react-dom.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/tsc/dts/**",
    ":!:target/",
    ":!:tests/registry/**",
    ":!:tests/specs/**",
    ":!:tests/testdata/encoding/**",
    ":!:tests/testdata/error_syntax.js",
    ":!:tests/testdata/file_extensions/ts_with_js_extension.js",
    ":!:tests/testdata/fmt/**",
    ":!:tests/testdata/lint/**",
    ":!:tests/testdata/npm/**",
    ":!:tests/testdata/run/**",
    ":!:tests/testdata/tsc/**",
    ":!:tests/testdata/test/glob/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:tests/wpt/suite/**",
    ":!:tests/wpt/runner/**",
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
  await Promise.all(pending);
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
  const cmd = ["clippy", "--all-targets", "--all-features", "--locked"];

  if (currentBuildMode != "debug") {
    cmd.push("--release");
  }

  const cargoCmd = new Deno.Command("cargo", {
    cwd: ROOT_PATH,
    args: [
      ...cmd,
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
    ],
    stdout: "inherit",
    stderr: "inherit",
  });
  const { code } = await cargoCmd.output();

  if (code > 0) {
    throw new Error("clippy failed");
  }
}

async function ensureCiYmlUpToDate() {
  const expectedCiFileText = ciFile.generate();
  const actualCiFileText = await Deno.readTextFile(ciFile.CI_YML_URL);
  if (expectedCiFileText !== actualCiFileText) {
    throw new Error(
      "./.github/workflows/ci.yml is out of date. Run: ./.github/workflows/ci.generate.ts",
    );
  }
}

async function ensureNoNewITests() {
  // Note: Only decrease these numbers. Never increase them!!
  // This is to help ensure we slowly deprecate these tests and
  // replace them with spec tests.
  const iTestCounts = {
    "bench_tests.rs": 0,
    "bundle_tests.rs": 11,
    "cache_tests.rs": 0,
    "cert_tests.rs": 0,
    "check_tests.rs": 23,
    "compile_tests.rs": 0,
    "coverage_tests.rs": 0,
    "doc_tests.rs": 15,
    "eval_tests.rs": 9,
    "flags_tests.rs": 0,
    "fmt_tests.rs": 17,
    "info_tests.rs": 18,
    "init_tests.rs": 0,
    "inspector_tests.rs": 0,
    "install_tests.rs": 0,
    "jsr_tests.rs": 0,
    "js_unit_tests.rs": 0,
    "jupyter_tests.rs": 0,
    "lint_tests.rs": 18,
    // Read the comment above. Please don't increase these numbers!
    "lsp_tests.rs": 0,
    "node_compat_tests.rs": 4,
    "node_unit_tests.rs": 2,
    "npm_tests.rs": 93,
    "pm_tests.rs": 0,
    "publish_tests.rs": 0,
    "repl_tests.rs": 0,
    "run_tests.rs": 360,
    "shared_library_tests.rs": 0,
    "task_tests.rs": 30,
    "test_tests.rs": 77,
    "upgrade_tests.rs": 0,
    "vendor_tests.rs": 1,
    "watcher_tests.rs": 0,
    "worker_tests.rs": 18,
  };
  const integrationDir = join(ROOT_PATH, "tests", "integration");
  for await (const entry of Deno.readDir(integrationDir)) {
    if (!entry.name.endsWith("_tests.rs")) {
      continue;
    }
    const fileText = await Deno.readTextFile(join(integrationDir, entry.name));
    const actualCount = fileText.match(/itest\!/g)?.length ?? 0;
    const expectedCount = iTestCounts[entry.name] ?? 0;
    // console.log(`"${entry.name}": ${actualCount},`);
    if (actualCount > expectedCount) {
      throw new Error(
        `New itest added to ${entry.name}! The itest macro is deprecated. Please move your new test to ~/tests/specs.`,
      );
    } else if (actualCount < expectedCount) {
      throw new Error(
        `Thanks for removing an itest in ${entry.name}. ` +
          `Please update the count in tools/lint.js for this file to ${actualCount}.`,
      );
    }
  }
}
