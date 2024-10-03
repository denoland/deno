#!/usr/bin/env -S deno run --allow-write --allow-read --allow-net --allow-env --allow-run --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

// This script is used to run WPT tests for Deno.

import {
  runSingleTest,
  runWithTestUtil,
  TestCaseResult,
  TestResult,
} from "./runner/runner.ts";
import {
  assert,
  autoConfig,
  cargoBuild,
  checkPy3Available,
  escapeLoneSurrogates,
  Expectation,
  EXPECTATION_PATH,
  generateRunInfo,
  getExpectation,
  getExpectFailForCase,
  getManifest,
  inspectBrk,
  json,
  ManifestFolder,
  ManifestTestOptions,
  ManifestTestVariation,
  noIgnore,
  quiet,
  rest,
  runGitDiff,
  runPy,
  updateManifest,
  wptreport,
} from "./runner/utils.ts";
import { pooledMap } from "@std/async/pool";
import { blue, bold, green, red, yellow } from "@std/fmt/colors";
import { writeAll, writeAllSync } from "@std/io/write-all";
import { saveExpectation } from "./runner/utils.ts";

class TestFilter {
  filter?: string[];
  constructor(filter?: string[]) {
    this.filter = filter;
  }

  matches(path: string): boolean {
    if (this.filter === undefined || this.filter.length == 0) {
      return true;
    }
    for (const filter of this.filter) {
      if (filter.startsWith("/")) {
        if (path.startsWith(filter)) {
          return true;
        }
      } else {
        if (path.substring(1).startsWith(filter)) {
          return true;
        }
      }
    }
    return false;
  }
}

const command = Deno.args[0];

switch (command) {
  case "setup":
    await checkPy3Available();
    await updateManifest();
    await setup();
    break;

  case "run":
    await checkPy3Available();
    await cargoBuild();
    await run();
    break;

  case "update":
    await cargoBuild();
    await update();
    break;

  default:
    console.log(`Possible commands:

    setup
      Validate that your environment is configured correctly, or help you configure it.

    run
      Run all tests like specified in \`expectation.json\`.

    update
      Update the \`expectation.json\` to match the current reality.

More details at https://docs.deno.com/runtime/manual/references/contributing/web_platform_tests

    `);
    break;
}

async function setup() {
  const hostsPath = Deno.build.os == "windows"
    ? `${Deno.env.get("SystemRoot")}\\System32\\drivers\\etc\\hosts`
    : "/etc/hosts";
  // TODO(lucacsonato): use this when 1.7.1 is released.
  // const records = await Deno.resolveDns("web-platform.test", "A");
  // const etcHostsConfigured = records[0] == "127.0.0.1";
  const hostsFile = await Deno.readTextFile(hostsPath);
  const etcHostsConfigured = hostsFile.includes("web-platform.test");

  if (etcHostsConfigured) {
    console.log(hostsPath + " is already configured.");
  } else {
    const autoConfigure = autoConfig ||
      confirm(
        `The WPT require certain entries to be present in your ${hostsPath} file. Should these be configured automatically?`,
      );
    if (autoConfigure) {
      const { success, stdout } = await runPy(["wpt", "make-hosts-file"], {
        stdout: "piped",
      }).output();
      assert(success, "wpt make-hosts-file should not fail");
      const entries = new TextDecoder().decode(stdout);
      const file = await Deno.open(hostsPath, { append: true }).catch((err) => {
        if (err instanceof Deno.errors.PermissionDenied) {
          throw new Error(
            `Failed to open ${hostsPath} (permission error). Please run this command again with sudo, or configure the entries manually.`,
          );
        } else {
          throw err;
        }
      });
      await writeAll(
        file,
        new TextEncoder().encode(
          "\n\n# Configured for Web Platform Tests (Deno)\n" + entries,
        ),
      );
      console.log(`Updated ${hostsPath}`);
    } else {
      console.log(`Please configure the ${hostsPath} entries manually.`);
      if (Deno.build.os == "windows") {
        console.log("To do this run the following command in PowerShell:");
        console.log("");
        console.log("    cd tests/wpt/suite/");
        console.log(
          "    python.exe wpt make-hosts-file | Out-File $env:SystemRoot\\System32\\drivers\\etc\\hosts -Encoding ascii -Append",
        );
        console.log("");
      } else {
        console.log("To do this run the following command in your shell:");
        console.log("");
        console.log("    cd tests/wpt/suite/");
        console.log(
          "    python3 ./wpt make-hosts-file | sudo tee -a /etc/hosts",
        );
        console.log("");
      }
    }
  }

  console.log(green("Setup complete!"));
}

