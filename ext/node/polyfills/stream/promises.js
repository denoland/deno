// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  isIterable,
  isNodeStream,
  isWebStream,
} = core.loadExtScript("ext:deno_node/internal/streams/utils.js");
import { pipelineImpl as pl } from "ext:deno_node/internal/streams/pipeline.js";
const { finished } = core.loadExtScript(
  "ext:deno_node/internal/streams/end-of-stream.js",
);
// qjs_v8_compat: avoid the node:stream namespace cycle by deferring
// the import; the binding is unused below other than a touch.
"use strict";

const {
  ArrayPrototypePop,
  Promise,
} = primordials;

function pipeline(...streams) {
  return new Promise((resolve, reject) => {
    let signal;
    let end;
    const lastArg = streams[streams.length - 1];
    if (
      lastArg && typeof lastArg === "object" &&
      !isNodeStream(lastArg) && !isIterable(lastArg) && !isWebStream(lastArg)
    ) {
      const options = ArrayPrototypePop(streams);
      signal = options.signal;
      end = options.end;
    }

    pl(streams, (err, value) => {
      if (err) {
        reject(err);
      } else {
        resolve(value);
      }
    }, { signal, end });
  });
}

const _defaultExport1 = {
  finished,
  pipeline,
};

export default _defaultExport1;
export { finished, pipeline };
