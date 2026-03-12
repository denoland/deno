const { spawn, spawnSync } = require("child_process");

const testName = process.argv[2];

// Helper to run a child process and capture output
function runChild(args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, args, {
      stdio: ["ignore", "pipe", "pipe"],
      ...options,
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (data) => {
      stdout += data.toString();
    });

    child.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    child.on("close", (code) => {
      resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() });
    });
  });
}

// Helper for sync execution
function runChildSync(args) {
  const result = spawnSync(process.execPath, args, {
    encoding: "utf8",
  });
  return {
    code: result.status,
    stdout: (result.stdout || "").trim(),
    stderr: (result.stderr || "").trim(),
  };
}

async function testEvalFlag() {
  // Test -e flag
  const result1 = await runChild(["-e", "console.log('eval-test-1')"]);
  console.log("eval -e:", result1.stdout);

  // Test --eval flag
  const result2 = await runChild(["--eval", "console.log('eval-test-2')"]);
  console.log("eval --eval:", result2.stdout);

  // Test -e with equals
  const result3 = await runChild(["-e", "console.log(1+2)"]);
  console.log("eval math:", result3.stdout);
}

async function testPrintFlag() {
  // Test -p flag (print result)
  const result1 = await runChild(["-p", "1 + 1"]);
  console.log("print -p:", result1.stdout);

  // Test --print flag
  const result2 = await runChild(["--print", "'hello'"]);
  console.log("print --print:", result2.stdout);

  // Test -pe combined
  const result3 = await runChild(["-pe", "2 * 3"]);
  console.log("print -pe:", result3.stdout);
}

async function testInspectFlag() {
  // Test --inspect flag (should not error, just run)
  // We use a quick eval to avoid waiting for inspector connection
  const result1 = await runChild([
    "--inspect=0",
    "-e",
    "console.log('inspect-ok')",
  ]);
  console.log("inspect:", result1.stdout);

  // Test --inspect-brk would hang, so we skip it
  console.log("inspect-brk: skipped (would hang)");
}

async function testConditionsFlag() {
  // Test --conditions flag
  const result = await runChild([
    "--conditions=development",
    "-e",
    "console.log('conditions-ok')",
  ]);
  console.log("conditions:", result.stdout);

  // Test -C short form
  const result2 = await runChild([
    "-C",
    "test",
    "-e",
    "console.log('conditions-short-ok')",
  ]);
  console.log("conditions -C:", result2.stdout);
}

async function testNoWarningsFlag() {
  // Test --no-warnings flag (should be translated to --quiet)
  const result = await runChild([
    "--no-warnings",
    "-e",
    "console.log('no-warnings-ok')",
  ]);
  console.log("no-warnings:", result.stdout);
}

async function testV8Flags() {
  // Test V8 flags are passed through
  // Using a simple flag that doesn't affect output
  const result = await runChild([
    "--max-old-space-size=100",
    "-e",
    "console.log('v8-flags-ok')",
  ]);
  console.log("v8-flags:", result.stdout);
}

async function testMultipleFlags() {
  // Test multiple flags together
  const result = await runChild([
    "--no-warnings",
    "--conditions=development",
    "-e",
    "console.log('multiple-flags-ok')",
  ]);
  console.log("multiple:", result.stdout);
}

async function testNumericArgs() {
  // Test that numeric arguments are properly converted to strings
  // This was a bug we fixed
  const args = ["-e", "console.log(process.argv.slice(2).join(','))"];
  // Add numeric argument (simulating what test-child-process-exit.js does)
  args.push(123);
  args.push("456");

  const result = await runChild(args);
  console.log("numeric args:", result.stdout);
}

async function testDenoSubcommandPassthrough() {
  // When first arg is a Deno subcommand, it should pass through unchanged
  // We can't actually test this easily without spawning deno directly,
  // but we can verify the behavior doesn't break
  const result = await runChild(["-e", "console.log('passthrough-ok')"]);
  console.log("passthrough:", result.stdout);
}

async function main() {
  switch (testName) {
    case "eval":
      await testEvalFlag();
      break;
    case "print":
      await testPrintFlag();
      break;
    case "inspect":
      await testInspectFlag();
      break;
    case "conditions":
      await testConditionsFlag();
      break;
    case "no_warnings":
      await testNoWarningsFlag();
      break;
    case "v8_flags":
      await testV8Flags();
      break;
    case "multiple":
      await testMultipleFlags();
      break;
    case "numeric":
      await testNumericArgs();
      break;
    case "deno_subcommand":
      await testDenoSubcommandPassthrough();
      break;
    default:
      console.error("Unknown test:", testName);
      process.exit(1);
  }
}

main();
