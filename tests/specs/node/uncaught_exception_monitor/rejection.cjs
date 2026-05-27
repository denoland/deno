"use strict";

// Verify that a real unhandled promise rejection (not a top-level CJS throw)
// still fires 'uncaughtExceptionMonitor' with origin 'unhandledRejection'.

const theErr = new Error("rejected");

process.on("uncaughtExceptionMonitor", (err, origin) => {
  console.log(
    "monitor",
    origin,
    err === theErr ? "same-error" : "different-error",
  );
});

process.on("uncaughtException", (err, origin) => {
  console.log(
    "uncaught",
    origin,
    err === theErr ? "same-error" : "different-error",
  );
  process.exit(0);
});

// Cause an unhandled promise rejection from an async context (not a
// top-level sync throw). With no 'unhandledRejection' listener, this falls
// back through the polyfill to 'uncaughtException' with origin
// 'unhandledRejection'.
setImmediate(() => {
  Promise.reject(theErr);
});
