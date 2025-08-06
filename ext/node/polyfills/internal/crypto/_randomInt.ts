// Copyright 2018-2025 the Deno authors. MIT license.

import { op_node_random_int } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
const {
  Error,
  MathCeil,
  MathFloor,
  MathPow,
  NumberIsSafeInteger,
  RangeError,
} = primordials;

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

export default function randomInt(
  max: number,
  min?: ((err: Error | null, n?: number) => void) | number,
  cb?: (err: Error | null, n?: number) => void,
): number | void {
  if (typeof max === "number" && typeof min === "number") {
    const temp = max;
    max = min;
    min = temp;
  }
  if (min === undefined) min = 0;
  else if (typeof min === "function") {
    cb = min;
    min = 0;
  }

  if (!NumberIsSafeInteger(min)) {
    throw new ERR_INVALID_ARG_TYPE("min", "a safe integer", min);
  }

  if (!NumberIsSafeInteger(max)) {
    throw new ERR_INVALID_ARG_TYPE("max", "a safe integer", max);
  }

  if (max - min > MathPow(2, 48)) {
    throw new RangeError("max - min should be less than 2^48!");
  }

  if (min >= max) {
    throw new Error("Min is bigger than Max!");
  }

  min = MathCeil(min);
  max = MathFloor(max);
  const result = op_node_random_int(min, max);

  if (cb) {
    cb(null, result);
    return;
  }

  return result;
}
