#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-net --allow-env --allow-run
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// This script is used to run WPT tests for Deno.

import {
  runSingleTest,
  runWithTestUtil,
  TestCaseResult,
  TestResult,
} from "./wpt/runner.ts";
import {
  assert,
  autoConfig,
  cargoBuild,
  checkPy3Available,
  Expectation,
  getExpectation,
  getExpectFailForCase,
  getManifest,
  json,
  ManifestFolder,
  ManifestTestOptions,
  ManifestTestVariation,
  quiet,
  rest,
  runPy,
  updateManifest,
} from "./wpt/utils.ts";
import {
  blue,
  bold,
  green,
  red,
  yellow,
} from "https://deno.land/std@0.84.0/fmt/colors.ts";
import { saveExpectation } from "./wpt/utils.ts";

const command = Deno.args[0];

switch (command) {
  case "setup":
    await checkPy3Available();
    await updateManifest();
    await setup();
    break;

  case "run":
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

More details at https://deno.land/manual@master/contributing/web_platform_tests

    `);
    break;
}

async function setup() {
  // TODO(lucacsonato): use this when 1.7.1 is released.
  // const records = await Deno.resolveDns("web-platform.test", "A");
  // const etcHostsConfigured = records[0] == "127.0.0.1";
  const hostsFile = await Deno.readTextFile("/etc/hosts");
  const etcHostsConfigured = hostsFile.includes("web-platform.test");

  if (etcHostsConfigured) {
    console.log("/etc/hosts is already configured.");
  } else {
    const autoConfigure = autoConfig ||
      confirm(
        "The WPT require certain entries to be present in your /etc/hosts file. Should these be configured automatically?",
      );
    if (autoConfigure) {
      const proc = runPy(["wpt", "make-hosts-file"], { stdout: "piped" });
      const status = await proc.status();
      assert(status.success, "wpt make-hosts-file should not fail");
      const entries = new TextDecoder().decode(await proc.output());
      const hostsPath = Deno.build.os == "windows"
        ? `${Deno.env.get("SystemRoot")}\\System32\\drivers\\etc\\hosts`
        : "/etc/hosts";
      const file = await Deno.open(hostsPath, { append: true }).catch((err) => {
        if (err instanceof Deno.errors.PermissionDenied) {
          throw new Error(
            `Failed to open ${hostsPath} (permission error). Please run this command again with sudo, or configure the entries manually.`,
          );
        } else {
          throw err;
        }
      });
      await Deno.writeAll(
        file,
        new TextEncoder().encode(
          "\n\n# Configured for Web Platform Tests (Deno)\n" + entries,
        ),
      );
      console.log("Updated /etc/hosts");
    } else {
      console.log("Please configure the /etc/hosts entries manually.");
      if (Deno.build.os == "windows") {
        console.log("To do this run the following command in PowerShell:");
        console.log("");
        console.log("    cd test_util/wpt/");
        console.log(
          "    python.exe wpt make-hosts-file | Out-File $env:SystemRoot\\System32\\drivers\\etc\\hosts -Encoding ascii -Append",
        );
        console.log("");
      } else {
        console.log("To do this run the following command in your shell:");
        console.log("");
        console.log("    cd test_util/wpt/");
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
  sourcePath: string;
  path: string;
  url: URL;
  options: ManifestTestOptions;
  expectation: boolean | string[];
}

async function run() {
  assert(Array.isArray(rest), "filter must be array");
  const tests = discoverTestsToRun(rest.length == 0 ? undefined : rest);
  console.log(`Going to run ${tests.length} test files.`);

  const results = await runWithTestUtil(false, async () => {
    const results = [];

    for (const test of tests) {
      console.log(`${blue("-".repeat(40))}\n${bold(test.path)}\n`);
      const result = await runSingleTest(
        test.url,
        test.options,
        json ? () => {} : createReportTestCase(test.expectation),
      );
      results.push({ test, result });
      reportVariation(result, test.expectation);
    }

    return results;
  });

  if (json) {
    await Deno.writeTextFile(json, JSON.stringify(results));
  }
  const code = reportFinal(results);
  Deno.exit(code);
}

async function update() {
  assert(Array.isArray(rest), "filter must be array");
  const tests = discoverTestsToRun(rest.length == 0 ? undefined : rest, true);
  console.log(`Going to run ${tests.length} test files.`);

  const results = await runWithTestUtil(false, async () => {
    const results = [];

    for (const test of tests) {
      console.log(`${blue("-".repeat(40))}\n${bold(test.path)}\n`);
      const result = await runSingleTest(
        test.url,
        test.options,
        json ? () => {} : createReportTestCase(test.expectation),
      );
      results.push({ test, result });
      reportVariation(result, test.expectation);
    }

    return results;
  });

  if (json) {
    await Deno.writeTextFile(json, JSON.stringify(results));
  }

  const resultTests: Record<
    string,
    { passed: string[]; failed: string[]; status: number }
  > = {};
  for (const { test, result } of results) {
    if (!resultTests[test.sourcePath]) {
      resultTests[test.sourcePath] = {
        passed: [],
        failed: [],
        status: result.status,
      };
    }
    for (const case_ of result.cases) {
      if (case_.passed) {
        resultTests[test.sourcePath].passed.push(case_.name);
      } else {
        resultTests[test.sourcePath].failed.push(case_.name);
      }
    }
  }

  const currentExpectation = getExpectation();

  for (const path in resultTests) {
    const { passed, failed, status } = resultTests[path];
    let finalExpectation: boolean | string[];
    if (failed.length == 0 && status == 0) {
      finalExpectation = true;
    } else if (failed.length > 0 && passed.length > 0 && status == 0) {
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

  saveExpectation(currentExpectation);

  reportFinal(results);

  console.log(blue("Updated expectation.json to match reality."));

  Deno.exit(0);
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
      !currentExpectation[segment] ||
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
    currentExpectation[segment] = finalExpectation;
  }
}

function reportFinal(
  results: { test: TestToRun; result: TestResult }[],
): number {
  const finalTotalCount = results.length;
  let finalFailedCount = 0;
  const finalFailed: [string, TestCaseResult][] = [];
  let finalExpectedFailedAndFailedCount = 0;
  const finalExpectedFailedButPassedTests: [string, TestCaseResult][] = [];
  const finalExpectedFailedButPassedFiles: string[] = [];
  for (const { test, result } of results) {
    const { failed, failedCount, expectedFailedButPassed } = analyzeTestResult(
      result,
      test.expectation,
    );
    if (result.status !== 0) {
      if (test.expectation === false) {
        finalExpectedFailedAndFailedCount += 1;
      } else {
        finalFailedCount += 1;
        finalExpectedFailedButPassedFiles.push(test.path);
      }
    } else if (failedCount > 0) {
      finalFailedCount += 1;
      for (const case_ of failed) {
        finalFailed.push([test.path, case_]);
      }
      for (const case_ of expectedFailedButPassed) {
        finalExpectedFailedButPassedTests.push([test.path, case_]);
      }
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

  console.log(
    `\nfinal result: ${
      finalFailedCount > 0 ? red("failed") : green("ok")
    }. ${finalPassedCount} passed; ${finalFailedCount} failed; ${finalExpectedFailedAndFailedCount} expected failure; total ${finalTotalCount}\n`,
  );

  return finalFailedCount > 0 ? 1 : 0;
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
  if (result.status !== 0) {
    console.log(`test stderr:`);
    Deno.writeAllSync(Deno.stdout, new TextEncoder().encode(result.stderr));

    const expectFail = expectation === false;
    console.log(
      `\nfile result: ${
        expectFail ? yellow("failed (expected)") : red("failed")
      }. runner failed during test\n`,
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

  if (failed.length > 0) {
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
  console.log(
    `\nfile result: ${
      failedCount > 0 ? red("failed") : green("ok")
    }. ${passedCount} passed; ${failedCount} failed; ${expectedFailedAndFailedCount} expected failure; total ${totalCount}\n`,
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

    console.log(simpleMessage);
  };
}

function discoverTestsToRun(
  filter?: string[],
  expectation: Expectation | string[] | boolean = getExpectation(),
): TestToRun[] {
  const manifestFolder = getManifest().items.testharness;

  const testsToRun: TestToRun[] = [];

  function walk(
    parentFolder: ManifestFolder,
    parentExpectation: Expectation | string[] | boolean,
    prefix: string,
  ) {
    for (const key in parentFolder) {
      const sourcePath = `${prefix}/${key}`;
      const entry = parentFolder[key];
      const expectation = Array.isArray(parentExpectation) ||
          typeof parentExpectation == "boolean"
        ? parentExpectation
        : parentExpectation[key];

      if (expectation === undefined) continue;

      if (Array.isArray(entry)) {
        assert(
          Array.isArray(expectation) || typeof expectation == "boolean",
          "test entry must not have a folder expectation",
        );
        if (
          filter &&
          !filter.find((filter) => sourcePath.substring(1).startsWith(filter))
        ) {
          continue;
        }

        for (
          const [path, options] of entry.slice(
            1,
          ) as ManifestTestVariation[]
        ) {
          if (!path) continue;
          const url = new URL(path, "http://web-platform.test:8000");
          if (!url.pathname.endsWith(".any.html")) continue;
          testsToRun.push({
            sourcePath,
            path: url.pathname + url.search,
            url,
            options,
            expectation,
          });
        }
      } else {
        walk(entry, expectation, sourcePath);
      }
    }
  }
  walk(manifestFolder, expectation, "");

  return testsToRun;
}
