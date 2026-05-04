// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_random_int } from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
} from "ext:deno_node/internal/errors.ts";
const { validateFunction } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const {
  MathCeil,
  MathFloor,
  NumberIsSafeInteger,
} = primordials;

// Largest integer that can be expressed in 6 bytes, mirrors Node's RAND_MAX
// in lib/internal/crypto/random.js.
const RAND_MAX = 0xFFFF_FFFF_FFFF;

export default function randomInt(max: number): number;
export default function randomInt(min: number, max: number): number;
export default function randomInt(
  max: number,
  cb: (err: Error | null, n?: number) => void,
): void;
export default function randomInt(
  min: number,
  max: number,
  cb: (err: Error | null, n?: number) => void,
): void;

// Generates an integer in [min, max) range where min is inclusive and max is
// exclusive. Matches Node's lib/internal/crypto/random.js randomInt().
export default function randomInt(
  min: number,
  max?: ((err: Error | null, n?: number) => void) | number,
  callback?: (err: Error | null, n?: number) => void,
): number | void {
  // Detect optional min syntax
  // randomInt(max)
  // randomInt(max, callback)
  const minNotSpecified = typeof max === "undefined" ||
    typeof max === "function";

  if (minNotSpecified) {
    callback = max as (err: Error | null, n?: number) => void;
    max = min;
    min = 0;
  }

  const isSync = typeof callback === "undefined";
  if (!isSync) {
    validateFunction(callback, "callback");
  }
  if (!NumberIsSafeInteger(min)) {
    throw new ERR_INVALID_ARG_TYPE("min", "a safe integer", min);
  }
  if (!NumberIsSafeInteger(max)) {
    throw new ERR_INVALID_ARG_TYPE("max", "a safe integer", max);
  }
  if ((max as number) <= min) {
    throw new ERR_OUT_OF_RANGE(
      "max",
      `greater than the value of "min" (${min})`,
      max,
    );
  }

  const range = (max as number) - min;
  if (!(range <= RAND_MAX)) {
    throw new ERR_OUT_OF_RANGE(
      `max${minNotSpecified ? "" : " - min"}`,
      `<= ${RAND_MAX}`,
      range,
    );
  }

  min = MathCeil(min);
  const result = op_node_random_int(min, MathFloor(max as number));

  if (!isSync) {
    callback!(null, result);
    return;
  }

  return result;
}
