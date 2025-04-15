// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  isIterable,
  isNodeStream,
  isWebStream,
} from "ext:deno_node/internal/streams/utils.js";
import { pipelineImpl as pl } from "ext:deno_node/internal/streams/pipeline.js";
import { finished } from "ext:deno_node/internal/streams/end-of-stream.js";
import * as _mod2 from "node:stream";
"use strict";

const {
  ArrayPrototypePop,
  Promise,
} = primordials;

_mod2;

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
