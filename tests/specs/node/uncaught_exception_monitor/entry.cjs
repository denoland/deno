"use strict";

// Verify that a synchronous top-level throw in a CommonJS entry module fires
// 'uncaughtExceptionMonitor' (and 'uncaughtException') with origin
// 'uncaughtException', matching Node.js semantics. Without the
// Module._load entry-catch in 01_require.js, the throw would surface as a
// module-evaluation rejection and reach the listener with origin
// 'unhandledRejection'.

const theErr = new Error("boom");

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

  process.nextTick(() => {
    process.setUncaughtExceptionCaptureCallback((capturedErr) => {
      console.log(
        "capture",
        capturedErr === theErr ? "same-error" : "different-error",
      );
    });
    throw theErr;
  });
});

throw theErr;
