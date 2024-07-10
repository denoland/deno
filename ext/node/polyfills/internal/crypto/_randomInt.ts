// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { op_node_random_int } from "ext:core/ops";

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
    [max, min] = [min, max];
  }
  if (min === undefined) min = 0;
  else if (typeof min === "function") {
    cb = min;
    min = 0;
  }

  if (
    !Number.isSafeInteger(min) ||
    typeof max === "number" && !Number.isSafeInteger(max)
  ) {
    throw new Error("max or min is not a Safe Number");
  }

  if (max - min > Math.pow(2, 48)) {
    throw new RangeError("max - min should be less than 2^48!");
  }

  if (min >= max) {
    throw new Error("Min is bigger than Max!");
  }

  min = Math.ceil(min);
  max = Math.floor(max);
  const result = op_node_random_int(min, max);

  if (cb) {
    cb(null, result);
    return;
  }

  return result;
}
