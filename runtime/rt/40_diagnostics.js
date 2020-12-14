// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Diagnostic provides an abstraction for advice/errors received from a
// compiler, which is strongly influenced by the format of TypeScript
// diagnostics.

((window) => {
  const DiagnosticCategory = {
    0: "Warning",
    1: "Error",
    2: "Suggestion",
    3: "Message",

    Warning: 0,
    Error: 1,
    Suggestion: 2,
    Message: 3,
  };

  window.__bootstrap.diagnostics = {
    DiagnosticCategory,
  };
})(this);
