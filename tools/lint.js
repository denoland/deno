#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2025 the Deno authors. MIT license.

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
  promises.push(ensureNoNewITests());
  promises.push(ensureNoNonPermissionCapitalLetterShortFlags());
}

if (js) {
  promises.push(dlint());
  promises.push(dlintPreferPrimordials());
  promises.push(ensureCiYmlUpToDate());
  promises.push(ensureNoUnusedOutFiles());
  promises.push(ensureNoNewTopLevelFiles());

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
    ":!:cli/bench/testdata/npm/*",
    ":!:cli/bench/testdata/express-router.js",
    ":!:cli/bench/testdata/react-dom.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/tools/coverage/script.js",
    ":!:cli/tools/doc/prism.css",
    ":!:cli/tools/doc/prism.js",
    ":!:cli/tsc/dts/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:ext/node/polyfills/deps/**",
    ":!:runtime/examples/",
    ":!:target/",
    ":!:tests/ffi/tests/test.js",
    ":!:tests/registry/**",
    ":!:tests/specs/**",
    ":!:tests/testdata/**",
    ":!:tests/unit_node/testdata/**",
    ":!:tests/wpt/runner/**",
    ":!:tests/wpt/suite/**",
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
      "--deny",
      "clippy::large_futures",
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
    "cache_tests.rs": 0,
    "cert_tests.rs": 0,
    "check_tests.rs": 0,
    "compile_tests.rs": 0,
    "coverage_tests.rs": 0,
    "eval_tests.rs": 0,
    "flags_tests.rs": 0,
    "fmt_tests.rs": 14,
    "init_tests.rs": 0,
    "inspector_tests.rs": 0,
    "install_tests.rs": 0,
    "jsr_tests.rs": 0,
    "js_unit_tests.rs": 0,
    "jupyter_tests.rs": 0,
    // Read the comment above. Please don't increase these numbers!
    "lsp_tests.rs": 0,
    "node_compat_tests.rs": 0,
    "node_unit_tests.rs": 2,
    "npm_tests.rs": 5,
    "pm_tests.rs": 0,
    "publish_tests.rs": 0,
    "repl_tests.rs": 0,
    "run_tests.rs": 17,
    "shared_library_tests.rs": 0,
    "task_tests.rs": 2,
    "test_tests.rs": 0,
    "upgrade_tests.rs": 0,
    "vendor_tests.rs": 1,
    "watcher_tests.rs": 0,
    "worker_tests.rs": 0,
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

async function listTopLevelFiles() {
  const files = await gitLsFiles(ROOT_PATH, []);
  return [
    ...new Set(
      files.map((f) =>
        f.replace(
          ROOT_PATH.replace(new RegExp(SEPARATOR + "$"), "") + SEPARATOR,
          "",
        )
      )
        .filter((file) => !file.includes(SEPARATOR)),
    ),
  ].sort();
}

async function ensureNoNewTopLevelFiles() {
  const currentFiles = await listTopLevelFiles();

  const allowedFiles = [
    ".dlint.json",
    ".dprint.json",
    ".editorconfig",
    ".gitattributes",
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
  ].sort();

  const newFiles = currentFiles.filter((file) => !allowedFiles.includes(file));
  if (newFiles.length > 0) {
    throw new Error(
      `New top-level files detected: ${newFiles.join(", ")}. ` +
        `Only the following top-level files are allowed: ${
          allowedFiles.join(", ")
        }`,
    );
  }
}