interface TestToRun {
  path: string;
  url: URL;
  options: ManifestTestOptions;
  expectation: boolean | string[];
}

function getTestTimeout(test: TestToRun) {
  if (Deno.env.get("CI")) {
    // Don't give expected failures the full time
    if (test.expectation === false) {
      return { long: 60_000, default: 10_000 };
    }
    return { long: 4 * 60_000, default: 4 * 60_000 };
  }

  return { long: 60_000, default: 10_000 };
}

async function run() {
  const startTime = new Date().getTime();
  assert(Array.isArray(rest), "filter must be array");
  const expectation = getExpectation();
  const filter = new TestFilter(rest);
  const tests = discoverTestsToRun(
    filter,
    expectation,
  );
  assertAllExpectationsHaveTests(expectation, tests, filter);
  const cores = navigator.hardwareConcurrency;
  console.log(`Going to run ${tests.length} test files on ${cores} cores.`);

  const results = await runWithTestUtil(false, async () => {
    const results: { test: TestToRun; result: TestResult }[] = [];
    const inParallel = !(cores === 1 || tests.length === 1);
    // ideally we would parallelize all tests, but we ran into some flakiness
    // on the CI, so here we're partitioning based on the start of the test path
    const partitionedTests = partitionTests(tests);

    const iter = pooledMap(cores, partitionedTests, async (tests) => {
      for (const test of tests) {
        if (!inParallel) {
          console.log(`${blue("-".repeat(40))}\n${bold(test.path)}\n`);
        }
        const result = await runSingleTest(
          test.url,
          test.options,
          inParallel ? () => {} : createReportTestCase(test.expectation),
          inspectBrk,
          getTestTimeout(test),
        );
        results.push({ test, result });
        if (inParallel) {
          console.log(`${blue("-".repeat(40))}\n${bold(test.path)}\n`);
        }
        reportVariation(result, test.expectation);
      }
    });

    for await (const _ of iter) {
      // ignore
    }

    return results;
  });
  const endTime = new Date().getTime();

  if (json) {
    const minifiedResults = [];
    for (const result of results) {
      const minified = {
        file: result.test.path,
        name:
          Object.fromEntries(result.test.options.script_metadata ?? []).title ??
            null,
        cases: result.result.cases.map((case_) => ({
          name: case_.name,
          passed: case_.passed,
        })),
      };
      minifiedResults.push(minified);
    }
    await Deno.writeTextFile(json, JSON.stringify(minifiedResults) + "\n");
  }

  if (wptreport) {
    const report = await generateWptReport(results, startTime, endTime);
    await Deno.writeTextFile(wptreport, JSON.stringify(report) + "\n");
  }

  const newExpectations = newExpectation(results);
  const tmp = Deno.makeTempFileSync();
  saveExpectation(newExpectations, tmp);

  const code = reportFinal(results, endTime - startTime);

  // Run git diff to see what changed
  await runGitDiff([EXPECTATION_PATH, tmp]);
  Deno.removeSync(tmp);

  Deno.exit(code);
}

