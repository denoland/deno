// Copyright 2018-2026 the Deno authors. MIT license.

// Tiny IIFE-wrapped helper for `node:repl`'s inline preview path. Lives
// here (rather than in `repl.ts`) because the deno_node ops we need are
// only reachable from scripts loaded via `core.loadExtScript` -- those
// see the snapshot-time `__bootstrap.core.ops` capture. ES-module
// polyfills like `repl.ts` only see the post-`removeImportedOps` view,
// which doesn't include `op_node_repl_inspector_connect`. See the long
// comment around `loadExtScript` / `op_set_captured_bootstrap` in
// `libs/core/01_core.js`.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayPrototypePush,
  JSONParse,
  JSONStringify,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  SafeArrayIterator,
  SafeMap,
} = primordials;
const {
  op_node_repl_inspector_connect,
  op_inspector_dispatch,
  op_inspector_disconnect,
} = core.ops;

/**
 * Create a private V8 inspector session that answers "would this REPL
 * preview expression have observable side effects?" synchronously.
 *
 * The session is dispatched to via raw op calls (not through
 * `node:inspector`'s `Session` class) so we can capture responses and
 * notifications inline rather than waiting on the regular nextTick
 * drain. That matters because:
 *
 *   1. `Runtime.enable` replays `Runtime.executionContextCreated` for
 *      existing contexts synchronously -- we read off the main realm's
 *      contextId in the same tick to pass into `Runtime.evaluate`.
 *   2. `Runtime.evaluate({ throwOnSideEffect: true, ... })` returns
 *      the response synchronously for non-promise expressions, which
 *      keeps the preview path keystroke-fast.
 *
 * The inspector is used purely as a side-effect *oracle* here. The
 * caller still renders the preview value via `vm.Script` so
 * `util.inspect` sees the real JS object (prototype, getters, class
 * info, etc.) rather than the CDP wire shape.
 *
 * Returns `null` if the inspector isn't reachable -- callers fall back
 * to *no* preview rather than to an unprotected `vm.Script` eval.
 */
function createPreviewSession() {
  let session;
  let nextId = 1;
  const responses = new SafeMap();
  const notifications = [];

  try {
    session = op_node_repl_inspector_connect((messageStr) => {
      let parsed;
      try {
        parsed = JSONParse(messageStr);
      } catch {
        return;
      }
      if (parsed.id !== undefined) {
        MapPrototypeSet(responses, parsed.id, parsed);
      } else if (parsed.method) {
        ArrayPrototypePush(notifications, parsed);
      }
    });
  } catch {
    return null;
  }

  function call(method, params) {
    const id = nextId++;
    const message = { id, method };
    if (params) message.params = params;
    try {
      op_inspector_dispatch(session, JSONStringify(message));
    } catch {
      return undefined;
    }
    const res = MapPrototypeGet(responses, id);
    MapPrototypeDelete(responses, id);
    return res;
  }

  const enableRes = call("Runtime.enable");
  if (enableRes === undefined || enableRes.error) {
    try {
      op_inspector_disconnect(session);
    } catch { /* ignore */ }
    return null;
  }

  // Pick up the main realm's contextId from the
  // `executionContextCreated` notification v8 fired during
  // `Runtime.enable`.
  let mainContextId;
  for (const n of new SafeArrayIterator(notifications)) {
    if (n.method !== "Runtime.executionContextCreated") continue;
    const ctx = n.params && n.params.context;
    if (!ctx) continue;
    if (ctx.auxData && ctx.auxData.isDefault) {
      mainContextId = ctx.id;
      break;
    }
    if (mainContextId === undefined) mainContextId = ctx.id;
  }
  notifications.length = 0;

  return {
    isSafe(expression) {
      const res = call("Runtime.evaluate", {
        expression,
        throwOnSideEffect: true,
        // Same timeout Node's `setupPreview` uses for the probe.
        // Long-running expressions (e.g. `while(true){}`) abort here
        // so keystrokes don't stall on the preview path.
        timeout: 333,
        contextId: mainContextId,
        returnByValue: false,
        generatePreview: false,
      });
      if (!res || res.error) return false;
      const result = res.result;
      if (!result) return false;
      // v8 reports `throwOnSideEffect` violations through
      // `exceptionDetails` -- same channel as SyntaxError /
      // ReferenceError / runtime throws. Any of those means
      // "don't preview".
      if (result.exceptionDetails) return false;
      return true;
    },

    close() {
      try {
        call("Runtime.disable");
      } catch { /* ignore */ }
      try {
        op_inspector_disconnect(session);
      } catch { /* ignore */ }
    },
  };
}

return { createPreviewSession };
})();
