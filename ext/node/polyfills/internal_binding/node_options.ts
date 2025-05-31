// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  SafeMap,
  ArrayPrototypeForEach,
  SafeRegExp,
  StringPrototypeSplit,
} = primordials;

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/node_options-inl.h
// - https://github.com/nodejs/node/blob/master/src/node_options.cc
// - https://github.com/nodejs/node/blob/master/src/node_options.h

export function getOptions() {
  const options = new SafeMap();
  options.set("--warnings", { value: true });

  const nodeOptions = Deno.env.get("NODE_OPTIONS");
  const args = nodeOptions
    ? StringPrototypeSplit(nodeOptions, new SafeRegExp("\\s"))
    : [];
  ArrayPrototypeForEach(args, (arg) => {
    switch (arg) {
      case "--no-warnings":
        options.set("--warnings", { value: false });
        break;
      // TODO(kt3k): Handle other options
      default:
        break;
    }
  });
  return { options };
}
