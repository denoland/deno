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
import {
  parseFlags,
  RUN_ARGS,
  TEST_ARGS,
  usesNodeTestModule,
} from "./common.ts";

// The timeout ms for single test execution. If a single test didn't finish in this timeout milliseconds, the test is considered as failure
const TIMEOUT = 2000;
const testDirUrl = new URL("runner/suite/test/", import.meta.url).href;
const IS_CI = !!Deno.env.get("CI");

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

enum NodeTestFileResult {
  PASS = "pass",
  FAIL = "fail",
  SKIP = "skip",
}

interface NodeTestFileReport {
  result: NodeTestFileResult;
  error?: ErrorExit | ErrorTimeout | ErrorUnexpected;
}

type TestReports = Record<string, NodeTestFileReport>;

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

function getV8Flags(source: string): string[] {
  const v8Flags = [] as string[];
  const flags = parseFlags(source);
  flags.forEach((flag) => {
    switch (flag) {
      case "--expose_externalize_string":
        v8Flags.push("--expose-externalize-string");
        break;
      case "--expose-gc":
        v8Flags.push("--expose-gc");
        break;
      default:
        break;
    }
  });
  return v8Flags;
}

/**
 * Run a single node test file. Retries 3 times on WouldBlock error.
 *
 * @param testPath Relative path to the test file
 */
