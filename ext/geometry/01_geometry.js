// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  Float32Array,
  Float64Array,
  MathMax,
  MathMin,
  ObjectIs,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";

webidl.converters.DOMPointInit = webidl.createDictionaryConverter(
  "DOMPointInit",
  [
    {
      key: "x",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "y",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "z",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "w",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 1,
    },
  ],
);

webidl.converters.DOMRectInit = webidl.createDictionaryConverter(
  "DOMRectInit",
  [
    {
      key: "x",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "y",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "width",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "height",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
  ],
);

webidl.converters.DOMQuadInit = webidl.createDictionaryConverter(
  "DOMQuadInit",
  [
    {
      key: "p1",
      converter: webidl.converters.DOMPointInit,
    },
    {
      key: "p2",
      converter: webidl.converters.DOMPointInit,
    },
    {
      key: "p3",
      converter: webidl.converters.DOMPointInit,
    },
    {
      key: "p4",
      converter: webidl.converters.DOMPointInit,
    },
  ],
);

webidl.converters.DOMMatrixInit = webidl.createDictionaryConverter(
  "DOMMatrixInit",
  [
    {
      key: "a",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "b",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "c",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "d",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "e",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "f",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m11",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m12",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m21",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m22",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m41",
      converter: webidl.converters["unrestricted double"],
    },
    {
      key: "m42",
      converter: webidl.converters["unrestricted double"],
    },
  ],
  [
    {
      key: "m13",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m14",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m23",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m24",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m31",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m32",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m33",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 1,
    },
    {
      key: "m34",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m43",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 0,
    },
    {
      key: "m44",
      converter: webidl.converters["unrestricted double"],
      defaultValue: 1,
    },
    {
      key: "is2D",
      converter: webidl.converters["boolean"],
    },
  ],
);

const _raw = Symbol("[[raw]]");
const _brand = webidl.brand;

class DOMPointReadOnly {
  [_raw];

  constructor(x = 0, y = 0, z = 0, w = 1) {
    this[_raw] = new Float64Array([
      webidl.converters["unrestricted double"](x),
      webidl.converters["unrestricted double"](y),
      webidl.converters["unrestricted double"](z),
      webidl.converters["unrestricted double"](w),
    ]);
    this[_brand] = _brand;
  }

  static fromPoint(other = {}) {
    other = webidl.converters.DOMPointInit(
      other,
      "Failed to call 'DOMPointReadOnly.fromPoint'",
      "Argument 1",
    );
    const point = webidl.createBranded(DOMPointReadOnly);
    point[_raw] = new Float64Array([
      other.x,
      other.y,
      other.z,
      other.w,
    ]);
    return point;
  }

  get x() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_raw][0];
  }
  get y() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_raw][1];
  }
  get z() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_raw][2];
  }
  get w() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_raw][3];
  }

  // TODO
  matrixTransform() {}

  toJSON() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    const raw = this[_raw];
    return {
      x: raw[0],
      y: raw[1],
      z: raw[2],
      w: raw[3],
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMPointReadOnlyPrototype, this),
        keys: [
          "x",
          "y",
          "z",
          "w",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMPointReadOnlyPrototype = DOMPointReadOnly.prototype;

class DOMPoint extends DOMPointReadOnly {
  static fromPoint(other = {}) {
    other = webidl.converters.DOMPointInit(
      other,
      "Failed to call 'DOMPoint.fromPoint'",
      "Argument 1",
    );
    const point = webidl.createBranded(DOMPoint);
    point[_raw] = new Float64Array([
      other.x,
      other.y,
      other.z,
      other.w,
    ]);
    return point;
  }

  get x() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][0];
  }
  set x(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_raw][0] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][1];
  }
  set y(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_raw][1] = webidl.converters["unrestricted double"](value);
  }
  get z() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][2];
  }
  set z(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_raw][2] = webidl.converters["unrestricted double"](value);
  }
  get w() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][3];
  }
  set w(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_raw][3] = webidl.converters["unrestricted double"](value);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMPointPrototype, this),
        keys: [
          "x",
          "y",
          "z",
          "w",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMPointPrototype = DOMPoint.prototype;

class DOMRectReadOnly {
  [_raw];

  constructor(x = 0, y = 0, width = 0, height = 0) {
    this[_raw] = new Float64Array([
      webidl.converters["unrestricted double"](x),
      webidl.converters["unrestricted double"](y),
      webidl.converters["unrestricted double"](width),
      webidl.converters["unrestricted double"](height),
    ]);
    this[_brand] = _brand;
  }

  static fromRect(other = {}) {
    other = webidl.converters.DOMRectInit(
      other,
      "Failed to call 'DOMRectReadOnly.fromRect'",
      "Argument 1",
    );
    const rect = webidl.createBranded(DOMRectReadOnly);
    rect[_raw] = new Float64Array([
      other.x,
      other.y,
      other.width,
      other.height,
    ]);
    return rect;
  }

  get x() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_raw][0];
  }
  get y() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_raw][1];
  }
  get width() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_raw][2];
  }
  get height() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_raw][3];
  }
  get top() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    const raw = this[_raw];
    return MathMin(raw[1], raw[1] + raw[3]);
  }
  get right() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    const raw = this[_raw];
    return MathMax(raw[0], raw[0] + raw[2]);
  }
  get bottom() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    const raw = this[_raw];
    return MathMax(raw[1], raw[1] + raw[3]);
  }
  get left() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    const raw = this[_raw];
    return MathMin(raw[0], raw[0] + raw[2]);
  }

  toJSON() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    const raw = this[_raw];
    return {
      x: raw[0],
      y: raw[1],
      width: raw[2],
      height: raw[3],
      top: MathMin(raw[1], raw[1] + raw[3]),
      right: MathMax(raw[0], raw[0] + raw[2]),
      bottom: MathMax(raw[1], raw[1] + raw[3]),
      left: MathMin(raw[0], raw[0] + raw[2]),
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMRectReadOnlyPrototype, this),
        keys: [
          "x",
          "y",
          "width",
          "height",
          "top",
          "right",
          "bottom",
          "left",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMRectReadOnlyPrototype = DOMRectReadOnly.prototype;

class DOMRect extends DOMRectReadOnly {
  static fromRect(other = {}) {
    other = webidl.converters.DOMRectInit(
      other,
      "Failed to call 'DOMRect.fromRect'",
      "Argument 1",
    );
    const rect = webidl.createBranded(DOMRect);
    rect[_raw] = new Float64Array([
      other.x,
      other.y,
      other.width,
      other.height,
    ]);
    return rect;
  }

  get x() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][0];
  }
  set x(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_raw][0] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][1];
  }
  set y(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_raw][1] = webidl.converters["unrestricted double"](value);
  }
  get width() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][2];
  }
  set width(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_raw][2] = webidl.converters["unrestricted double"](value);
  }
  get height() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][3];
  }
  set height(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_raw][3] = webidl.converters["unrestricted double"](value);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMRectPrototype, this),
        keys: [
          "x",
          "y",
          "width",
          "height",
          "top",
          "right",
          "bottom",
          "left",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMRectPrototype = DOMRect.prototype;

const _p1 = Symbol("[[p1]]");
const _p2 = Symbol("[[p2]]");
const _p3 = Symbol("[[p3]]");
const _p4 = Symbol("[[p4]]");

class DOMQuad {
  [_p1];
  [_p2];
  [_p3];
  [_p4];

  constructor(p1 = {}, p2 = {}, p3 = {}, p4 = {}) {
    this[_p1] = DOMPoint.fromPoint(p1);
    this[_p2] = DOMPoint.fromPoint(p2);
    this[_p3] = DOMPoint.fromPoint(p3);
    this[_p4] = DOMPoint.fromPoint(p4);
    this[_brand] = _brand;
  }

  static fromRect(other = {}) {
    other = webidl.converters.DOMRectInit(
      other,
      "Failed to call 'DOMQuad.fromRect'",
      "Argument 1",
    );
    const { x, y, width, height } = other;
    const point = webidl.createBranded(DOMQuad);
    point[_p1] = new DOMPoint(x, y, 0, 1);
    point[_p2] = new DOMPoint(x + width, y, 0, 1);
    point[_p3] = new DOMPoint(x + width, y + height, 0, 1);
    point[_p4] = new DOMPoint(x, y + height, 0, 1);
    return point;
  }

  static fromQuad(other = {}) {
    other = webidl.converters.DOMQuadInit(
      other,
      "Failed to call 'DOMQuad.fromQuad'",
      "Argument 1",
    );
    const point = webidl.createBranded(DOMQuad);
    point[_p1] = DOMPoint.fromPoint(other.p1);
    point[_p2] = DOMPoint.fromPoint(other.p2);
    point[_p3] = DOMPoint.fromPoint(other.p3);
    point[_p4] = DOMPoint.fromPoint(other.p4);
    return point;
  }

  get p1() {
    webidl.assertBranded(this, DOMQuadPrototype);
    return this[_p1];
  }
  get p2() {
    webidl.assertBranded(this, DOMQuadPrototype);
    return this[_p2];
  }
  get p3() {
    webidl.assertBranded(this, DOMQuadPrototype);
    return this[_p3];
  }
  get p4() {
    webidl.assertBranded(this, DOMQuadPrototype);
    return this[_p4];
  }

  getBounds() {
    webidl.assertBranded(this, DOMQuadPrototype);
    const { x: p1x, y: p1y } = this[_p1];
    const { x: p2x, y: p2y } = this[_p2];
    const { x: p3x, y: p3y } = this[_p3];
    const { x: p4x, y: p4y } = this[_p4];

    const left = MathMin(p1x, p2x, p3x, p4x);
    const top = MathMin(p1y, p2y, p3y, p4y);
    const right = MathMax(p1x, p2x, p3x, p4x);
    const bottom = MathMax(p1y, p2y, p3y, p4y);

    const bounds = webidl.createBranded(DOMRect);
    bounds[_raw] = new Float64Array([
      left,
      top,
      right - left,
      bottom - top,
    ]);
    return bounds;
  }

  toJSON() {
    webidl.assertBranded(this, DOMQuadPrototype);
    return {
      p1: this[_p1],
      p2: this[_p2],
      p3: this[_p3],
      p4: this[_p4],
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMQuadPrototype, this),
        keys: [
          "p1",
          "p2",
          "p3",
          "p4",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMQuadPrototype = DOMQuad.prototype;

const _a = 0;
const _b = 1;
const _c = 4;
const _d = 5;
const _e = 12;
const _f = 13;
const _m11 = 0;
const _m12 = 1;
const _m13 = 2;
const _m14 = 3;
const _m21 = 4;
const _m22 = 5;
const _m23 = 6;
const _m24 = 7;
const _m31 = 8;
const _m32 = 9;
const _m33 = 10;
const _m34 = 11;
const _m41 = 12;
const _m42 = 13;
const _m43 = 14;
const _m44 = 15;
const _is2D = Symbol("[[is2D]]");

class DOMMatrixReadOnly {
  [_raw];
  [_is2D];

  constructor(init = undefined) {
    const prefix = `Failed to construct '${this.constructor.name}'`;
    if (typeof init === "string") {
      // TODO(petamoriken): Add parser in Window global object
      throw new TypeError(
        `${prefix}: CSS <transform-list> parser is not implemented`,
      );
    }
    this[_brand] = _brand;
    if (init === undefined) {
      // deno-fmt-ignore
      this[_raw] = new Float64Array([
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
      ]);
      this[_is2D] = true;
    } else {
      init = webidl.converters["sequence<unrestricted double>"](
        init,
        prefix,
        "Argument 1",
      );
      initMatrixFromSequence(this, init, prefix);
    }
  }

  // TODO: fast path from DOMMatrix, DOMMatrixReadOnly
  static fromMatrix(other = {}) {
    const prefix = "Failed to call 'DOMMatrixReadOnly.fromMatrix'";
    other = webidl.converters.DOMMatrixInit(other, prefix, "Argument 1");
    validateAndFixupMatrix(other, prefix);
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    initMatrixFromDictonary(matrix, other);
    return matrix;
  }

  static fromFloat32Array(float32) {
    const prefix = "Failed to call 'DOMMatrixReadOnly.fromFloat32Array'";
    float32 = webidl.converters.Float32Array(float32, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    initMatrixFromSequence(matrix, float32, prefix);
    return matrix;
  }

  static fromFloat64Array(float64) {
    const prefix = "Failed to call 'DOMMatrixReadOnly.fromFloat64Array'";
    float64 = webidl.converters.Float64Array(float64, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    initMatrixFromSequence(matrix, float64, prefix);
    return matrix;
  }

  get a() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_a];
  }
  get b() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_b];
  }
  get c() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_c];
  }
  get d() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_d];
  }
  get e() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_e];
  }
  get f() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_f];
  }
  get m11() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m11];
  }
  get m12() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m12];
  }
  get m13() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m13];
  }
  get m14() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m14];
  }
  get m21() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m21];
  }
  get m22() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m22];
  }
  get m23() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m23];
  }
  get m24() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m24];
  }
  get m31() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m31];
  }
  get m32() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m32];
  }
  get m33() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m33];
  }
  get m34() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m34];
  }
  get m41() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m41];
  }
  get m42() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m42];
  }
  get m43() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m43];
  }
  get m44() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][_m44];
  }
  get is2D() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_is2D];
  }
  get isIdentity() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return isMatrixIdentity(this);
  }

  toFloat32Array() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return new Float32Array(this[_raw]);
  }

  toFloat64Array() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return new Float64Array(this[_raw]);
  }

  toJSON() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const raw = this[_raw];
    return {
      a: raw[_a],
      b: raw[_b],
      c: raw[_c],
      d: raw[_d],
      e: raw[_e],
      f: raw[_f],
      m11: raw[_m11],
      m12: raw[_m12],
      m13: raw[_m13],
      m14: raw[_m14],
      m21: raw[_m21],
      m22: raw[_m22],
      m23: raw[_m23],
      m24: raw[_m24],
      m31: raw[_m31],
      m32: raw[_m32],
      m33: raw[_m33],
      m34: raw[_m34],
      m41: raw[_m41],
      m42: raw[_m42],
      m43: raw[_m43],
      m44: raw[_m44],
      is2D: this[_is2D],
      isIdentity: isMatrixIdentity(this),
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          DOMMatrixReadOnlyPrototype,
          this,
        ),
        keys: [
          "a",
          "b",
          "c",
          "d",
          "e",
          "f",
          "m11",
          "m12",
          "m13",
          "m14",
          "m21",
          "m22",
          "m23",
          "m24",
          "m31",
          "m32",
          "m33",
          "m34",
          "m41",
          "m42",
          "m43",
          "m44",
          "is2D",
          "isIdentity",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMMatrixReadOnlyPrototype = DOMMatrixReadOnly.prototype;

class DOMMatrix extends DOMMatrixReadOnly {
  // TODO: fast path from DOMMatrix, DOMMatrixReadOnly
  static fromMatrix(other = {}) {
    const prefix = "Failed to call 'DOMMatrix.fromMatrix'";
    other = webidl.converters.DOMMatrixInit(other, prefix, "Argument 1");
    validateAndFixupMatrix(other, prefix);
    const matrix = webidl.createBranded(DOMMatrix);
    initMatrixFromDictonary(matrix, other);
    return matrix;
  }

  static fromFloat32Array(float32) {
    const prefix = "Failed to call 'DOMMatrix.fromFloat32Array'";
    float32 = webidl.converters.Float32Array(float32, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrix);
    initMatrixFromSequence(matrix, float32, prefix);
    return matrix;
  }

  static fromFloat64Array(float64) {
    const prefix = "Failed to call 'DOMMatrix.fromFloat64Array'";
    float64 = webidl.converters.Float64Array(float64, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrix);
    initMatrixFromSequence(matrix, float64, prefix);
    return matrix;
  }

  get a() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_a];
  }
  set a(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_a] = webidl.converters["unrestricted double"](value);
  }
  get b() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_b];
  }
  set b(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_b] = webidl.converters["unrestricted double"](value);
  }
  get c() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_c];
  }
  set c(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_c] = webidl.converters["unrestricted double"](value);
  }
  get d() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_d];
  }
  set d(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_d] = webidl.converters["unrestricted double"](value);
  }
  get e() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_e];
  }
  set e(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_e] = webidl.converters["unrestricted double"](value);
  }
  get f() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_f];
  }
  set f(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_f] = webidl.converters["unrestricted double"](value);
  }
  get m11() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m11];
  }
  set m11(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m11] = webidl.converters["unrestricted double"](value);
  }
  get m12() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m12];
  }
  set m12(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m12] = webidl.converters["unrestricted double"](value);
  }
  get m13() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m13];
  }
  set m13(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m13] = webidl.converters["unrestricted double"](value);
  }
  get m14() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m14];
  }
  set m14(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m14] = webidl.converters["unrestricted double"](value);
  }
  get m21() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m21];
  }
  set m21(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m21] = webidl.converters["unrestricted double"](value);
  }
  get m22() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m22];
  }
  set m22(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m22] = webidl.converters["unrestricted double"](value);
  }
  get m23() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m23];
  }
  set m23(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m23] = webidl.converters["unrestricted double"](value);
  }
  get m24() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m24];
  }
  set m24(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m24] = webidl.converters["unrestricted double"](value);
  }
  get m31() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m31];
  }
  set m31(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m31] = webidl.converters["unrestricted double"](value);
  }
  get m32() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m32];
  }
  set m32(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m32] = webidl.converters["unrestricted double"](value);
  }
  get m33() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m33];
  }
  set m33(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 1) {
      this[_is2D] = false;
    }
    this[_raw][_m33] = webidl.converters["unrestricted double"](value);
  }
  get m34() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m34];
  }
  set m34(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m34] = webidl.converters["unrestricted double"](value);
  }
  get m41() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m41];
  }
  set m41(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m41] = webidl.converters["unrestricted double"](value);
  }
  get m42() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m42];
  }
  set m42(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    this[_raw][_m42] = webidl.converters["unrestricted double"](value);
  }
  get m43() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m43];
  }
  set m43(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][_m43] = webidl.converters["unrestricted double"](value);
  }
  get m44() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][_m44];
  }
  set m44(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    if (value !== 1) {
      this[_is2D] = false;
    }
    this[_raw][_m44] = webidl.converters["unrestricted double"](value);
  }

  // TODO(petamoriken): Add setMatrixValue in Window global object
  // setMatrixValue(transformList) {}

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMMatrixPrototype, this),
        keys: [
          "a",
          "b",
          "c",
          "d",
          "e",
          "f",
          "m11",
          "m12",
          "m13",
          "m14",
          "m21",
          "m22",
          "m23",
          "m24",
          "m31",
          "m32",
          "m33",
          "m34",
          "m41",
          "m42",
          "m43",
          "m44",
          "is2D",
          "isIdentity",
        ],
      }),
      inspectOptions,
    );
  }
}

