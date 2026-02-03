const { fork } = require("node:child_process");
const path = require("node:path");

const childPath = path.join(__dirname, "child.js");

// Test 1: Fork without custom conditions (should use default)
console.log("Test 1: Fork without conditions");
const child1 = fork(childPath);
child1.on("message", (msg) => {
  console.log("Without conditions:", msg.type);

  // Test 2: Fork with --conditions=custom
  console.log("\nTest 2: Fork with --conditions=custom");
  const child2 = fork(childPath, [], {
    execArgv: ["--conditions=custom"],
  });
  child2.on("message", (msg) => {
    console.log("With --conditions=custom:", msg.type);

    // Test 3: Fork with -C custom (short form, separate arg)
    console.log("\nTest 3: Fork with -C custom");
    const child3 = fork(childPath, [], {
      execArgv: ["-C", "custom"],
    });
    child3.on("message", (msg) => {
      console.log("With -C custom:", msg.type);
    });
  });
});