async function runSingle(
  testPath: string,
  retry = 0,
): Promise<NodeTestFileReport> {
  let cmd: Deno.ChildProcess | undefined;
  const testPath_ = "tests/node_compat/runner/suite/test/" + testPath;
  try {
    const source = await Deno.readTextFile(testPath_);
    const usesNodeTest = usesNodeTestModule(source);
    const v8Flags = getV8Flags(source);
    cmd = new Deno.Command(Deno.execPath(), {
      args: [
        ...(usesNodeTest ? TEST_ARGS : RUN_ARGS),
        ...(v8Flags.length > 0 ? ["--v8-flags=" + v8Flags.join(",")] : []),
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
      return { result: NodeTestFileResult.PASS };
    } else {
      const output = usesNodeTest ? result.stdout : result.stderr;
      const outputText = new TextDecoder().decode(output);
      const stderr = IS_CI ? truncateTestOutput(outputText) : outputText;
      return {
        result: NodeTestFileResult.FAIL,
        error: {
          code: result.code,
          stderr,
        },
      };
    }
  } catch (e) {
    if (e instanceof DOMException && e.name === "TimeoutError") {
      try {
        cmd?.kill();
      } catch {
        // ignore
      }
      return { result: NodeTestFileResult.FAIL, error: { timeout: TIMEOUT } };
    } else if (e instanceof Deno.errors.WouldBlock && retry < 3) {
      // retry 2 times on WouldBlock error (Resource temporarily unavailable)
      return runSingle(testPath, retry + 1);
    } else {
      return {
        result: NodeTestFileResult.FAIL,
        error: { message: (e as Error).message },
      };
    }
  }
}

function transformReportsIntoResults(
  reports: TestReports,
) {
  const results = {} as Record<string, SingleResult>;

  for (const [key, value] of Object.entries(reports)) {
    if (value.result === NodeTestFileResult.SKIP) {
      throw new Error("Can't transform 'SKIP' result into `SingleResult`");
    }
    let result: SingleResult = [true];
    if (value.result === NodeTestFileResult.FAIL) {
      result = [false, value.error];
    }
    results[key] = result;
  }

  return results;
}

async function writeTestReport(
  reports: TestReports,
  total: number,
  pass: number,
) {
  // First transform the results - before we added `NodeTestFileReport` we used `SingleResult`.
  // For now we opt to keep that format, as migrating existing results is cumbersome.
  const results = transformReportsIntoResults(reports);

  await Deno.writeTextFile(
    "tests/node_compat/report.json",
    JSON.stringify(
      {
        date: new Date().toISOString().slice(0, 10),
        denoVersion: Deno.version.deno,
        os: Deno.build.os,
        arch: Deno.build.arch,
        nodeVersion,
        runId: Deno.env.get("GITHUB_RUN_ID") ?? null,
        total,
        pass,
        results,
      } satisfies TestReport,
    ),
  );
}

async function main() {
  const filterIdx = Deno.args.indexOf("--filter");
  let filterTerm = undefined;

  // Filtering can only be done locally, we want to avoid having CI run only a subset of tests.
  if (!IS_CI && filterIdx > -1) {
    filterTerm = Deno.args[filterIdx + 1];
  }

  const start = Date.now();
  const tests = [] as string[];
  const categories = {} as Record<string, string[]>;
  for await (
    const test of expandGlob(
      "tests/node_compat/runner/suite/**/test-*{.mjs,.cjs.,.js,.ts}",
    )
  ) {
    if (!test.isFile) continue;
    const relUrl = toFileUrl(test.path).href.replace(testDirUrl, "");
    if (NODE_IGNORED_TEST_DIRS.every((dir) => !relUrl.startsWith(dir))) {
      tests.push(relUrl);
      (categories[getCategoryFromPath(relUrl)] ??= []).push(relUrl);
    }
  }
  collectNonCategorizedItems(categories);
  const categoryList = Object.entries(categories)
    .sort(([c0], [c1]) => c0.localeCompare(c1));
  const reports = {} as TestReports;
  let i = 0;

  async function run(testPath: string) {
    const num = String(++i).padStart(4, " ");
    const result = await runSingle(testPath);
    reports[testPath] = result;
    if (result.result === NodeTestFileResult.PASS) {
      console.log(`${num} %cPASS`, "color: green", testPath);
    } else if (result.result === NodeTestFileResult.FAIL) {
      console.log(`${num} %cFAIL`, "color: red", testPath);
    } else {
      // Don't print message for "skip" for now, as it's too noisy
      // console.log(`${num} %cSKIP`, "color: yellow", testPath);
    }
  }

  let [sequential, parallel] = partition(
    tests,
    (test) => getGroupRelUrl(test) === "sequential",
  );

  console.log;
  if (filterTerm) {
    sequential = sequential.filter((term) => {
      if (term.includes(filterTerm)) {
        return true;
      }

      reports[term] = { result: NodeTestFileResult.SKIP };
      return false;
    });
    parallel = parallel.filter((term) => {
      if (term.includes(filterTerm)) {
        return true;
      }
      reports[term] = { result: NodeTestFileResult.SKIP };
      return false;
    });
    console.log(
      `Found ${sequential.length} sequential tests and ${parallel.length} parallel tests`,
    );
  }

  console.log("Running", sequential.length + parallel.length, "tests");
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
    if (
      tests.every((test) => reports[test].result === NodeTestFileResult.SKIP)
    ) {
      continue;
    }
    const s = tests.filter((test) =>
      reports[test].result === NodeTestFileResult.PASS
    ).length;
    const all = filterTerm
      ? tests.map((testPath) => reports[testPath].result).filter((result) =>
        result !== NodeTestFileResult.SKIP
      ).length
      : tests.length;
    console.log(`  ${category} ${s}/${all} (${(s / all * 100).toFixed(2)}%)`);
    for (const testPath of tests) {
      switch (reports[testPath].result) {
        case NodeTestFileResult.PASS: {
          console.log(`    %cPASS`, "color: green", testPath);
          break;
        }
        case NodeTestFileResult.FAIL: {
          // deno-lint-ignore no-explicit-any
          let elements: any[] = [];
          const error = reports[testPath].error!;
          if (error.code) {
            elements = ["exit code:", error.code, "\n   ", error.stderr];
          } else if (error.timeout) {
            elements = ["timeout out after", error.timeout, "seconds"];
          } else {
            elements = ["errored with:", error.message];
          }
          console.log(`    %cFAIL`, "color: red", testPath);
          console.log("   ", ...elements);
          break;
        }
        case NodeTestFileResult.SKIP: {
          // Don't print message for "skip" for now, as it's too noisy
          // console.log(`    %cSKIP`, "color: yellow", testPath);
          break;
        }
        default:
          console.warn(
            `Unknown result (${reports[testPath].result}) for ${testPath}`,
          );
      }
    }
  }

  // Summary
  let total;
  const pass =
    tests.filter((test) => reports[test].result === NodeTestFileResult.PASS)
      .length;
  if (filterTerm) {
    total = tests.map((testPath) =>
      reports[testPath].result
    ).filter((result) => result !== NodeTestFileResult.SKIP).length;
    console.log(
      `Filtered tests: ${pass}/${total} (${(pass / total * 100).toFixed(2)}%)`,
    );
  } else {
    total = tests.length;
    console.log(
      `All tests: ${pass}/${total} (${(pass / total * 100).toFixed(2)}%)`,
    );
  }

  console.log(`Elapsed time: ${((Date.now() - start) / 1000).toFixed(2)}s`);
  // Store the results in a JSON file

  if (!filterTerm) {
    await writeTestReport(reports, total, pass);
  }

  Deno.exit(0);
}

if (import.meta.main) {
  await main();
}
