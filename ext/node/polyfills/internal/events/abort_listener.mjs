// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core, primordials } = globalThis.__bootstrap;
  const { queueMicrotask, SymbolDispose } = primordials;
  const { validateAbortSignal, validateFunction } = core.loadExtScript(
    "ext:deno_node/internal/validators.mjs",
  );
  const { codes } = core.loadExtScript("ext:deno_node/internal/errors.ts");
  const { ERR_INVALID_ARG_TYPE } = codes;

  /**
   * @param {AbortSignal} signal
   * @param {EventListener} listener
   * @returns {Disposable}
   */
  function addAbortListener(signal, listener) {
    if (signal === undefined) {
      throw new ERR_INVALID_ARG_TYPE("signal", "AbortSignal", signal);
    }
    validateAbortSignal(signal, "signal");
    validateFunction(listener, "listener");

    let removeEventListener;
    if (signal.aborted) {
      queueMicrotask(() => listener({ target: signal }));
    } else {
      signal.addEventListener("abort", listener, {
        __proto__: null,
        once: true,
      });
      removeEventListener = () => {
        signal.removeEventListener("abort", listener);
      };
    }
    return {
      __proto__: null,
      [SymbolDispose]() {
        removeEventListener?.();
      },
    };
  }

  const _defaultExport = { addAbortListener };

  return {
    addAbortListener,
    default: _defaultExport,
  };
})()