const DOMMatrixPrototype = DOMMatrix.prototype;

/**
 * https://tc39.es/ecma262/#sec-samevaluezero
 * @param {number} x
 * @param {number} y
 */
function sameValueZero(x, y) {
  return x === y || ObjectIs(x, y);
}

/**
 * https://drafts.fxtf.org/geometry/#matrix-validate-and-fixup-2d
 * @param {object} init
 * @param {string} prefix
 */
function validateAndFixup2DMatrix(init, prefix) {
  if (
    (
      init.a !== undefined && init.m11 !== undefined &&
      !sameValueZero(init.a, init.m11)
    ) ||
    (
      init.b !== undefined && init.m12 !== undefined &&
      !sameValueZero(init.b, init.m12)
    ) ||
    (
      init.c !== undefined && init.m21 !== undefined &&
      !sameValueZero(init.c, init.m21)
    ) ||
    (
      init.d !== undefined && init.m22 !== undefined &&
      !sameValueZero(init.d, init.m22)
    ) ||
    (
      init.e !== undefined && init.m41 !== undefined &&
      !sameValueZero(init.e, init.m41)
    ) ||
    (
      init.f !== undefined && init.m42 !== undefined &&
      !sameValueZero(init.f, init.m42)
    )
  ) {
    throw new TypeError(`${prefix}: Inconsistent 2d matrix value`);
  }
  if (init.m11 === undefined) init.m11 = init.a ?? 1;
  if (init.m12 === undefined) init.m12 = init.b ?? 0;
  if (init.m21 === undefined) init.m21 = init.c ?? 0;
  if (init.m22 === undefined) init.m22 = init.d ?? 1;
  if (init.m41 === undefined) init.m41 = init.e ?? 0;
  if (init.m42 === undefined) init.m42 = init.f ?? 0;
}

