// We should get diagnostics from this import under this check scope.
import "./member/mod.ts";

// Only defined for window.
localStorage;

// Only defined for worker.
onmessage;