async function generateWptReport(
  results: { test: TestToRun; result: TestResult }[],
  startTime: number,
  endTime: number,
) {
  const runInfo = await generateRunInfo();
  const reportResults = [];
  for (const { test, result } of results) {
    const status = result.status !== 0
      ? "CRASH"
      : result.harnessStatus?.status === 0
      ? "OK"
      : "ERROR";
    let message;
    if (result.harnessStatus === null && result.status === 0) {
      // If the only error is the event loop running out of tasks, using stderr
      // as the message won't help.
      message = "Event loop run out of tasks.";
    } else {
      message = result.harnessStatus?.message ?? (result.stderr.trim() || null);
    }
    const reportResult = {
      test: test.url.pathname + test.url.search + test.url.hash,
      subtests: result.cases.map((case_) => {
        let expected = undefined;
        if (!case_.passed) {
          if (typeof test.expectation === "boolean") {
            expected = test.expectation ? "PASS" : "FAIL";
          } else if (Array.isArray(test.expectation)) {
            expected = test.expectation.includes(case_.name) ? "FAIL" : "PASS";
          } else {
            expected = "PASS";
          }
        }

        return {
          name: escapeLoneSurrogates(case_.name),
          status: case_.passed ? "PASS" : "FAIL",
          message: escapeLoneSurrogates(case_.message),
          expected,
          known_intermittent: [],
        };
      }),
      status,
      message: escapeLoneSurrogates(message),
      duration: result.duration,
      expected: status === "OK" ? undefined : "OK",
      "known_intermittent": [],
    };
    reportResults.push(reportResult);
  }
  return {
    "run_info": runInfo,
    "time_start": startTime,
    "time_end": endTime,
    "results": reportResults,
  };
}

// Check that all expectations in the expectations file have a test that will be
// run.
function assertAllExpectationsHaveTests(
  expectation: Expectation,
  testsToRun: TestToRun[],
  filter: TestFilter,
): void {
  const tests = new Set(testsToRun.map((t) => t.path));
  const missingTests: string[] = [];
  function walk(parentExpectation: Expectation, parent: string) {
    for (const [key, expectation] of Object.entries(parentExpectation)) {
      const path = `${parent}/${key}`;
      if (!filter.matches(path)) continue;
      if (
        (typeof expectation == "boolean" || Array.isArray(expectation)) &&
        key !== "ignore"
      ) {
        if (!tests.has(path)) {
          missingTests.push(path);
        }
      } else {
        walk(expectation, path);
      }
    }
  }

  walk(expectation, "");

  if (missingTests.length > 0) {
    console.log(
      red(
        "Following tests are missing in manifest, but are present in expectations:",
      ),
    );
    console.log("");
    console.log(missingTests.join("\n"));
    Deno.exit(1);
  }
}

async function update() {
  assert(Array.isArray(rest), "filter must be array");
  const startTime = new Date().getTime();
  const filter = new TestFilter(rest);
  const tests = discoverTestsToRun(filter, true);
  console.log(`Going to run ${tests.length} test files.`);

  const results = await runWithTestUtil(false, async () => {
    const results = [];

    for (const test of tests) {
      console.log(`${blue("-".repeat(40))}\n${bold(test.path)}\n`);
      const result = await runSingleTest(
        test.url,
        test.options,
        json ? () => {} : createReportTestCase(test.expectation),
        inspectBrk,
        { long: 60_000, default: 10_000 },
      );
      results.push({ test, result });
      reportVariation(result, test.expectation);
    }

    return results;
  });
  const endTime = new Date().getTime();

  if (json) {
    await Deno.writeTextFile(json, JSON.stringify(results) + "\n");
  }

  const newExpectations = newExpectation(results);
  saveExpectation(newExpectations);

  reportFinal(results, endTime - startTime);

  console.log(blue("Updated expectation.json to match reality."));

  Deno.exit(0);
}

function newExpectation(
  results: { test: TestToRun; result: TestResult }[],
): Expectation {
  const resultTests: Record<
    string,
    { passed: string[]; failed: string[]; testSucceeded: boolean }
  > = {};
  for (const { test, result } of results) {
    if (!resultTests[test.path]) {
      resultTests[test.path] = {
        passed: [],
        failed: [],
        testSucceeded: result.status === 0 && result.harnessStatus !== null,
      };
    }
    for (const case_ of result.cases) {
      if (case_.passed) {
        resultTests[test.path].passed.push(case_.name);
      } else {
        resultTests[test.path].failed.push(case_.name);
      }
    }
  }

  const currentExpectation = getExpectation();

  for (const [path, result] of Object.entries(resultTests)) {
    const { passed, failed, testSucceeded } = result;
    let finalExpectation: boolean | string[];
    if (failed.length == 0 && testSucceeded) {
      finalExpectation = true;
    } else if (failed.length > 0 && passed.length > 0 && testSucceeded) {
      finalExpectation = failed;
    } else {
      finalExpectation = false;
    }

    insertExpectation(
      path.slice(1).split("/"),
      currentExpectation,
      finalExpectation,
    );
  }

  return currentExpectation;
}