/**
 * https://drafts.fxtf.org/geometry/#matrix-validate-and-fixup
 * @param {object} init
 * @param {string} prefix
 */
function validateAndFixupMatrix(init, prefix) {
  validateAndFixup2DMatrix(init, prefix);
  const is2DCanBeTrue = init.m13 === 0 &&
    init.m14 === 0 &&
    init.m23 === 0 &&
    init.m24 === 0 &&
    init.m31 === 0 &&
    init.m32 === 0 &&
    init.m33 === 1 &&
    init.m34 === 0 &&
    init.m43 === 0 &&
    init.m44 === 1;
  if (init.is2D === true && !is2DCanBeTrue) {
    throw new TypeError(
      `${prefix}: is2D property is true but the input matrix is a 3d matrix`,
    );
  }
  if (init.is2D === undefined) {
    init.is2D = is2DCanBeTrue;
  }
}

/**
 * @param {object} target
 * @param {number[] | Float32Array | Float64Array} seq
 * @param {string} prefix
 */
function initMatrixFromSequence(target, seq, prefix) {
  if (seq.length === 6) {
    const { 0: a, 1: b, 2: c, 3: d, 4: e, 5: f } = seq;
    // deno-fmt-ignore
    target[_raw] = new Float64Array([
      a, b, 0, 0,
      c, d, 0, 0,
      0, 0, 1, 0,
      e, f, 0, 1,
    ]);
    target[_is2D] = true;
  } else if (seq.length === 16) {
    target[_raw] = new Float64Array(seq);
    target[_is2D] = false;
  } else {
    throw new TypeError(
      `${prefix}: The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix`,
    );
  }
}

