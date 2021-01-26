#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-net --allow-env --allow-run
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// This script is used to run WPT tests for Deno.

import {
  runWithTestUtil,
  runSingleTest,
  TestCaseResult,
  TestResult,
} from "./wpt/runner.ts";
import {
  getExpectations as getExpectation,
  getManifest,
  generateTestExpectations,
  ManifestFolder,
  Expectation,
  assert,
  ManifestTestVariation,
  quiet,
  json,
  updateManifest,
  checkPy3Available,
  rest,
  runPy,
} from "./wpt/utils.ts";
import {
  red,
  green,
  yellow,
  blue,
  bold,
} from "https://deno.land/std@0.84.0/fmt/colors.ts";

const command = Deno.args[0];

switch (command) {
  case "setup":
    await checkPy3Available();
    await updateManifest();
    await setup();

    break;
  case "run":
    await run();
    break;

  case "gen-expectations":
    genExpectations();
    break;

  default:
    console.log(`Possible commands:

    run
      Run all tests like specified in expectations.json.

    gen-expectations
      Re-generate the ./tools/wpt/expectations.json file from the test manifest.    
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
    const autoConfigure = confirm(
      "The WPT require certain entries to be present in your /etc/hosts file. Should these be configured automatically?"
    );
    if (autoConfigure) {
      const proc = runPy(["wpt", "make-hosts-file"], { stdout: "piped" });
      const status = await proc.status();
      assert(status.success, "wpt make-hosts-file should not fail");
      const entries = new TextDecoder().decode(await proc.output());
      const hostsPath =
        Deno.build.os == "windows"
          ? `${Deno.env.get("SystemRoot")}\\System32\\drivers\\etc\\hosts`
          : "/etc/hosts";
      const file = await Deno.open(hostsPath, { append: true }).catch((err) => {
        if (err instanceof Deno.errors.PermissionDenied) {
          throw new Error(
            `Failed to open ${hostsPath} (permission error). Please run this command again with sudo, or configure the entries manually.`
          );
        } else {
          throw err;
        }
      });
      await Deno.writeAll(
        file,
        new TextEncoder().encode(
          "\n\n# Configured for Web Platform Tests (Deno)\n" + entries
        )
      );
      console.log("Updated /etc/hosts");
    } else {
      console.log("Please configure the /etc/hosts entries manually.");
      if (Deno.build.os == "windows") {
        console.log("To do this run the following command in PowerShell:");
        console.log("");
        console.log("    cd test_util/wpt/");
        console.log(
          "    python.exe wpt make-hosts-file | Out-File $env:SystemRoot\\System32\\drivers\\etc\\hosts -Encoding ascii -Append"
        );
        console.log("");
      } else {
        console.log("To do this run the following command in your shell:");
        console.log("");
        console.log("    cd test_util/wpt/");
        console.log(
          "    python3 ./wpt make-hosts-file | sudo tee -a /etc/hosts"
        );
        console.log("");
      }
    }
  }

  console.log(green("Setup complete!"));
}

interface TestToRun {
  path: string;
  variations: ManifestTestVariation[];
  expectation: boolean | string[];
}

async function run() {
  assert(Array.isArray(rest), "filter must be array");
  const tests = discoverTestsToRun(rest.length == 0 ? undefined : rest);
  console.log(`Going to run ${tests.length} test files.`);

  const results = await runWithTestUtil(false, async () => {
    const results = [];

    for (const test of tests) {
      for (const variation of test.variations) {
        const [path, options] = variation;
        const url = new URL(path, "http://web-platform.test:8000");
        console.log(`${blue("-".repeat(40))}\n${bold(path)}\n`);
        const result = await runSingleTest(
          path,
          url,
          options,
          test.expectation,
          json ? () => {} : reportTestCase
        );
        results.push(result);
        reportVariation(result);
      }
    }

    return results;
  });

  if (json) {
    await Deno.writeTextFile(json, JSON.stringify(results));
  }
  const code = reportFinal(results);
  Deno.exit(code);
}

function reportFinal(results: TestResult[]): number {
  const finalTotalCount = results.length;
  let finalFailedCount = 0;
  const finalFailed: [string, TestCaseResult][] = [];
  let finalExpectedFailedAndFailedCount = 0;
  const finalExpectedFailedButPassedTests: [string, TestCaseResult][] = [];
  const finalExpectedFailedButPassedFiles: string[] = [];
  for (const result of results) {
    const { failed, failedCount, expectedFailedButPassed } = analzyeTestResult(
      result
    );
    if (result.status !== 0) {
      if (result.expectFail) {
        finalExpectedFailedAndFailedCount += 1;
      } else {
        finalFailedCount += 1;
        finalExpectedFailedButPassedFiles.push(result.path);
      }
    } else if (failedCount > 0) {
      finalFailedCount += 1;
      for (const test of failed) {
        finalFailed.push([result.path, test]);
      }
      for (const test of expectedFailedButPassed) {
        finalExpectedFailedButPassedTests.push([result.path, test]);
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
      `        ${JSON.stringify(`${result[0]} - ${result[1].name}`)}`
    );
  }
  if (finalExpectedFailedButPassedTests.length > 0) {
    console.log(`\nexpected test failures that passed:\n`);
  }
  for (const result of finalExpectedFailedButPassedTests) {
    console.log(
      `        ${JSON.stringify(`${result[0]} - ${result[1].name}`)}`
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
    }. ${finalPassedCount} passed; ${finalFailedCount} failed; ${finalExpectedFailedAndFailedCount} expected failure; total ${finalTotalCount}\n`
  );

  return finalFailedCount > 0 ? 1 : 0;
}

