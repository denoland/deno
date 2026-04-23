// Test that fork() works correctly when execArgv contains Deno-style args
// This tests the fix for double-translation when vitest or similar tools
// pass already-translated args like ["run", "-A", "--conditions=...", ...]
const { fork } = require("node:child_process");
const path = require("node:path");

const childPath = path.join(__dirname, "child.js");

// Test 1: Fork with Deno-style execArgv (run -A --conditions=...)
// This simulates what vitest does when spawning workers
console.log("Test 1: Fork with Deno-style execArgv");
const child1 = fork(childPath, [], {
  execArgv: ["run", "-A", "--conditions=custom"],
});
child1.on("message", (msg) => {
  console.log("With run -A --conditions=custom:", msg.result);

  // Test 2: Fork with multiple Deno-style conditions
  console.log("\nTest 2: Fork with multiple Deno-style conditions");
  const child2 = fork(childPath, [], {
    execArgv: ["run", "-A", "--conditions=dev", "--conditions=browser"],
  });
  child2.on("message", (msg) => {
    console.log("With multiple conditions:", msg.result);

    // Test 3: Fork with normal Node-style args (should still work)
    console.log("\nTest 3: Fork with Node-style execArgv");
    const child3 = fork(childPath, [], {
      execArgv: ["--conditions=node-style"],
    });
    child3.on("message", (msg) => {
      console.log("With Node-style args:", msg.result);
    });
  });
});
