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
  configFile,
  parseFlags,
  RUN_ARGS,
  TEST_ARGS,
  usesNodeTestModule,
} from "./common.ts";
import { generateTestSerialId } from "./test.ts";

const testSuitePath = new URL(import.meta.resolve("./runner/suite/"));
const testDirUrl = new URL("runner/suite/test/", import.meta.url).href;
const IS_CI = !!Deno.env.get("CI");
// The timeout ms for single test execution. If a single test didn't finish in this timeout milliseconds, the test is considered as failure
const TIMEOUT = IS_CI
  ? Deno.build.os === "darwin" && Deno.build.arch === "x86_64" ? 20_000 : 10_000
  : 5000;

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
  "known_issues",
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

const runnerOs = Deno.build.os;
const ignoredTests = Object.entries(configFile.tests)
  .flatMap((
    [testName, config],
  ) => {
    if (config[runnerOs as keyof typeof config] !== false) return [];
    return [{ testName, reason: config.reason }];
  });

const NODE_IGNORED_TEST_CASES_TO_REASON = new Map<string, string | undefined>(
  ignoredTests.map(({ testName, reason }) => [testName, reason]),
);

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

interface NonIgnoredTestFileReport {
  result:
    | NodeTestFileResult.PASS
    | NodeTestFileResult.FAIL
    | NodeTestFileResult.SKIP; // Currently skipped tests will not be written to the report
  error?: ErrorExit | ErrorTimeout | ErrorUnexpected;
}

interface IgnoredTestFileReport {
  result: NodeTestFileResult.IGNORED;
  reason: string | undefined;
  error?: undefined;
}

type NodeTestFileReport = (NonIgnoredTestFileReport | IgnoredTestFileReport) & {
  usesNodeTest: boolean; // whether the test uses `node:test` module
};

type TestReports = Record<string, NodeTestFileReport>;

type BaseSingleResultInfo = {
  usesNodeTest?: 1; // Uses this form to minimize the size of the report.json
};

type IgnoredSingleResultInfo = BaseSingleResultInfo & {
  ignoreReason: string | undefined;
};

type NonIgnoredSingleResult = [
  pass: boolean,
  error: ErrorExit | ErrorTimeout | ErrorUnexpected | undefined,
  info: BaseSingleResultInfo,
];

type IgnoredSingleResult = [
  pass: "IGNORE",
  error: undefined,
  info: IgnoredSingleResultInfo,
];

export type SingleResult = NonIgnoredSingleResult | IgnoredSingleResult;

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
 * @param testPath Relative path from test/ dir of Node.js (e.g. "parallel/test-assert.js").
 */
export async function runSingle(
  testPath: string,
  {
    flaky = !!Deno.env.get("CI"),
    retry = 0,
  }: {
    flaky?: boolean;
    retry?: number;
  },
): Promise<NodeTestFileReport> {
  const testSerialId = generateTestSerialId();
  let cmd: Deno.ChildProcess | undefined;
  const testPath_ = "test/" + testPath;
  let usesNodeTest = false;
  try {
    const testFileUrl = new URL(testPath_, testSuitePath);
    const source = await Deno.readTextFile(testFileUrl);
    usesNodeTest = usesNodeTestModule(source);
    if (NODE_IGNORED_TEST_CASES_TO_REASON.has(testPath)) {
      return {
        result: NodeTestFileResult.IGNORED,
        usesNodeTest,
        reason: NODE_IGNORED_TEST_CASES_TO_REASON.get(testPath),
      };
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
        TEST_SERIAL_ID: String(testSerialId),
      },
      stdout: "piped",
      stderr: "piped",
      cwd: testSuitePath,
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
      return runSingle(testPath, { flaky, retry: retry + 1 });
    } else if (flaky && retry < 5) {
      await new Promise((resolve) => setTimeout(resolve, 100 * retry));
      return runSingle(testPath, { flaky, retry: retry + 1 });
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
    const info = {} as BaseSingleResultInfo;
    if (value.usesNodeTest) {
      info.usesNodeTest = 1;
    }
    let result: SingleResult = [true, undefined, info];
    if (value.result === NodeTestFileResult.FAIL) {
      result = [false, value.error, info];
    } else if (value.result === NodeTestFileResult.IGNORED) {
      // @ts-expect-error info is now `IgnoredSingleResultInfo`
      info.ignoreReason = value.reason;
      result = ["IGNORE", undefined, info as IgnoredSingleResultInfo];
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
    const result = await runSingle(testPath, {});
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

      reports[term] = { result: NodeTestFileResult.SKIP, usesNodeTest: false };
      return false;
    });
    parallel = parallel.filter((term) => {
      if (term.includes(filterTerm)) {
        return true;
      }
      reports[term] = { result: NodeTestFileResult.SKIP, usesNodeTest: false };
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
    const _ of pooledMap(
      Math.max(1, navigator.hardwareConcurrency - 1),
      parallel,
      run,
    )
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
          // @ts-expect-error unknown result type
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
