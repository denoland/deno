import process from "node:process";

function sigintHandler() {
  console.log("SIGINT caught");
}

// Test 1: once + removeListener should fully remove the listener
process.once("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners before removeListener:",
  process.listenerCount("SIGINT"),
);
process.removeListener("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners after removeListener:",
  process.listenerCount("SIGINT"),
);

// Test 2: once + off should also work
process.once("SIGINT", sigintHandler);
console.log("SIGINT listeners before off:", process.listenerCount("SIGINT"));
process.off("SIGINT", sigintHandler);
console.log("SIGINT listeners after off:", process.listenerCount("SIGINT"));

// Test 3: on + removeListener should still work (no once-wrapping)
process.on("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners before on removeListener:",
  process.listenerCount("SIGINT"),
);
process.removeListener("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners after on removeListener:",
  process.listenerCount("SIGINT"),
);

// Test 4: prependOnceListener + removeListener
process.prependOnceListener("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners before prependOnce removeListener:",
  process.listenerCount("SIGINT"),
);
process.removeListener("SIGINT", sigintHandler);
console.log(
  "SIGINT listeners after prependOnce removeListener:",
  process.listenerCount("SIGINT"),
);
