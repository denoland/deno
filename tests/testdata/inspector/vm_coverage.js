// Copyright 2018-2025 the Deno authors. MIT license.
// Small script exercising the Node "vm" module and inspector coverage.
// This script creates an inspector session, enables precise coverage, then
// compiles a new script via `vm.runInThisContext` with an absolute file name.
// After a tick, it requests coverage and prints out any file URLs reported.

import inspector from "node:inspector";
import vm from "node:vm";

const session = new inspector.Session();
session.connect();
session.post("Profiler.enable");
// Start precise coverage then compile and run a small script under a fixed filename.
session.post(
  "Profiler.startPreciseCoverage",
  { callCount: true, detailed: true },
  () => {
    vm.runInThisContext("function x(){}; x();", { filename: "/tmp/foo.js" });
    setTimeout(() => {
      session.post("Profiler.takePreciseCoverage", (err, res) => {
        if (err) throw err;
        // Only show file:// URLs.
        const files = res.result.filter((e) => e.url.startsWith("file://"));
        console.log(JSON.stringify(files));
      });
    }, 0);
  },
);
