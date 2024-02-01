// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStrictEquals } from "./test_util.ts";

Deno.test(function matrixTranslate() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.translate(1, 2, 3);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2, m31:  3, m41:  1 * 1 +  2 * 2 +  3 * 3 +  4 * 1,
      m12:  5, m22:  6, m32:  7, m42:  5 * 1 +  6 * 2 +  7 * 3 +  8 * 1,
      m13:  9, m23: 10, m33: 11, m43:  9 * 1 + 10 * 2 + 11 * 3 + 12 * 1,
      m14: 13, m24: 14, m34: 15, m44: 13 * 1 + 14 * 2 + 15 * 3 + 16 * 1,
    }),
  );
});

Deno.test(function matrixTranslateSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.translateSelf(1, 2, 3);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2, m31:  3, m41:  1 * 1 +  2 * 2 +  3 * 3 +  4 * 1,
      m12:  5, m22:  6, m32:  7, m42:  5 * 1 +  6 * 2 +  7 * 3 +  8 * 1,
      m13:  9, m23: 10, m33: 11, m43:  9 * 1 + 10 * 2 + 11 * 3 + 12 * 1,
      m14: 13, m24: 14, m34: 15, m44: 13 * 1 + 14 * 2 + 15 * 3 + 16 * 1,
    }),
  );
});

Deno.test(function matrixMultiply() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.multiply({ m11: 1, m22: 2, m33: 3, m44: 4 });
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 1, m21:  2 * 2, m31:  3 * 3, m41:  4 * 4,
      m12:  5 * 1, m22:  6 * 2, m32:  7 * 3, m42:  8 * 4,
      m13:  9 * 1, m23: 10 * 2, m33: 11 * 3, m43: 12 * 4,
      m14: 13 * 1, m24: 14 * 2, m34: 15 * 3, m44: 16 * 4,
    }),
  );
});

Deno.test(function matrixMultiplySelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.multiplySelf({ m11: 1, m22: 2, m33: 3, m44: 4 });
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 1, m21:  2 * 2, m31:  3 * 3, m41:  4 * 4,
      m12:  5 * 1, m22:  6 * 2, m32:  7 * 3, m42:  8 * 4,
      m13:  9 * 1, m23: 10 * 2, m33: 11 * 3, m43: 12 * 4,
      m14: 13 * 1, m24: 14 * 2, m34: 15 * 3, m44: 16 * 4,
    }),
  );
});

Deno.test(function matrixMultiplySelfWithSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.multiplySelf(matrix);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  90, m21: 100, m31: 110, m41: 120,
      m12: 202, m22: 228, m32: 254, m42: 280,
      m13: 314, m23: 356, m33: 398, m43: 440,
      m14: 426, m24: 484, m34: 542, m44: 600,
    }),
  );
});

Deno.test(function matrixPreMultiplySelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.preMultiplySelf({ m11: 1, m22: 2, m33: 3, m44: 4 });
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 1, m21:  2 * 1, m31:  3 * 1, m41:  4 * 1,
      m12:  5 * 2, m22:  6 * 2, m32:  7 * 2, m42:  8 * 2,
      m13:  9 * 3, m23: 10 * 3, m33: 11 * 3, m43: 12 * 3,
      m14: 13 * 4, m24: 14 * 4, m34: 15 * 4, m44: 16 * 4,
    }),
  );
});

Deno.test(function matrixPreMultiplySelfWithSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.preMultiplySelf(matrix);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  90, m21: 100, m31: 110, m41: 120,
      m12: 202, m22: 228, m32: 254, m42: 280,
      m13: 314, m23: 356, m33: 398, m43: 440,
      m14: 426, m24: 484, m34: 542, m44: 600,
    }),
  );
});