function insertExpectation(
  segments: string[],
  currentExpectation: Expectation,
  finalExpectation: boolean | string[],
) {
  const segment = segments.shift();
  assert(segment, "segments array must never be empty");
  if (segments.length > 0) {
    if (
      currentExpectation[segment] === undefined ||
      Array.isArray(currentExpectation[segment]) ||
      typeof currentExpectation[segment] === "boolean"
    ) {
      currentExpectation[segment] = {};
    }
    insertExpectation(
      segments,
      currentExpectation[segment] as Expectation,
      finalExpectation,
    );
  } else {
    if (
      currentExpectation[segment] === undefined ||
      Array.isArray(currentExpectation[segment]) ||
      typeof currentExpectation[segment] === "boolean" ||
      (currentExpectation[segment] as { ignore: boolean })?.ignore !== true
    ) {
      currentExpectation[segment] = finalExpectation;
    }
  }
}

function reportFinal(
  results: { test: TestToRun; result: TestResult }[],
  duration: number,
): number {
  const finalTotalCount = results.length;
  let finalFailedCount = 0;
  const finalFailed: [string, TestCaseResult][] = [];
  let finalExpectedFailedAndFailedCount = 0;
  const finalExpectedFailedButPassedTests: [string, TestCaseResult][] = [];
  const finalExpectedFailedButPassedFiles: string[] = [];
  const finalFailedFiles: string[] = [];
  for (const { test, result } of results) {
    const {
      failed,
      failedCount,
      expectedFailedButPassed,
      expectedFailedAndFailedCount,
    } = analyzeTestResult(
      result,
      test.expectation,
    );
    if (result.status !== 0 || result.harnessStatus === null) {
      if (test.expectation === false) {
        finalExpectedFailedAndFailedCount += 1;
      } else {
        finalFailedCount += 1;
        finalFailedFiles.push(test.path);
      }
    } else if (failedCount > 0) {
      finalFailedCount += 1;
      for (const case_ of failed) {
        finalFailed.push([test.path, case_]);
      }
      for (const case_ of expectedFailedButPassed) {
        finalExpectedFailedButPassedTests.push([test.path, case_]);
      }
    } else if (
      test.expectation === false &&
      expectedFailedAndFailedCount != result.cases.length
    ) {
      finalExpectedFailedButPassedFiles.push(test.path);
    }
  }
  const finalPassedCount = finalTotalCount - finalFailedCount;

  console.log(bold(blue("=".repeat(40))));

  if (finalFailed.length > 0) {
    console.log(`\nfailures:\n`);
  }
  for (const result of finalFailed) {
    console.log(
      `        ${JSON.stringify(`${result[0]} - ${result[1].name}`)}`,
    );
  }
  if (finalFailedFiles.length > 0) {
    console.log(`\nfile failures:\n`);
  }
  for (const result of finalFailedFiles) {
    console.log(
      `        ${JSON.stringify(result)}`,
    );
  }
  if (finalExpectedFailedButPassedTests.length > 0) {
    console.log(`\nexpected test failures that passed:\n`);
  }
  for (const result of finalExpectedFailedButPassedTests) {
    console.log(
      `        ${JSON.stringify(`${result[0]} - ${result[1].name}`)}`,
    );
  }
  if (finalExpectedFailedButPassedFiles.length > 0) {
    console.log(`\nexpected file failures that passed:\n`);
  }
  for (const result of finalExpectedFailedButPassedFiles) {
    console.log(`        ${JSON.stringify(result)}`);
  }

  const failed = (finalFailedCount > 0) ||
    (finalExpectedFailedButPassedFiles.length > 0);

  console.log(
    `\nfinal result: ${
      failed ? red("failed") : green("ok")
    }. ${finalPassedCount} passed; ${finalFailedCount} failed; ${finalExpectedFailedAndFailedCount} expected failure; total ${finalTotalCount} (${duration}ms)\n`,
  );

  // We ignore the exit code of the test run because the CI job reports the
  // results to WPT.fyi, and we still want to report failure.
  if (Deno.args.includes("--exit-zero")) {
    return 0;
  }

  return failed ? 1 : 0;
}

