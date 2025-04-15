// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { deadline } from "@std/async/deadline";
import { expandGlob } from "@std/fs/expand-glob";
import { toFileUrl } from "@std/path/to-file-url";
import { basename } from "@std/path/basename";
import { pooledMap } from "@std/async/pool";
import { partition } from "@std/collections/partition";
import { stripAnsiCode } from "@std/fmt/colors";
import { version as nodeVersion } from "./runner/suite/node_version.ts";
import { RUN_ARGS, TEST_ARGS, usesNodeTestModule } from "./common.ts";

// The timeout ms for single test execution. If a single test didn't finish in this timeout milliseconds, the test is considered as failure
const TIMEOUT = 2000;
const testDirUrl = new URL("runner/suite/test/", import.meta.url).href;

// The metadata of the test report
export type TestReportMetadata = {
  date: string;
  denoVersion: string;
  os: string;
  arch: string;
  nodeVersion: string;
  runId: string | null;
  total: number;
  pass: number;
};

// The test report format, which is stored in JSON file
type TestReport = TestReportMetadata & {
  results: Record<string, SingleResult>;
};

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
  "cpu-prof",
  "double-tls",
  "diagnostics-channel",
  "force-repl",
  "listen-fd",
  "memory-usage",
  "next-tick",
  "outgoing-message",
  "shadow-realm",
  "single-executable",
  "string-decoder",
];

const categoryMap = {
  cjs: "module",
  cwd: "process",
  diagnostic: "diagnostics-channel",
  "double-tls": "net",
  event: "events",
  eventsource: "events",
  eventtarget: "events",
  esm: "module",
  file: "fs",
  filehandle: "fs",
  "force-repl": "repl",
  inspect: "util",
  "listen-fd": "net",
  "next-tick": "process",
  "outgoing-message": "http",
  promises: "promise",
  readable: "stream",
  require: "module",
  socket: "net",
  stdin: "stdio",
  stdout: "stdio",
  stream2: "stream",
  stream3: "stream",
  tcp: "net",
  ttywrap: "tty",
  webstream: "webstreams",
} as Record<string, string>;

// These name could appear as category name, but they are actually not.
// If the category name is one of these, it should be categorized as "others".
const otherCategories = [
  "common",
  "compile",
  "corepack",
  "disable",
  "env",
  "error",
  "errors",
  "eslint",
  "eval",
  "exception",
  "handle",
  "heap",
  "heapdump",
  "heapsnapshot",
  "internal",
  "memory",
  "no",
  "queue",
  "release",
  "set",
  "source",
  "startup",
  "sync",
  "trace",
  "tick",
  "unhandled",
  "uv",
  "warn",
  "windows",
  "wrap",
];

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
 * - module-hooks/test-require.js -> module
 */
