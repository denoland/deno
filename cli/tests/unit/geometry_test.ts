// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assertAlmostEquals,
  assertEquals,
  assertStrictEquals,
} from "./test_util.ts";

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

Deno.test(function matrixScale() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale(1, 2, 3);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2 * 2, m31:  3 * 3, m41:  4,
      m12:  5, m22:  6 * 2, m32:  7 * 3, m42:  8,
      m13:  9, m23: 10 * 2, m33: 11 * 3, m43: 12,
      m14: 13, m24: 14 * 2, m34: 15 * 3, m44: 16,
    }),
  );
});

Deno.test(function matrixScaleSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scaleSelf(1, 2, 3);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2 * 2, m31:  3 * 3, m41:  4,
      m12:  5, m22:  6 * 2, m32:  7 * 3, m42:  8,
      m13:  9, m23: 10 * 2, m33: 11 * 3, m43: 12,
      m14: 13, m24: 14 * 2, m34: 15 * 3, m44: 16,
    }),
  );
});

Deno.test(function matrixScaleWithOrigin() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale(1, 2, 3, 4, 5, 6);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2 * 2, m31:  3 * 3, m41:  -42,
      m12:  5, m22:  6 * 2, m32:  7 * 3, m42: -106,
      m13:  9, m23: 10 * 2, m33: 11 * 3, m43: -170,
      m14: 13, m24: 14 * 2, m34: 15 * 3, m44: -234,
    }),
  );
});

Deno.test(function matrixScaleWithOriginSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scaleSelf(1, 2, 3, 4, 5, 6);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2 * 2, m31:  3 * 3, m41:  -42,
      m12:  5, m22:  6 * 2, m32:  7 * 3, m42: -106,
      m13:  9, m23: 10 * 2, m33: 11 * 3, m43: -170,
      m14: 13, m24: 14 * 2, m34: 15 * 3, m44: -234,
    }),
  );
});

Deno.test(function matrixScaleNonUniform() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scaleNonUniform(1, 2);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  2 * 2, m31:  3, m41:  4,
      m12:  5, m22:  6 * 2, m32:  7, m42:  8,
      m13:  9, m23: 10 * 2, m33: 11, m43: 12,
      m14: 13, m24: 14 * 2, m34: 15, m44: 16,
    }),
  );
});

Deno.test(function matrixScale3d() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale3d(2);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 2, m21:  2 * 2, m31:  3 * 2, m41:  4,
      m12:  5 * 2, m22:  6 * 2, m32:  7 * 2, m42:  8,
      m13:  9 * 2, m23: 10 * 2, m33: 11 * 2, m43: 12,
      m14: 13 * 2, m24: 14 * 2, m34: 15 * 2, m44: 16,
    }),
  );
});

Deno.test(function matrixScale3dSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale3dSelf(2);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 2, m21:  2 * 2, m31:  3 * 2, m41:  4,
      m12:  5 * 2, m22:  6 * 2, m32:  7 * 2, m42:  8,
      m13:  9 * 2, m23: 10 * 2, m33: 11 * 2, m43: 12,
      m14: 13 * 2, m24: 14 * 2, m34: 15 * 2, m44: 16,
    }),
  );
});

Deno.test(function matrixScale3dWithOrigin() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale3d(2, 4, 5, 6);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 2, m21:  2 * 2, m31:  3 * 2, m41:  -28,
      m12:  5 * 2, m22:  6 * 2, m32:  7 * 2, m42:  -84,
      m13:  9 * 2, m23: 10 * 2, m33: 11 * 2, m43: -140,
      m14: 13 * 2, m24: 14 * 2, m34: 15 * 2, m44: -196,
    }),
  );
});

Deno.test(function matrixScale3dWithOriginSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.scale3dSelf(2, 4, 5, 6);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  assertEquals(
    matrix,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1 * 2, m21:  2 * 2, m31:  3 * 2, m41:  -28,
      m12:  5 * 2, m22:  6 * 2, m32:  7 * 2, m42:  -84,
      m13:  9 * 2, m23: 10 * 2, m33: 11 * 2, m43: -140,
      m14: 13 * 2, m24: 14 * 2, m34: 15 * 2, m44: -196,
    }),
  );
});

Deno.test(function matrixRotate() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  -3, m21:  -2, m31:  -1, m41:  4,
    m12:  -7, m22:  -6, m32:  -5, m42:  8,
    m13: -11, m23: -10, m33:  -9, m43: 12,
    m14: -15, m24: -14, m34: -13, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotate(0, 90, 180);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix2[key],
      value,
    );
  }
});

Deno.test(function matrixRotateSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  -3, m21:  -2, m31:  -1, m41:  4,
    m12:  -7, m22:  -6, m32:  -5, m42:  8,
    m13: -11, m23: -10, m33:  -9, m43: 12,
    m14: -15, m24: -14, m34: -13, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotateSelf(0, 90, 180);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix[key],
      value,
    );
  }
});

