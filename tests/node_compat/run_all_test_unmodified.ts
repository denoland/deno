// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { deadline } from "@std/async/deadline";
import { expandGlob } from "@std/fs/expand-glob";
import { toFileUrl } from "@std/path/to-file-url";
import { basename } from "@std/path/basename";
import { pooledMap } from "@std/async/pool";
import { partition } from "@std/collections/partition";

// The timeout ms for single test execution. If a single test didn't finish in this timeout milliseconds, the test is considered as failure */
const TIMEOUT = 2000;
const testDirUrl = new URL("runner/suite/test/", import.meta.url).href;

// from https://github.com/denoland/std/pull/2787#discussion_r1001237016
const NODE_IGNORED_TEST_DIRS = [
  "addons",
  "async-hooks",
  "benchmark",
  "cctest",
  "common",
  "doctool",
  "embedding",
  "fixtures",
  "fuzzers",
  "js-native-api",
  "node-api",
  "overlapped-checker",
  "report",
  "testpy",
  "tick-processor",
  "tools",
  "v8-updates",
  "wasi",
  "wpt",
];

// Category names are usually one word, but there are some exceptions.
// This list contains the exceptions.
const multiWordsCategoryNames = [
  "async-hooks",
  "async-local-storage",
  "async-wrap",
  "child-process",
  "compile-cache",
  "cpu-prof",
  "double-tls",
  "diagnostics-channel",
  "force-repl",
  "listen-fd",
  "memory-usage",
  "next-tick",
  "outgoing-message",
  "queue-microtask",
  "shadow-realm",
  "single-executable",
  "source-map",
  "string-decoder",
  "tcp-wrap",
  "tick-processor",
  "unhandled-exception",
  "wrap-js-stream",
];

const categoryMap = {
  error: "errors",
  diagnostic: "diagnostics-channel",
  no: "others",
  promises: "promise",
  set: "others",
  startup: "others",
  stream2: "stream",
  stream3: "stream",
  sync: "others",
  webstream: "webstreams",
  warn: "others",
} as Record<string, string>;

/**
 * The test files in these dirs seem categorized in the form
 * test-[category-name]-test-case.js
 */
const categorizedTestGroups = [
  "es-module",
  "parallel",
  "pummel",
  "sequential",
  "internet",
];

/** The group is the directory name of the test file.
 * e.g. parallel, internet, pummel, sequential, pseudo-tty, etc */
function getGroupRelUrl(str: string) {
  return str.split("/")[0];
}

/** Gets the category name from the test path
 * e.g.
 * - parallel/test-async-hooks-destroyed-context.js -> async-hooks
 * - sequential/test-child-process-exec-stderr.js -> child-process
 * - internet/test-http-keep-alive.js -> http
 * - pseudo-tty/test-stdin.js -> tty
 * - module-hooks/test-require.js -> module-hooks
 */
function getCategoryFromPath(str: string) {
  const group = getGroupRelUrl(str);
  if (group === "pseudo-tty") {
    return "tty";
  } else if (group === "module-hooks") {
    return "module-hooks";
  } else if (categorizedTestGroups.includes(group)) {
    const name = basename(str).replace(/\.js/, "");
    for (const multiWord of multiWordsCategoryNames) {
      if (name.startsWith("test-" + multiWord)) {
        return multiWord;
      }
    }
    const category = name.split("-")[1];
    return categoryMap[category] ?? category;
  } else {
    return "others";
  }
}

/** Collect the items that are not categorized into the "others" category. */
function collectNonCategorizedItems(categories: Record<string, string[]>) {
  const others = [] as string[];
  for (const [category, items] of Object.entries(categories)) {
    if (items.length === 1) {
      delete categories[category];
      others.push(...items);
    }
  }
  (categories["others"] ??= []).push(...others);
}

/** Run a test case */
function runTest(path: string, signal: AbortSignal) {
  return new Deno.Command(Deno.execPath(), {
    args: [
      "-A",
      "--unstable-bare-node-builtins",
      "--unstable-node-globals",
      "tests/node_compat/runner/suite/test/" + path,
    ],
    env: {
      NODE_TEST_KNOWN_GLOBALS: "0",
    },
    stdout: "piped",
    stderr: "piped",
    signal,
  });
}

async function main() {
  const tests = [] as string[];
  const categories = {} as Record<string, string[]>;
  for await (
    const test of expandGlob("tests/node_compat/runner/suite/**/test-*.js")
  ) {
    if (!test.isFile) continue;
    const relUrl = toFileUrl(test.path).href.replace(testDirUrl, "");
    if (NODE_IGNORED_TEST_DIRS.every((dir) => !relUrl.startsWith(dir))) {
      tests.push(relUrl);
      (categories[getCategoryFromPath(relUrl)] ??= []).push(relUrl);
    }
  }
  collectNonCategorizedItems(categories);
  console.log(tests.join("\n"));
  console.log("Running", tests.length, "tests");
  const categoryList = Object.entries(categories).sort(([c0], [c1]) =>
    c0.localeCompare(c1)
  );
  console.log(`Categories(${categoryList.length}):`);
  for (const [category, tests] of categoryList) {
    console.log(`  ${category}:`, tests.length);
  }
  const success = {} as Record<string, boolean>;
  let i = 0;
  async function run(testPath: string) {
    i++;
    const num = String(i).padStart(4, " ");
    try {
      const cp = await runTest(testPath, AbortSignal.timeout(TIMEOUT));
      const result = await deadline(cp.output(), TIMEOUT + 1000);
      if (result.code === 0) {
        console.log(`${num} %cPASS`, "color: green", testPath);
        success[testPath] = true;
      } else {
        console.log(`${num} %cFAIL`, "color: red", testPath);
      }
    } catch (e) {
      if (e instanceof DOMException && e.name === "TimeoutError") {
        console.log(`${num} %cFAIL`, "color: red", testPath);
      } else {
        console.log(`Unexpected Error`, e);
      }
    }
  }
  const [sequential, parallel] = partition(tests, (test) =>
    getGroupRelUrl(test) === "sequential"
  );
  // Runs sequential tests
  for (const path of sequential) {
    await run(path);
  }
  // Runs parallel tests
  for await (const _ of pooledMap(navigator.hardwareConcurrency * 2, parallel, run)) {
    // pass
  }
  const all = tests.length;
  const s = tests.filter((test) => success[test]).length;
  console.log(`All tests ${s}/${all} (${(s/all*100).toFixed(2)}%):`);
  console.log(`Result by categories (${categoryList.length}):`);
  for (const [category, tests] of categoryList) {
    const s = tests.filter((test) => success[test]).length;
    const all = tests.length;
    console.log(`  ${category} ${s}/${all} (${(s/all*100).toFixed(2)}%)`);
    for (const testPath of tests) {
      if (success[testPath]) {
        console.log(`    %cPASS`, "color: green", testPath);
      } else {
        console.log(`    %cFAIL`, "color: red", testPath);
      }
    }
  }
  Deno.exit(0);
}

await main();
