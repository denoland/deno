// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  SafeMap,
  ArrayPrototypeForEach,
  SafeRegExp,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
} = primordials;

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/node_options-inl.h
// - https://github.com/nodejs/node/blob/master/src/node_options.cc
// - https://github.com/nodejs/node/blob/master/src/node_options.h

/** Gets the all options for Node.js
 * This function is expensive to execute. `getOptionValue` in `internal/options.ts`
 * should be used instead to get a specific option. */
export function getOptions() {
  const options = new SafeMap([
    ["--warnings", { value: true }],
    ["--pending-deprecation", { value: false }],
  ]);

  const nodeOptions = Deno.env.get("NODE_OPTIONS");
  const args = nodeOptions
    ? StringPrototypeSplit(nodeOptions, new SafeRegExp("\\s"))
    : [];
  ArrayPrototypeForEach(args, (arg) => {
    switch (arg) {
      case "--no-warnings":
        options.set("--warnings", { value: false });
        break;
      case "--pending-deprecation":
        options.set("--pending-deprecation", { value: true });
        break;
      default:
        if (StringPrototypeStartsWith(arg, "--dns-result-order=")) {
          const value = StringPrototypeSlice(
            arg,
            "--dns-result-order=".length,
          );
          options.set("--dns-result-order", { value });
        }
        break;
    }
  });
  return { options };
}