Deno.test(function matrixRotateFromVector() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  2.121320343559643, m21: 0.7071067811865476, m31:  3, m41:  4,
    m12:  7.778174593052023, m22: 0.7071067811865479, m32:  7, m42:  8,
    m13: 13.435028842544405, m23: 0.707106781186547 , m33: 11, m43: 12,
    m14: 19.091883092036785, m24: 0.7071067811865461, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotateFromVector(1, 1);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix2[key],
      value,
    );
  }
});

Deno.test(function matrixRotateFromVectorSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  2.121320343559643, m21: 0.7071067811865476, m31:  3, m41:  4,
    m12:  7.778174593052023, m22: 0.7071067811865479, m32:  7, m42:  8,
    m13: 13.435028842544405, m23: 0.7071067811865470, m33: 11, m43: 12,
    m14: 19.091883092036785, m24: 0.7071067811865461, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotateFromVectorSelf(1, 1);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix[key],
      value,
    );
  }
});

Deno.test(function matrixRotateAxisAngle() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  1,                 m21:  2,                 m31:  3,                  m41:  4,
    m12:  5.228294835332138, m22:  4.854398120227125, m32:  7.6876363080712045, m42:  8,
    m13:  9.456589670664275, m23:  7.708796240454249, m33: 12.3752726161424090, m43: 12,
    m14: 13.684884505996411, m24: 10.563194360681376, m34: 17.0629089242136120, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotateAxisAngle(1, 2, 3, 30);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix2[key],
      value,
    );
  }
});

Deno.test(function matrixRotateAxisAngleSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  1,                 m21:  2,                 m31:  3,                  m41:  4,
    m12:  5.228294835332138, m22:  4.854398120227125, m32:  7.6876363080712045, m42:  8,
    m13:  9.456589670664275, m23:  7.708796240454249, m33: 12.3752726161424090, m43: 12,
    m14: 13.684884505996411, m24: 10.563194360681376, m34: 17.0629089242136120, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.rotateAxisAngleSelf(1, 2, 3, 30);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix[key],
      value,
    );
  }
});

Deno.test(function matrixSkewX() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  1, m21:  2.5773502691896257, m31:  3, m41:  4,
    m12:  5, m22:  8.8867513459481270, m32:  7, m42:  8,
    m13:  9, m23: 15.1961524227066300, m33: 11, m43: 12,
    m14: 13, m24: 21.5055534994651330, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.skewX(30);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix2[key],
      value,
    );
  }
});

Deno.test(function matrixSkewXSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  1, m21:  2.5773502691896257, m31:  3, m41:  4,
    m12:  5, m22:  8.8867513459481270, m32:  7, m42:  8,
    m13:  9, m23: 15.1961524227066300, m33: 11, m43: 12,
    m14: 13, m24: 21.5055534994651330, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.skewXSelf(30);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix[key],
      value,
    );
  }
});

Deno.test(function matrixSkewY() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  2.1547005383792515, m21:  2, m31:  3, m41:  4,
    m12:  8.4641016151377530, m22:  6, m32:  7, m42:  8,
    m13: 14.7735026918962560, m23: 10, m33: 11, m43: 12,
    m14: 21.0829037686547600, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.skewY(30);
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix2[key],
      value,
    );
  }
});

Deno.test(function matrixSkewYSelf() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  // deno-fmt-ignore
  const expect = {
    m11:  2.1547005383792515, m21:  2, m31:  3, m41:  4,
    m12:  8.4641016151377530, m22:  6, m32:  7, m42:  8,
    m13: 14.7735026918962560, m23: 10, m33: 11, m43: 12,
    m14: 21.0829037686547600, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.skewYSelf(30);
  assertStrictEquals(
    matrix,
    matrix2,
  );
  for (
    const [key, value] of Object.entries(expect) as [
      keyof typeof expect,
      number,
    ][]
  ) {
    assertAlmostEquals(
      matrix[key],
      value,
    );
  }
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

Deno.test(function matrixflipX() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.flipX();
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  -1, m21:  2, m31:  3, m41:  4,
      m12:  -5, m22:  6, m32:  7, m42:  8,
      m13:  -9, m23: 10, m33: 11, m43: 12,
      m14: -13, m24: 14, m34: 15, m44: 16,
    }),
  );
});

Deno.test(function matrixflipX() {
  // deno-fmt-ignore
  const init = {
    m11:  1, m21:  2, m31:  3, m41:  4,
    m12:  5, m22:  6, m32:  7, m42:  8,
    m13:  9, m23: 10, m33: 11, m43: 12,
    m14: 13, m24: 14, m34: 15, m44: 16,
  };
  const matrix = DOMMatrix.fromMatrix(init);
  const matrix2 = matrix.flipY();
  assertEquals(
    matrix,
    DOMMatrix.fromMatrix(init),
  );
  assertEquals(
    matrix2,
    // deno-fmt-ignore
    DOMMatrix.fromMatrix({
      m11:  1, m21:  -2, m31:  3, m41:  4,
      m12:  5, m22:  -6, m32:  7, m42:  8,
      m13:  9, m23: -10, m33: 11, m43: 12,
      m14: 13, m24: -14, m34: 15, m44: 16,
    }),
  );
});

