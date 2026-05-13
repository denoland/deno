"use strict";

// Top-level primitive throw (no Error object). Without the primitive-slot
// dedupe in process.ts, the 'uncaughtExceptionMonitor' listener would fire
// twice: once synchronously from Module._load via process._fatalException,
// and a second time from the unhandled-rejection fallback after the
// primitive gets wrapped in an ERR_UNHANDLED_REJECTION Error.

let monitorCalls = 0;
let uncaughtCalls = 0;

process.on("uncaughtExceptionMonitor", (err, origin) => {
  monitorCalls++;
  console.log("monitor", origin, "err=", err);
});

process.on("uncaughtException", (err, origin) => {
  uncaughtCalls++;
  console.log("uncaught", origin, "err=", err);
  console.log("done monitor=" + monitorCalls + " uncaught=" + uncaughtCalls);
  process.exit(0);
});

throw "boom";
