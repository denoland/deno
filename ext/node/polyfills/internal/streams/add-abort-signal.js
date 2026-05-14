// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const imported1 = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  isNodeStream,
  isWebStream,
  kControllerErrorFunction,
} = core.loadExtScript("ext:deno_node/internal/streams/utils.js");
const eos =
  core.loadExtScript("ext:deno_node/internal/streams/end-of-stream.js").default;
const _mod2 = core.loadExtScript(
  "ext:deno_node/internal/events/abort_listener.mjs",
);

const {
  AbortError,
  codes: {
    ERR_INVALID_ARG_TYPE,
  },
} = imported1;

const {
  SymbolDispose,
} = primordials;

let addAbortListener;

// This method is inlined here for readable-stream
// It also does not allow for signal to not exist on the stream
// https://github.com/nodejs/node/pull/36061#discussion_r533718029
const validateAbortSignal = (signal, name) => {
  if (
    typeof signal !== "object" ||
    !("aborted" in signal)
  ) {
    throw new ERR_INVALID_ARG_TYPE(name, "AbortSignal", signal);
  }
};

const addAbortSignal = function addAbortSignal(signal, stream) {
  validateAbortSignal(signal, "signal");
  if (!isNodeStream(stream) && !isWebStream(stream)) {
    throw new ERR_INVALID_ARG_TYPE("stream", [
      "ReadableStream",
      "WritableStream",
      "Stream",
    ], stream);
  }
  return addAbortSignalNoValidate(signal, stream);
};

const addAbortSignalNoValidate = function (signal, stream) {
  if (typeof signal !== "object" || !("aborted" in signal)) {
    return stream;
  }
  const onAbort = isNodeStream(stream)
    ? () => {
      stream.destroy(new AbortError(undefined, { cause: signal.reason }));
    }
    : () => {
      stream[kControllerErrorFunction](
        new AbortError(undefined, { cause: signal.reason }),
      );
    };
  if (signal.aborted) {
    onAbort();
  } else {
    addAbortListener ??= _mod2.addAbortListener;
    const disposable = addAbortListener(signal, onAbort);
    eos(stream, disposable[SymbolDispose]);
  }
  return stream;
};

return {
  addAbortSignal,
  addAbortSignalNoValidate,
  default: { addAbortSignal, addAbortSignalNoValidate },
};
})();
