// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import imported1 from "ext:deno_node/internal/errors.ts";
import {
  isNodeStream,
  isWebStream,
  kControllerErrorFunction,
} from "ext:deno_node/internal/streams/utils.js";
import eos from "ext:deno_node/internal/streams/end-of-stream.js";
import * as _mod2 from "ext:deno_node/internal/events/abort_listener.mjs";

const {
  AbortError,
  codes: {
    ERR_INVALID_ARG_TYPE,
  },
} = imported1;

"use strict";

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

export { addAbortSignal };

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

export { addAbortSignalNoValidate };

export default {
  addAbortSignal,
  addAbortSignalNoValidate,
};
