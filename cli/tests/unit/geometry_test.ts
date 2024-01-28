// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(function matrixMultiply() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.multiply(init);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  90, m21: 100, m31: 110, m41: 120,
      m12: 202, m22: 228, m32: 254, m42: 280,
      m13: 314, m23: 356, m33: 398, m43: 440,
      m14: 426, m24: 484, m34: 542, m44: 600,
    }),
  );
});