function analzyeTestResult(
  result: TestResult
): {
  failed: TestCaseResult[];
  failedCount: number;
  passedCount: number;
  totalCount: number;
  expectedFailedButPassed: TestCaseResult[];
  expectedFailedButPassedCount: number;
  expectedFailedAndFailedCount: number;
} {
  const failed = result.cases.filter((t) => !t.expectFail && !t.passed);
  const expectedFailedButPassed = result.cases.filter(
    (t) => t.expectFail && t.passed
  );
  const expectedFailedButPassedCount = expectedFailedButPassed.length;
  const failedCount = failed.length + expectedFailedButPassedCount;
  const expectedFailedAndFailedCount = result.cases.filter(
    (t) => t.expectFail && !t.passed
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

function reportVariation(result: TestResult) {
  if (result.status !== 0) {
    console.log(`test stderr:`);
    Deno.writeAllSync(Deno.stdout, new TextEncoder().encode(result.stderr));

    console.log(
      `\nfile result: ${
        result.expectFail ? yellow("failed (expected)") : red("failed")
      }. runner failed during test\n`
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
  } = analzyeTestResult(result);

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
    }. ${passedCount} passed; ${failedCount} failed; ${expectedFailedAndFailedCount} expected failure; total ${totalCount}\n`
  );
}

function reportTestCase({ name, status, expectFail }: TestCaseResult) {
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
}

function discoverTestsToRun(filter?: string[]): TestToRun[] {
  const manifestFolder = getManifest().items.testharness;
  const expectation = getExpectation();

  const testsToRun: TestToRun[] = [];

  function walk(
    parentFolder: ManifestFolder,
    parentExpectation: Expectation | string[] | boolean,
    prefix: string
  ) {
    for (const key in parentFolder) {
      const path = `${prefix}/${key}`;
      const entry = parentFolder[key];
      const expectation =
        Array.isArray(parentExpectation) ||
        typeof parentExpectation == "boolean"
          ? parentExpectation
          : parentExpectation[key];

      if (expectation === undefined) continue;

      if (Array.isArray(entry)) {
        assert(
          Array.isArray(expectation) || typeof expectation == "boolean",
          "test entry must not have a folder expectation"
        );
        if (
          filter &&
          !filter.find(
            (filter) =>
              path.startsWith(filter) || path.substring(1).startsWith(filter)
          )
        ) {
          continue;
        }
        const variations = (entry.slice(1) as ManifestTestVariation[]).filter(
          ([path]) => {
            if (!path) return false;
            return path.endsWith(".any.html");
          }
        );

        if (variations.length == 0) continue;

        testsToRun.push({
          path,
          variations,
          expectation,
        });
      } else {
        walk(entry, expectation, path);
      }
    }
  }
  walk(manifestFolder, expectation, "");

  return testsToRun;
}

function genExpectations() {
  const enabledSuites = [
    "/WebCryptoAPI/",
    "/WebIDL/",
    "/compat/",
    "/compression/",
    "/console/",
    "/dom/",
    "/encoding/",
    "/fetch/",
    "/hr-time/",
    "/streams/",
    "/url/",
    "/user-timing/",
    "/wasm/",
    "/websockets/",
    "/workers/",
  ];
  const expectation = generateTestExpectations(enabledSuites);
  const expectationText = JSON.stringify(expectation, undefined, "  ");
  Deno.writeTextFileSync("./tools/wpt/expectation.json", expectationText);
}
