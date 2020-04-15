// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/diagnostics.ts", [], function (exports_13, context_13) {
  "use strict";
  let DiagnosticCategory;
  const __moduleName = context_13 && context_13.id;
  return {
    setters: [],
    execute: function () {
      // Diagnostic provides an abstraction for advice/errors received from a
      // compiler, which is strongly influenced by the format of TypeScript
      // diagnostics.
      (function (DiagnosticCategory) {
        DiagnosticCategory[(DiagnosticCategory["Log"] = 0)] = "Log";
        DiagnosticCategory[(DiagnosticCategory["Debug"] = 1)] = "Debug";
        DiagnosticCategory[(DiagnosticCategory["Info"] = 2)] = "Info";
        DiagnosticCategory[(DiagnosticCategory["Error"] = 3)] = "Error";
        DiagnosticCategory[(DiagnosticCategory["Warning"] = 4)] = "Warning";
        DiagnosticCategory[(DiagnosticCategory["Suggestion"] = 5)] =
          "Suggestion";
      })(DiagnosticCategory || (DiagnosticCategory = {}));
      exports_13("DiagnosticCategory", DiagnosticCategory);
    },
  };
});