function analyzeTestResult(
  result: TestResult,
  expectation: boolean | string[],
): {
  failed: TestCaseResult[];
  failedCount: number;
  passedCount: number;
  totalCount: number;
  expectedFailedButPassed: TestCaseResult[];
  expectedFailedButPassedCount: number;
  expectedFailedAndFailedCount: number;
} {
  const failed = result.cases.filter(
    (t) => !getExpectFailForCase(expectation, t.name) && !t.passed,
  );
  const expectedFailedButPassed = result.cases.filter(
    (t) => getExpectFailForCase(expectation, t.name) && t.passed,
  );
  const expectedFailedButPassedCount = expectedFailedButPassed.length;
  const failedCount = failed.length + expectedFailedButPassedCount;
  const expectedFailedAndFailedCount = result.cases.filter(
    (t) => getExpectFailForCase(expectation, t.name) && !t.passed,
  ).length;
  const totalCount = result.cases.length;
  const passedCount = totalCount - failedCount - expectedFailedAndFailedCount;

  return {
    failed,
    failedCount,
    passedCount,
    totalCount,
    expectedFailedButPassed,
    expectedFailedButPassedCount,
    expectedFailedAndFailedCount,
  };
}

function reportVariation(result: TestResult, expectation: boolean | string[]) {
  if (result.status !== 0 || result.harnessStatus === null) {
    if (result.stderr) {
      console.log(`test stderr:\n${result.stderr}\n`);
    }

    const expectFail = expectation === false;
    const failReason = result.status !== 0
      ? "runner failed during test"
      : "the event loop run out of tasks during the test";
    console.log(
      `\nfile result: ${
        expectFail ? yellow("failed (expected)") : red("failed")
      }. ${failReason} (${formatDuration(result.duration)})\n`,
    );
    return;
  }

  const {
    failed,
    failedCount,
    passedCount,
    totalCount,
    expectedFailedButPassed,
    expectedFailedButPassedCount,
    expectedFailedAndFailedCount,
  } = analyzeTestResult(result, expectation);

  if (failed.length > 0) {
    console.log(`\nfailures:`);
  }
  for (const result of failed) {
    console.log(`\n${result.name}\n${result.message}\n${result.stack}`);
  }

  if (failedCount > 0) {
    console.log(`\nfailures:\n`);
  }
  for (const result of failed) {
    console.log(`        ${JSON.stringify(result.name)}`);
  }
  if (expectedFailedButPassedCount > 0) {
    console.log(`\nexpected failures that passed:\n`);
  }
  for (const result of expectedFailedButPassed) {
    console.log(`        ${JSON.stringify(result.name)}`);
  }
  if (result.stderr) {
    console.log("\ntest stderr:\n" + result.stderr);
  }
  console.log(
    `\nfile result: ${
      failedCount > 0 ? red("failed") : green("ok")
    }. ${passedCount} passed; ${failedCount} failed; ${expectedFailedAndFailedCount} expected failure; total ${totalCount} (${
      formatDuration(result.duration)
    })\n`,
  );
}

function createReportTestCase(expectation: boolean | string[]) {
  return function reportTestCase({ name, status }: TestCaseResult) {
    const expectFail = getExpectFailForCase(expectation, name);
    let simpleMessage = `test ${name} ... `;
    switch (status) {
      case 0:
        if (expectFail) {
          simpleMessage += red("ok (expected fail)");
        } else {
          simpleMessage += green("ok");
          if (quiet) {
            // don't print `ok` tests if --quiet is enabled
            return;
          }
        }
        break;
      case 1:
        if (expectFail) {
          simpleMessage += yellow("failed (expected)");
        } else {
          simpleMessage += red("failed");
        }
        break;
      case 2:
        if (expectFail) {
          simpleMessage += yellow("failed (expected)");
        } else {
          simpleMessage += red("failed (timeout)");
        }
        break;
      case 3:
        if (expectFail) {
          simpleMessage += yellow("failed (expected)");
        } else {
          simpleMessage += red("failed (incomplete)");
        }
        break;
    }

    writeAllSync(Deno.stdout, new TextEncoder().encode(simpleMessage + "\n"));
  };
}