function getCategoryFromPath(str: string) {
  const group = getGroupRelUrl(str);
  if (group === "pseudo-tty") {
    return "tty";
  } else if (group === "module-hooks") {
    return "module";
  } else if (categorizedTestGroups.includes(group)) {
    const name = basename(str).replace(/\.js/, "");
    let category = name.split("-")[1];
    for (const multiWord of multiWordsCategoryNames) {
      if (name.startsWith("test-" + multiWord)) {
        category = multiWord;
      }
    }
    category = categoryMap[category] ?? category;
    if (otherCategories.includes(category)) {
      return "others";
    }
    return category;
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

function truncateTestOutput(output: string): string {
  output = stripAnsiCode(output);
  if (output.length > 2000) {
    return output.slice(0, 2000) + " ...";
  }
  return output;
}

export type SingleResult = [
  pass: boolean,
  error?: ErrorExit | ErrorTimeout | ErrorUnexpected,
];
type ErrorExit = {
  code: number;
  stderr: string;
};
type ErrorTimeout = {
  timeout: number;
};
type ErrorUnexpected = {
  message: string;
};

/**
 * Run a single node test file. Retries 3 times on WouldBlock error.
 *
 * @param testPath Relative path to the test file
 */
async function runSingle(testPath: string, retry = 0): Promise<SingleResult> {
  let cmd: Deno.ChildProcess | undefined;
  const testPath_ = "tests/node_compat/runner/suite/test/" + testPath;
  try {
    const usesNodeTest = await Deno.readTextFile(testPath_)
      .then(usesNodeTestModule).catch(() => false);
    cmd = new Deno.Command(Deno.execPath(), {
      args: [
        ...(usesNodeTest ? TEST_ARGS : RUN_ARGS),
        testPath_,
      ],
      env: {
        NODE_TEST_KNOWN_GLOBALS: "0",
        NODE_SKIP_FLAG_CHECK: "1",
        NO_COLOR: "1",
      },
      stdout: "piped",
      stderr: "piped",
    }).spawn();
    const result = await deadline(cmd.output(), TIMEOUT);
    if (result.code === 0) {
      return [true];
    } else {
      return [false, {
        code: result.code,
        stderr: truncateTestOutput(new TextDecoder().decode(result.stderr)),
      }];
    }
  } catch (e) {
    if (e instanceof DOMException && e.name === "TimeoutError") {
      try {
        cmd?.kill();
      } catch {
        // ignore
      }
      return [false, { timeout: TIMEOUT }];
    } else if (e instanceof Deno.errors.WouldBlock && retry < 3) {
      // retry 2 times on WouldBlock error (Resource temporarily unavailable)
      return runSingle(testPath, retry + 1);
    } else {
      return [false, { message: (e as Error).message }];
    }
  }
}

async function main() {
  const start = Date.now();
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
  console.log("Running", tests.length, "tests");
  const categoryList = Object.entries(categories)
    .sort(([c0], [c1]) => c0.localeCompare(c1));
  const results = {} as Record<string, SingleResult>;
  let i = 0;
  async function run(testPath: string) {
    const num = String(++i).padStart(4, " ");
    const result = await runSingle(testPath);
    results[testPath] = result;
    if (result[0]) {
      console.log(`${num} %cPASS`, "color: green", testPath);
    } else {
      console.log(`${num} %cFAIL`, "color: red", testPath);
    }
  }
  const [sequential, parallel] = partition(
    tests,
    (test) => getGroupRelUrl(test) === "sequential",
  );
  // Runs sequential tests
  for (const path of sequential) {
    await run(path);
  }
  // Runs parallel tests
  for await (
    const _ of pooledMap(navigator.hardwareConcurrency, parallel, run)
  ) {
    // pass
  }

  // Reporting to stdout
  console.log(`Result by categories (${categoryList.length}):`);
  for (const [category, tests] of categoryList) {
    const s = tests.filter((test) => results[test][0]).length;
    const all = tests.length;
    console.log(`  ${category} ${s}/${all} (${(s / all * 100).toFixed(2)}%)`);
    for (const testPath of tests) {
      if (results[testPath][0]) {
        console.log(`    %cPASS`, "color: green", testPath);
      } else {
        console.log(`    %cFAIL`, "color: red", testPath);
      }
    }
  }

  // Summary
  const total = tests.length;
  const pass = tests.filter((test) => results[test][0]).length;
  console.log(
    `All tests: ${pass}/${total} (${(pass / total * 100).toFixed(2)}%)`,
  );
  console.log(`Elapsed time: ${((Date.now() - start) / 1000).toFixed(2)}s`);
  // Store the results in a JSON file
  await Deno.writeTextFile(
    "tests/node_compat/report.json",
    JSON.stringify(
      {
        date: new Date().toISOString().slice(0, 10),
        denoVersion: Deno.version.deno,
        os: Deno.build.os,
        arch: Deno.build.arch,
        nodeVersion,
        runId: Deno.env.get("GTIHUB_RUN_ID") ?? null,
        total,
        pass,
        results,
      } satisfies TestReport,
    ),
  );
  Deno.exit(0);
}

if (import.meta.main) {
  await main();
}
