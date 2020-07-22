// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Diagnostic provides an abstraction for advice/errors received from a
// compiler, which is strongly influenced by the format of TypeScript
// diagnostics.

((window) => {
  const DiagnosticCategory = {
    0: "Log",
    1: "Debug",
    2: "Info",
    3: "Error",
    4: "Warning",
    5: "Suggestion",

    Log: 0,
    Debug: 1,
    Info: 2,
    Error: 3,
    Warning: 4,
    Suggestion: 5,
  };

  window.__bootstrap.diagnostics = {
    DiagnosticCategory,
  };
})(this);