function discoverTestsToRun(
  filter: TestFilter,
  expectation: Expectation | string[] | boolean = getExpectation(),
): TestToRun[] {
  const manifestFolder = getManifest().items.testharness;

  const testsToRun: TestToRun[] = [];

  function walk(
    parentFolder: ManifestFolder,
    parentExpectation: Expectation | string[] | boolean,
    prefix: string,
  ) {
    for (const [key, entry] of Object.entries(parentFolder)) {
      if (Array.isArray(entry)) {
        for (
          const [path, options] of entry.slice(
            1,
          ) as ManifestTestVariation[]
        ) {
          // Test keys ending with ".html" include their own html boilerplate.
          // Test keys ending with ".js" will have the necessary boilerplate generated and
          // the manifest path will contain the full path to the generated html test file.
          // See: https://web-platform-tests.org/writing-tests/testharness.html
          if (!key.endsWith(".html") && !key.endsWith(".js")) continue;

          const testHtmlPath = path ?? `${prefix}/${key}`;
          const url = new URL(testHtmlPath, "http://web-platform.test:8000");
          if (!url.pathname.endsWith(".html")) {
            continue;
          }
          // These tests require an HTTP2 compatible server.
          if (url.pathname.includes(".h2.")) {
            continue;
          }
          // Streaming fetch requests need a server that supports chunked
          // encoding, which the WPT test server does not. Unfortunately this
          // also disables some useful fetch tests.
          if (url.pathname.includes("request-upload")) {
            continue;
          }
          const finalPath = url.pathname + url.search;

          const split = finalPath.split("/");
          const finalKey = split[split.length - 1];

          const expectation = Array.isArray(parentExpectation) ||
              typeof parentExpectation == "boolean"
            ? parentExpectation
            : parentExpectation[finalKey];

          if (expectation === undefined) continue;

          if (typeof expectation === "object") {
            if (typeof expectation.ignore !== "undefined") {
              assert(
                typeof expectation.ignore === "boolean",
                "test entry's `ignore` key must be a boolean",
              );
              if (expectation.ignore === true && !noIgnore) continue;
            }
          }

          if (!noIgnore) {
            assert(
              Array.isArray(expectation) || typeof expectation == "boolean",
              "test entry must not have a folder expectation",
            );
          }

          if (!filter.matches(finalPath)) continue;

          testsToRun.push({
            path: finalPath,
            url,
            options,
            expectation,
          });
        }
      } else {
        const expectation = Array.isArray(parentExpectation) ||
            typeof parentExpectation == "boolean"
          ? parentExpectation
          : parentExpectation[key];

        if (expectation === undefined) continue;

        walk(entry, expectation, `${prefix}/${key}`);
      }
    }
  }
  walk(manifestFolder, expectation, "");

  return testsToRun;
}

function partitionTests(tests: TestToRun[]): TestToRun[][] {
  const testsByKey: { [key: string]: TestToRun[] } = {};
  for (const test of tests) {
    // Run all WebCryptoAPI tests in parallel
    if (test.path.includes("/WebCryptoAPI")) {
      testsByKey[test.path] = [test];
      continue;
    }
    // Paths looks like: /fetch/corb/img-html-correctly-labeled.sub-ref.html
    const key = test.path.split("/")[1];
    if (!(key in testsByKey)) {
      testsByKey[key] = [];
    }
    testsByKey[key].push(test);
  }
  return Object.values(testsByKey);
}

function formatDuration(duration: number): string {
  if (duration >= 5000) {
    return red(`${duration}ms`);
  } else if (duration >= 1000) {
    return yellow(`${duration}ms`);
  } else {
    return `${duration}ms`;
  }
}
