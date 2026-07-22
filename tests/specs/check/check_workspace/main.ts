// We should get diagnostics from this import under this check scope.
import "./member/mod.ts";

// Only defined for window.
alert;

// Only defined for worker.
WorkerGlobalScope;
