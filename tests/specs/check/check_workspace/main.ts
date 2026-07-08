// We should get diagnostics from this import under this check scope.
import "./member/mod.ts";

// Only defined for window.
localStorage;

// Defined for worker; in window it resolves via the @types/node marker global.
onmessage;

// Only defined for worker.
postMessage;