/**
 * @param {object} target
 * @param {object} dict
 */
function initMatrixFromDictonary(target, dict) {
  if (dict.is2D) {
    const { m11, m12, m21, m22, m41, m42 } = dict;
    // deno-fmt-ignore
    target[_raw] = new Float64Array([
      m11, m12, 0, 0,
      m21, m22, 0, 0,
        0,   0, 1, 0,
      m41, m42, 0, 1,
    ]);
    target[_is2D] = true;
  } else {
    // deno-fmt-ignore
    const {
      m11, m12, m13, m14,
      m21, m22, m23, m24,
      m31, m32, m33, m34,
      m41, m42, m43, m44,
    } = dict;
    // deno-fmt-ignore
    target[_raw] = new Float64Array([
      m11, m12, m13, m14,
      m21, m22, m23, m24,
      m31, m32, m33, m34,
      m41, m42, m43, m44,
    ]);
    target[_is2D] = false;
  }
}

/**
 * https://drafts.fxtf.org/geometry/#dom-dommatrixreadonly-isidentity
 * @param {DOMMatrixReadOnly} matrix
 */
function isMatrixIdentity(matrix) {
  const raw = matrix[_raw];
  return (
    raw[_m11] === 1 &&
    raw[_m12] === 0 &&
    raw[_m13] === 0 &&
    raw[_m14] === 0 &&
    raw[_m21] === 0 &&
    raw[_m22] === 1 &&
    raw[_m23] === 0 &&
    raw[_m24] === 0 &&
    raw[_m31] === 0 &&
    raw[_m32] === 0 &&
    raw[_m33] === 1 &&
    raw[_m34] === 0 &&
    raw[_m41] === 0 &&
    raw[_m42] === 0 &&
    raw[_m43] === 0 &&
    raw[_m44] === 1
  );
}

export {
  DOMMatrix,
  DOMMatrixPrototype,
  DOMMatrixReadOnly,
  DOMMatrixReadOnlyPrototype,
  DOMPoint,
  DOMPointPrototype,
  DOMPointReadOnly,
  DOMPointReadOnlyPrototype,
  DOMQuad,
  DOMQuadPrototype,
  DOMRect,
  DOMRectPrototype,
  DOMRectReadOnly,
  DOMRectReadOnlyPrototype,
};
