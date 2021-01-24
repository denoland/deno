const noColor = globalThis.Deno?.noColor ?? true;
const enabled = !noColor;

function code(open, close) {
  return {
    open: `\x1b[${open.join(";")}m`,
    close: `\x1b[${close}m`,
    regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
  };
}

function run(str, code) {
  return enabled
    ? `${code.open}${str.replace(code.regexp, code.open)}${code.close}`
    : str;
}

function red(str) {
  return run(str, code([31], 39));
}

export function green(str) {
  return run(str, code([32], 39));
}

export function yellow(str) {
  return run(str, code([33], 39));
}

const testResults = [];
const testsExpectFail = JSON.parse(Deno.args[0]);
function shouldExpectFail(name) {
  if (testsExpectFail.includes(name)) return true;
  for (const expectFail of testsExpectFail) {
    if (name.startsWith(expectFail)) return true;
  }
  return false;
}

window.add_result_callback(({ message, name, stack, status }) => {
  const expectFail = shouldExpectFail(name);
  testResults.push({
    name,
    passed: status === 0,
    expectFail,
    message,
    stack,
  });
  let simpleMessage = `test ${name} ... `;
  switch (status) {
    case 0:
      if (expectFail) {
        simpleMessage += red("ok (expected fail)");
      } else {
        simpleMessage += green("ok");
        if (Deno.args[1] == "--quiet") {
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
});

window.add_completion_callback((tests, harnessStatus) => {
  const failed = testResults.filter((t) => !t.expectFail && !t.passed);
  const expectedFailedButPassed = testResults.filter((t) =>
    t.expectFail && t.passed
  );
  const expectedFailedButPassedCount = expectedFailedButPassed.length;
  const failedCount = failed.length + expectedFailedButPassedCount;
  const expectedFailedAndFailedCount = testResults.filter((t) =>
    t.expectFail && !t.passed
  ).length;
  const totalCount = testResults.length;
  const passedCount = totalCount - failedCount - expectedFailedAndFailedCount;

  if (failed.length > 0) {
    console.log(`\nfailures:`);
  }
  for (const result of failed) {
    console.log(
      `\n${result.name}\n${result.message}\n${result.stack}`,
    );
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
    `\ntest result: ${
      failedCount > 0 ? red("failed") : green("ok")
    }. ${passedCount} passed; ${failedCount} failed; ${expectedFailedAndFailedCount} expected failure; total ${totalCount}\n`,
  );

  Deno.exit(failedCount > 0 ? 1 : 0);
});
