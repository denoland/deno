// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// This script runs all Node.js test cases without modification
// It saves the test results in `tests/node_compat/report.json` when
// filtering is not specified.
// The saved results are uploaded to cloud storage bucket `dl.deno.land`
// daily, and can be viewed at the web page:
// https://node-test-viewer.deno.dev/

import { deadline } from "@std/async/deadline";
import { expandGlob } from "@std/fs/expand-glob";
import { toFileUrl } from "@std/path/to-file-url";
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
  ignore: number;
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

const NODE_IGNORED_TEST_CASES = new Set([
  "parallel/test-benchmark-cli.js", // testing private benchmark utility
  "parallel/test-buffer-backing-arraybuffer.js", // Deno does not allow heap-allocated ArrayBuffer, and we can't change it (for now)
  "parallel/test-eventsource-disabled.js", // EventSource global is always available in Deno (Web API)
  "parallel/test-crypto-secure-heap.js", // Secure heap is OpenSSL specific, not in Deno.
]);

/** The group is the directory name of the test file.
 * e.g. parallel, internet, pummel, sequential, pseudo-tty, etc */
function getGroupRelUrl(str: string) {
  return str.split("/")[0];
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
  SKIP = "skip", // skipped because of filtering (for debugging locally)
  IGNORED = "ignored", // ignored because the test case does not need to pass in Deno
}

interface NodeTestFileReport {
  result: NodeTestFileResult;
  error?: ErrorExit | ErrorTimeout | ErrorUnexpected;
  usesNodeTest: boolean; // whether the test uses `node:test` module
}

type TestReports = Record<string, NodeTestFileReport>;

type SingleResultInfo = {
  usesNodeTest?: 1; // Uses this form to minimize the size of the report.json
};

export type SingleResult = [
  pass: boolean | "IGNORE",
  error: ErrorExit | ErrorTimeout | ErrorUnexpected | undefined,
  info: SingleResultInfo,
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

function getFlags(source: string): [string[], string[]] {
  const v8Flags = [] as string[];
  const nodeOptions = [] as string[];
  const flags = parseFlags(source);
  flags.forEach((flag) => {
    switch (flag) {
      case "--expose_externalize_string":
        v8Flags.push("--expose-externalize-string");
        break;
      case "--expose-gc":
        v8Flags.push("--expose-gc");
        break;
      case "--no-warnings":
        nodeOptions.push("--no-warnings");
        break;
      case "--pending-deprecation":
        nodeOptions.push("--pending-deprecation");
        break;
      case "--allow-natives-syntax":
        v8Flags.push("--allow-natives-syntax");
        break;
      default:
        break;
    }
  });
  return [v8Flags, nodeOptions];
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
  let usesNodeTest = false;
  try {
    const source = await Deno.readTextFile(testPath_);
    usesNodeTest = usesNodeTestModule(source);
    if (NODE_IGNORED_TEST_CASES.has(testPath)) {
      return { result: NodeTestFileResult.IGNORED, usesNodeTest };
    }
    const [v8Flags, nodeOptions] = getFlags(source);
    cmd = new Deno.Command(Deno.execPath(), {
      args: [
        ...(usesNodeTest ? TEST_ARGS : RUN_ARGS),
        ...(v8Flags.length > 0 ? ["--v8-flags=" + v8Flags.join(",")] : []),
        testPath_,
      ],
      env: {
        NODE_TEST_KNOWN_GLOBALS: "0",
        NODE_SKIP_FLAG_CHECK: "1",
        NODE_OPTIONS: nodeOptions.join(" "),
        NO_COLOR: "1",
      },
      stdout: "piped",
      stderr: "piped",
    }).spawn();
    const result = await deadline(cmd.output(), TIMEOUT);
    if (result.code === 0) {
      return { result: NodeTestFileResult.PASS, usesNodeTest };
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
        usesNodeTest,
      };
    }
  } catch (e) {
    if (e instanceof DOMException && e.name === "TimeoutError") {
      try {
        cmd?.kill();
      } catch {
        // ignore
      }
      return {
        result: NodeTestFileResult.FAIL,
        error: { timeout: TIMEOUT },
        usesNodeTest,
      };
    } else if (e instanceof Deno.errors.WouldBlock && retry < 3) {
      // retry 2 times on WouldBlock error (Resource temporarily unavailable)
      return runSingle(testPath, retry + 1);
    } else {
      return {
        result: NodeTestFileResult.FAIL,
        error: { message: (e as Error).message },
        usesNodeTest,
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
    const info = {} as SingleResultInfo;
    if (value.usesNodeTest) {
      info.usesNodeTest = 1;
    }
    let result: SingleResult = [true, undefined, info];
    if (value.result === NodeTestFileResult.FAIL) {
      result = [false, value.error, info];
    } else if (value.result === NodeTestFileResult.IGNORED) {
      result = ["IGNORE", undefined, info];
    }
    results[key] = result;
  }

  return results;
}

async function writeTestReport(
  reports: TestReports,
  total: number,
  pass: number,
  ignore: number,
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
        ignore,
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
  for await (
    const test of expandGlob(
      "tests/node_compat/runner/suite/**/test-*{.mjs,.cjs.,.js,.ts}",
    )
  ) {
    if (!test.isFile) continue;
    const relUrl = toFileUrl(test.path).href.replace(testDirUrl, "");
    if (NODE_IGNORED_TEST_DIRS.every((dir) => !relUrl.startsWith(dir))) {
      tests.push(relUrl);
    }
  }
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
    } else if (result.result === NodeTestFileResult.IGNORED) {
      console.log(`${num} %cIGNORED`, "color: gray", testPath);
    } else {
      // Don't print message for "skip" for now, as it's too noisy
      // console.log(`${num} %cSKIP`, "color: yellow", testPath);
    }
  }

  let [sequential, parallel] = partition(
    tests,
    (test) => getGroupRelUrl(test) === "sequential",
  );

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

  for (const [testPath, fileResult] of Object.entries(reports)) {
    switch (fileResult.result) {
      case NodeTestFileResult.PASS: {
        console.log(`    %cPASS`, "color: green", testPath);
        break;
      }
      case NodeTestFileResult.FAIL: {
        let elements: string[] = [];
        const error = fileResult.error!;
        if ("code" in error) {
          elements = [`exit code: ${error.code}\n   `, error.stderr];
        } else if ("timeout" in error) {
          elements = [`timeout out after ${error.timeout} seconds`];
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
      case NodeTestFileResult.IGNORED: {
        console.log(`    %cIGNORED`, "color: gray", testPath);
        break;
      }
      default:
        console.warn(
          `Unknown result (${fileResult.result}) for ${testPath}`,
        );
    }
  }

  // Summary
  const pass =
    tests.filter((test) => reports[test].result === NodeTestFileResult.PASS)
      .length;
  const fail =
    tests.filter((test) => reports[test].result === NodeTestFileResult.FAIL)
      .length;
  const ignore =
    tests.filter((test) => reports[test].result === NodeTestFileResult.IGNORED)
      .length;
  const total = pass + fail;
  if (filterTerm) {
    console.log(
      `Filtered tests: ${pass}/${total} (${(pass / total * 100).toFixed(2)}%)`,
    );
  } else {
    console.log(
      `All tests: ${pass}/${total} (${(pass / total * 100).toFixed(2)}%)`,
    );
  }

  console.log(`Elapsed time: ${((Date.now() - start) / 1000).toFixed(2)}s`);
  // Store the results in a JSON file

  if (!filterTerm) {
    await writeTestReport(reports, total, pass, ignore);
  }

  Deno.exit(0);
}

if (import.meta.main) {
  await main();
}
