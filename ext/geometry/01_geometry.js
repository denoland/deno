// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  op_geometry_flip_x_self,
  op_geometry_flip_y_self,
  op_geometry_invert_2d_self,
  op_geometry_invert_self,
  op_geometry_multiply,
  op_geometry_multiply_self,
  op_geometry_premultiply_point_self,
  op_geometry_premultiply_self,
  op_geometry_rotate_axis_angle_self,
  op_geometry_rotate_from_vector_self,
  op_geometry_rotate_self,
  op_geometry_scale_self,
  op_geometry_scale_with_origin_self,
  op_geometry_skew_self,
  op_geometry_translate_self,
} from "ext:core/ops";
const {
  ArrayPrototypeJoin,
  Float32Array,
  Float64Array,
  MathMax,
  MathMin,
  NumberIsFinite,
  ObjectDefineProperty,
  ObjectIs,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
  TypedArrayPrototypeEvery,
  TypedArrayPrototypeJoin,
} = primordials;

import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";

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

/** @type {webidl.Dictionary} */
const dictDOMMatrix2DInit = [
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
];

webidl.converters.DOMMatrix2DInit = webidl.createDictionaryConverter(
  "DOMMatrix2DInit",
  dictDOMMatrix2DInit,
);

webidl.converters.DOMMatrixInit = webidl.createDictionaryConverter(
  "DOMMatrixInit",
  dictDOMMatrix2DInit,
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
// Property to prevent writing values when an immutable instance is changed to
// a mutable instance by Object.setPrototypeOf
// TODO(petamoriken): Implementing resistance to Object.setPrototypeOf in the WebIDL layer
const _writable = Symbol("[[writable]]");
const _brand = webidl.brand;

class DOMPointReadOnly {
  [_writable] = false;
  /** @type {Float64Array} */
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
      "Failed to execute 'DOMPointReadOnly.fromPoint'",
      "Argument 1",
    );
    const point = webidl.createBranded(DOMPointReadOnly);
    point[_writable] = false;
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

  matrixTransform(matrix = {}) {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    const prefix = "Failed to execute 'matrixTransform' on 'DOMPointReadOnly'";
    if (!ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, matrix)) {
      const _matrix = webidl.converters.DOMMatrixInit(
        matrix,
        prefix,
        "Argument 1",
      );
      validateAndFixupMatrixDictionary(_matrix, prefix);
      matrix = {};
      initMatrixFromDictonary(matrix, _matrix);
    }

    const point = webidl.createBranded(DOMPoint);
    point[_writable] = true;
    point[_raw] = new Float64Array(this[_raw]);
    op_geometry_premultiply_point_self(matrix[_raw], point[_raw]);
    return point;
  }

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

webidl.configureInterface(DOMPointReadOnly);
const DOMPointReadOnlyPrototype = DOMPointReadOnly.prototype;

class DOMPoint extends DOMPointReadOnly {
  [_writable] = true;

  static fromPoint(other = {}) {
    other = webidl.converters.DOMPointInit(
      other,
      "Failed to execute 'DOMPoint.fromPoint'",
      "Argument 1",
    );
    const point = webidl.createBranded(DOMPoint);
    point[_writable] = true;
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
    assertWritable(this);
    this[_raw][0] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][1];
  }
  set y(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    assertWritable(this);
    this[_raw][1] = webidl.converters["unrestricted double"](value);
  }
  get z() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][2];
  }
  set z(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    assertWritable(this);
    this[_raw][2] = webidl.converters["unrestricted double"](value);
  }
  get w() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_raw][3];
  }
  set w(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    assertWritable(this);
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

webidl.configureInterface(DOMPoint);
const DOMPointPrototype = DOMPoint.prototype;

class DOMRectReadOnly {
  [_writable] = false;
  /** @type {Float64Array} */
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
      "Failed to execute 'DOMRectReadOnly.fromRect'",
      "Argument 1",
    );
    const rect = webidl.createBranded(DOMRectReadOnly);
    rect[_writable] = false;
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

webidl.configureInterface(DOMRectReadOnly);
const DOMRectReadOnlyPrototype = DOMRectReadOnly.prototype;

class DOMRect extends DOMRectReadOnly {
  [_writable] = true;

  static fromRect(other = {}) {
    other = webidl.converters.DOMRectInit(
      other,
      "Failed to execute 'DOMRect.fromRect'",
      "Argument 1",
    );
    const rect = webidl.createBranded(DOMRect);
    rect[_writable] = true;
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
    assertWritable(this);
    this[_raw][0] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][1];
  }
  set y(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    assertWritable(this);
    this[_raw][1] = webidl.converters["unrestricted double"](value);
  }
  get width() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][2];
  }
  set width(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    assertWritable(this);
    this[_raw][2] = webidl.converters["unrestricted double"](value);
  }
  get height() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_raw][3];
  }
  set height(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    assertWritable(this);
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

webidl.configureInterface(DOMRect);
const DOMRectPrototype = DOMRect.prototype;

const _p1 = Symbol("[[p1]]");
const _p2 = Symbol("[[p2]]");
const _p3 = Symbol("[[p3]]");
const _p4 = Symbol("[[p4]]");

class DOMQuad {
  /** @type {DOMPoint} */
  [_p1];
  /** @type {DOMPoint} */
  [_p2];
  /** @type {DOMPoint} */
  [_p3];
  /** @type {DOMPoint} */
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
      "Failed to execute 'DOMQuad.fromRect'",
      "Argument 1",
    );
    const { x, y, width, height } = other;
    const quad = webidl.createBranded(DOMQuad);
    quad[_p1] = new DOMPoint(x, y, 0, 1);
    quad[_p2] = new DOMPoint(x + width, y, 0, 1);
    quad[_p3] = new DOMPoint(x + width, y + height, 0, 1);
    quad[_p4] = new DOMPoint(x, y + height, 0, 1);
    return quad;
  }

  static fromQuad(other = {}) {
    other = webidl.converters.DOMQuadInit(
      other,
      "Failed to execute 'DOMQuad.fromQuad'",
      "Argument 1",
    );
    const quad = webidl.createBranded(DOMQuad);
    quad[_p1] = DOMPoint.fromPoint(other.p1);
    quad[_p2] = DOMPoint.fromPoint(other.p2);
    quad[_p3] = DOMPoint.fromPoint(other.p3);
    quad[_p4] = DOMPoint.fromPoint(other.p4);
    return quad;
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
    bounds[_writable] = true;
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

webidl.configureInterface(DOMQuad);
const DOMQuadPrototype = DOMQuad.prototype;

/*
 * NOTE: column-major order
 *
 * For a 2D 3x2 matrix, the index of properties in
 * | a c 0 e |    | 0 4 _ 12 |
 * | b d 0 f |    | 1 5 _ 13 |
 * | 0 0 1 0 | is | _ _ _  _ |
 * | 0 0 0 1 |    | _ _ _  _ |
 */
const INDEX_A = 0;
const INDEX_B = 1;
const INDEX_C = 4;
const INDEX_D = 5;
const INDEX_E = 12;
const INDEX_F = 13;

/*
 * NOTE: column-major order
 *
 * The index of properties in
 * | m11 m21 m31 m41 |    | 0 4  8 12 |
 * | m12 m22 m32 m42 |    | 1 5  9 13 |
 * | m13 m23 m33 m43 | is | 2 6 10 14 |
 * | m14 m24 m34 m44 |    | 3 7 11 15 |
 */
const INDEX_M11 = 0;
const INDEX_M12 = 1;
const INDEX_M13 = 2;
const INDEX_M14 = 3;
const INDEX_M21 = 4;
const INDEX_M22 = 5;
const INDEX_M23 = 6;
const INDEX_M24 = 7;
const INDEX_M31 = 8;
const INDEX_M32 = 9;
const INDEX_M33 = 10;
const INDEX_M34 = 11;
const INDEX_M41 = 12;
const INDEX_M42 = 13;
const INDEX_M43 = 14;
const INDEX_M44 = 15;

const _is2D = Symbol("[[is2D]]");

class DOMMatrixReadOnly {
  [_writable] = false;
  /** @type {Float64Array} */
  [_raw];
  /** @type {boolean} */
  [_is2D];

  constructor(init = undefined) {
    const prefix = `Failed to construct '${this.constructor.name}'`;
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
    } else if (
      webidl.type(init) === "Object" && init[SymbolIterator] !== undefined
    ) {
      init = webidl.converters["sequence<unrestricted double>"](
        init,
        prefix,
        "Argument 1",
      );
      initMatrixFromSequence(this, init, prefix);
    } else {
      init = webidl.converters.DOMString(
        init,
        prefix,
        "Argument 1",
      );
      const { matrix, is2D } = parseTransformList(init, prefix);
      this[_raw] = matrix;
      this[_is2D] = is2D;
    }
  }

  static fromMatrix(other = {}) {
    const prefix = "Failed to execute 'DOMMatrixReadOnly.fromMatrix'";
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    matrix[_writable] = false;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)) {
      initMatrixFromMatrix(matrix, other);
    } else {
      other = webidl.converters.DOMMatrixInit(other, prefix, "Argument 1");
      validateAndFixupMatrixDictionary(other, prefix);
      initMatrixFromDictonary(matrix, other);
    }
    return matrix;
  }

  static fromFloat32Array(float32) {
    const prefix = "Failed to execute 'DOMMatrixReadOnly.fromFloat32Array'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    float32 = webidl.converters.Float32Array(float32, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    matrix[_writable] = false;
    initMatrixFromSequence(matrix, float32, prefix);
    return matrix;
  }

  static fromFloat64Array(float64) {
    const prefix = "Failed to execute 'DOMMatrixReadOnly.fromFloat64Array'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    float64 = webidl.converters.Float64Array(float64, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    matrix[_writable] = false;
    initMatrixFromSequence(matrix, float64, prefix);
    return matrix;
  }

  get a() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_A];
  }
  get b() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_B];
  }
  get c() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_C];
  }
  get d() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_D];
  }
  get e() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_E];
  }
  get f() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_F];
  }
  get m11() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M11];
  }
  get m12() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M12];
  }
  get m13() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M13];
  }
  get m14() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M14];
  }
  get m21() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M21];
  }
  get m22() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M22];
  }
  get m23() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M23];
  }
  get m24() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M24];
  }
  get m31() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M31];
  }
  get m32() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M32];
  }
  get m33() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M33];
  }
  get m34() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M34];
  }
  get m41() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M41];
  }
  get m42() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M42];
  }
  get m43() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M43];
  }
  get m44() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_raw][INDEX_M44];
  }
  get is2D() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_is2D];
  }
  get isIdentity() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return isIdentityMatrix(this);
  }

  translate(tx = 0, ty = 0, tz = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    tx = webidl.converters["unrestricted double"](tx);
    ty = webidl.converters["unrestricted double"](ty);
    tz = webidl.converters["unrestricted double"](tz);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_translate_self(
      tx,
      ty,
      tz,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D] && tz === 0;
    return matrix;
  }

  scale(
    scaleX = 1,
    scaleY = scaleX,
    scaleZ = 1,
    originX = 0,
    originY = 0,
    originZ = 0,
  ) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    scaleX = webidl.converters["unrestricted double"](scaleX);
    scaleY = webidl.converters["unrestricted double"](scaleY);
    scaleZ = webidl.converters["unrestricted double"](scaleZ);
    originX = webidl.converters["unrestricted double"](originX);
    originY = webidl.converters["unrestricted double"](originY);
    originZ = webidl.converters["unrestricted double"](originZ);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    if (originX === 0 && originY === 0 && originZ === 0) {
      op_geometry_scale_self(
        scaleX,
        scaleY,
        scaleZ,
        matrix[_raw],
      );
    } else {
      op_geometry_scale_with_origin_self(
        scaleX,
        scaleY,
        scaleZ,
        originX,
        originY,
        originZ,
        matrix[_raw],
      );
    }
    matrix[_is2D] = this[_is2D] && scaleZ === 1 && originZ === 0;
    return matrix;
  }

  scaleNonUniform(scaleX = 1, scaleY = 1) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    scaleX = webidl.converters["unrestricted double"](scaleX);
    scaleY = webidl.converters["unrestricted double"](scaleY);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_scale_self(
      scaleX,
      scaleY,
      1,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  scale3d(scale = 1, originX = 0, originY = 0, originZ = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    scale = webidl.converters["unrestricted double"](scale);
    originX = webidl.converters["unrestricted double"](originX);
    originY = webidl.converters["unrestricted double"](originY);
    originZ = webidl.converters["unrestricted double"](originZ);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    if (originX === 0 && originY === 0 && originZ === 0) {
      op_geometry_scale_self(
        scale,
        scale,
        scale,
        matrix[_raw],
      );
    } else {
      op_geometry_scale_with_origin_self(
        scale,
        scale,
        scale,
        originX,
        originY,
        originZ,
        matrix[_raw],
      );
    }
    matrix[_is2D] = this[_is2D] && scale === 1 && originZ === 0;
    return matrix;
  }

  rotate(rotX = 0, rotY, rotZ) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    rotX = webidl.converters["unrestricted double"](rotX);
    if (rotY === undefined && rotZ === undefined) {
      rotZ = rotX;
      rotX = 0;
      rotY = 0;
    } else {
      rotY = rotY !== undefined
        ? webidl.converters["unrestricted double"](rotY)
        : 0;
      rotZ = rotZ !== undefined
        ? webidl.converters["unrestricted double"](rotZ)
        : 0;
    }
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_rotate_self(
      rotX,
      rotY,
      rotZ,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D] && rotX === 0 && rotY === 0;
    return matrix;
  }

  rotateFromVector(x = 0, y = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    x = webidl.converters["unrestricted double"](x);
    y = webidl.converters["unrestricted double"](y);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_rotate_from_vector_self(
      x,
      y,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  rotateAxisAngle(x = 0, y = 0, z = 0, angle = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    x = webidl.converters["unrestricted double"](x);
    y = webidl.converters["unrestricted double"](y);
    z = webidl.converters["unrestricted double"](z);
    angle = webidl.converters["unrestricted double"](angle);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    if (x !== 0 || y !== 0 || z !== 0) {
      op_geometry_rotate_axis_angle_self(
        x,
        y,
        z,
        angle,
        matrix[_raw],
      );
    }
    matrix[_is2D] = this[_is2D] && x === 0 && y === 0;
    return matrix;
  }

  skewX(sx = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    sx = webidl.converters["unrestricted double"](sx);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_skew_self(
      sx,
      0,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  skewY(sy = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    sy = webidl.converters["unrestricted double"](sy);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_skew_self(
      0,
      sy,
      matrix[_raw],
    );
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  multiply(other = {}) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const prefix = "Failed to execute 'multiply' on 'DOMMatrixReadOnly'";
    if (!ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)) {
      const _other = webidl.converters.DOMMatrixInit(
        other,
        prefix,
        "Argument 1",
      );
      validateAndFixupMatrixDictionary(_other, prefix);
      other = {};
      initMatrixFromDictonary(other, _other);
    }
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(16);
    op_geometry_multiply(this[_raw], other[_raw], matrix[_raw]);
    matrix[_is2D] = this[_is2D] && other[_is2D];
    return matrix;
  }

  flipX() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_flip_x_self(matrix[_raw]);
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  flipY() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    op_geometry_flip_y_self(matrix[_raw]);
    matrix[_is2D] = this[_is2D];
    return matrix;
  }

  inverse() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_raw] = new Float64Array(this[_raw]);
    let invertible;
    if (this[_is2D]) {
      invertible = op_geometry_invert_2d_self(matrix[_raw]);
    } else {
      invertible = op_geometry_invert_self(matrix[_raw]);
    }
    matrix[_is2D] = this[_is2D] && invertible;
    return matrix;
  }

  transformPoint(point = {}) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    point = webidl.converters.DOMPointInit(
      point,
      "Failed to execute 'transformPoint' on 'DOMMatrixReadOnly'",
      "Argument 1",
    );
    const result = webidl.createBranded(DOMPoint);
    result[_writable] = true;
    result[_raw] = new Float64Array([
      point.x,
      point.y,
      point.z,
      point.w,
    ]);
    op_geometry_premultiply_point_self(this[_raw], result[_raw]);
    return result;
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
      a: raw[INDEX_A],
      b: raw[INDEX_B],
      c: raw[INDEX_C],
      d: raw[INDEX_D],
      e: raw[INDEX_E],
      f: raw[INDEX_F],
      m11: raw[INDEX_M11],
      m12: raw[INDEX_M12],
      m13: raw[INDEX_M13],
      m14: raw[INDEX_M14],
      m21: raw[INDEX_M21],
      m22: raw[INDEX_M22],
      m23: raw[INDEX_M23],
      m24: raw[INDEX_M24],
      m31: raw[INDEX_M31],
      m32: raw[INDEX_M32],
      m33: raw[INDEX_M33],
      m34: raw[INDEX_M34],
      m41: raw[INDEX_M41],
      m42: raw[INDEX_M42],
      m43: raw[INDEX_M43],
      m44: raw[INDEX_M44],
      is2D: this[_is2D],
      isIdentity: isIdentityMatrix(this),
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

webidl.configureInterface(DOMMatrixReadOnly);
const DOMMatrixReadOnlyPrototype = DOMMatrixReadOnly.prototype;

class DOMMatrix extends DOMMatrixReadOnly {
  [_writable] = true;

  static fromMatrix(other = {}) {
    const prefix = "Failed to execute 'DOMMatrix.fromMatrix'";
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)) {
      initMatrixFromMatrix(matrix, other);
    } else {
      other = webidl.converters.DOMMatrixInit(other, prefix, "Argument 1");
      validateAndFixupMatrixDictionary(other, prefix);
      initMatrixFromDictonary(matrix, other);
    }
    return matrix;
  }

  static fromFloat32Array(float32) {
    const prefix = "Failed to execute 'DOMMatrix.fromFloat32Array'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    float32 = webidl.converters.Float32Array(float32, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    initMatrixFromSequence(matrix, float32, prefix);
    return matrix;
  }

  static fromFloat64Array(float64) {
    const prefix = "Failed to execute 'DOMMatrix.fromFloat64Array'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    float64 = webidl.converters.Float64Array(float64, prefix, "Argument 1");
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    initMatrixFromSequence(matrix, float64, prefix);
    return matrix;
  }

  get a() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_A];
  }
  set a(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_A] = webidl.converters["unrestricted double"](value);
  }
  get b() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_B];
  }
  set b(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_B] = webidl.converters["unrestricted double"](value);
  }
  get c() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_C];
  }
  set c(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_C] = webidl.converters["unrestricted double"](value);
  }
  get d() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_D];
  }
  set d(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_D] = webidl.converters["unrestricted double"](value);
  }
  get e() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_E];
  }
  set e(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_E] = webidl.converters["unrestricted double"](value);
  }
  get f() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_F];
  }
  set f(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_F] = webidl.converters["unrestricted double"](value);
  }
  get m11() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M11];
  }
  set m11(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M11] = webidl.converters["unrestricted double"](value);
  }
  get m12() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M12];
  }
  set m12(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M12] = webidl.converters["unrestricted double"](value);
  }
  get m13() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M13];
  }
  set m13(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M13] = webidl.converters["unrestricted double"](value);
  }
  get m14() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M14];
  }
  set m14(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M14] = webidl.converters["unrestricted double"](value);
  }
  get m21() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M21];
  }
  set m21(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M21] = webidl.converters["unrestricted double"](value);
  }
  get m22() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M22];
  }
  set m22(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M22] = webidl.converters["unrestricted double"](value);
  }
  get m23() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M23];
  }
  set m23(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M23] = webidl.converters["unrestricted double"](value);
  }
  get m24() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M24];
  }
  set m24(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M24] = webidl.converters["unrestricted double"](value);
  }
  get m31() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M31];
  }
  set m31(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M31] = webidl.converters["unrestricted double"](value);
  }
  get m32() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M32];
  }
  set m32(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M32] = webidl.converters["unrestricted double"](value);
  }
  get m33() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M33];
  }
  set m33(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 1) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M33] = webidl.converters["unrestricted double"](value);
  }
  get m34() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M34];
  }
  set m34(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M34] = webidl.converters["unrestricted double"](value);
  }
  get m41() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M41];
  }
  set m41(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M41] = webidl.converters["unrestricted double"](value);
  }
  get m42() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M42];
  }
  set m42(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_raw][INDEX_M42] = webidl.converters["unrestricted double"](value);
  }
  get m43() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M43];
  }
  set m43(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 0) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M43] = webidl.converters["unrestricted double"](value);
  }
  get m44() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_raw][INDEX_M44];
  }
  set m44(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (value !== 1) {
      this[_is2D] = false;
    }
    this[_raw][INDEX_M44] = webidl.converters["unrestricted double"](value);
  }

  multiplySelf(other = {}) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    const prefix = "Failed to execute 'multiplySelf' on 'DOMMatrix'";
    if (!ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)) {
      const _other = webidl.converters.DOMMatrixInit(
        other,
        prefix,
        "Argument 1",
      );
      validateAndFixupMatrixDictionary(_other, prefix);
      other = {};
      initMatrixFromDictonary(other, _other);
    } else if (this[_raw] === other[_raw]) {
      other = {};
      initMatrixFromMatrix(other, this);
    }

    op_geometry_multiply_self(other[_raw], this[_raw]);
    this[_is2D] &&= other[_is2D];
    return this;
  }

  preMultiplySelf(other = {}) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    const prefix = "Failed to execute 'premultiplySelf' on 'DOMMatrix'";
    if (!ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)) {
      const _other = webidl.converters.DOMMatrixInit(
        other,
        prefix,
        "Argument 1",
      );
      validateAndFixupMatrixDictionary(_other, prefix);
      other = {};
      initMatrixFromDictonary(other, _other);
    } else if (this[_raw] === other[_raw]) {
      other = {};
      initMatrixFromMatrix(other, this);
    }

    op_geometry_premultiply_self(other[_raw], this[_raw]);
    this[_is2D] &&= other[_is2D];
    return this;
  }

  translateSelf(tx = 0, ty = 0, tz = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    tx = webidl.converters["unrestricted double"](tx);
    ty = webidl.converters["unrestricted double"](ty);
    tz = webidl.converters["unrestricted double"](tz);
    op_geometry_translate_self(
      tx,
      ty,
      tz,
      this[_raw],
    );
    this[_is2D] &&= tz === 0;
    return this;
  }

  scaleSelf(
    scaleX = 1,
    scaleY = scaleX,
    scaleZ = 1,
    originX = 0,
    originY = 0,
    originZ = 0,
  ) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    scaleX = webidl.converters["unrestricted double"](scaleX);
    scaleY = webidl.converters["unrestricted double"](scaleY);
    scaleZ = webidl.converters["unrestricted double"](scaleZ);
    originX = webidl.converters["unrestricted double"](originX);
    originY = webidl.converters["unrestricted double"](originY);
    originZ = webidl.converters["unrestricted double"](originZ);
    if (originX === 0 && originY === 0 && originZ === 0) {
      op_geometry_scale_self(
        scaleX,
        scaleY,
        scaleZ,
        this[_raw],
      );
    } else {
      op_geometry_scale_with_origin_self(
        scaleX,
        scaleY,
        scaleZ,
        originX,
        originY,
        originZ,
        this[_raw],
      );
    }
    this[_is2D] &&= scaleZ === 1 && originZ === 0;
    return this;
  }

  scale3dSelf(scale = 1, originX = 0, originY = 0, originZ = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    scale = webidl.converters["unrestricted double"](scale);
    originX = webidl.converters["unrestricted double"](originX);
    originY = webidl.converters["unrestricted double"](originY);
    originZ = webidl.converters["unrestricted double"](originZ);
    if (originX === 0 && originY === 0 && originZ === 0) {
      op_geometry_scale_self(
        scale,
        scale,
        scale,
        this[_raw],
      );
    } else {
      op_geometry_scale_with_origin_self(
        scale,
        scale,
        scale,
        originX,
        originY,
        originZ,
        this[_raw],
      );
    }
    this[_is2D] &&= scale === 1 && originZ === 0;
    return this;
  }

  rotateSelf(rotX = 0, rotY, rotZ) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    rotX = webidl.converters["unrestricted double"](rotX);
    if (rotY === undefined && rotZ === undefined) {
      rotZ = rotX;
      rotX = 0;
      rotY = 0;
    } else {
      rotY = rotY !== undefined
        ? webidl.converters["unrestricted double"](rotY)
        : 0;
      rotZ = rotZ !== undefined
        ? webidl.converters["unrestricted double"](rotZ)
        : 0;
    }
    op_geometry_rotate_self(
      rotX,
      rotY,
      rotZ,
      this[_raw],
    );
    this[_is2D] &&= rotX === 0 && rotY === 0;
    return this;
  }

  rotateFromVectorSelf(x = 0, y = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    x = webidl.converters["unrestricted double"](x);
    y = webidl.converters["unrestricted double"](y);
    op_geometry_rotate_from_vector_self(
      x,
      y,
      this[_raw],
    );
    return this;
  }

  rotateAxisAngleSelf(x = 0, y = 0, z = 0, angle = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    x = webidl.converters["unrestricted double"](x);
    y = webidl.converters["unrestricted double"](y);
    z = webidl.converters["unrestricted double"](z);
    angle = webidl.converters["unrestricted double"](angle);
    if (x !== 0 || y !== 0 || z !== 0) {
      op_geometry_rotate_axis_angle_self(
        x,
        y,
        z,
        angle,
        this[_raw],
      );
    }
    this[_is2D] &&= x === 0 && y === 0;
    return this;
  }

  skewXSelf(sx = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    sx = webidl.converters["unrestricted double"](sx);
    op_geometry_skew_self(
      sx,
      0,
      this[_raw],
    );
    return this;
  }

  skewYSelf(sy = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    sy = webidl.converters["unrestricted double"](sy);
    op_geometry_skew_self(
      0,
      sy,
      this[_raw],
    );
    return this;
  }

  invertSelf() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    let invertible;
    if (this[_is2D]) {
      invertible = op_geometry_invert_2d_self(this[_raw]);
    } else {
      invertible = op_geometry_invert_self(this[_raw]);
    }
    this[_is2D] &&= invertible;
    return this;
  }

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

webidl.configureInterface(DOMMatrix);
const DOMMatrixPrototype = DOMMatrix.prototype;

/**
 * TODO(petamoriken): Support this by updating WebIDL's brand features
 * @param {DOMRect | DOMPoint | DOMMatrix} self
 */
function assertWritable(self) {
  if (self[_writable] !== true) {
    throw new TypeError("Illegal invocation");
  }
}

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
 * @param {object} dict
 * @param {string} prefix
 */
function validateAndFixup2DMatrixDictionary(dict, prefix) {
  if (
    (
      dict.a !== undefined && dict.m11 !== undefined &&
      !sameValueZero(dict.a, dict.m11)
    ) ||
    (
      dict.b !== undefined && dict.m12 !== undefined &&
      !sameValueZero(dict.b, dict.m12)
    ) ||
    (
      dict.c !== undefined && dict.m21 !== undefined &&
      !sameValueZero(dict.c, dict.m21)
    ) ||
    (
      dict.d !== undefined && dict.m22 !== undefined &&
      !sameValueZero(dict.d, dict.m22)
    ) ||
    (
      dict.e !== undefined && dict.m41 !== undefined &&
      !sameValueZero(dict.e, dict.m41)
    ) ||
    (
      dict.f !== undefined && dict.m42 !== undefined &&
      !sameValueZero(dict.f, dict.m42)
    )
  ) {
    throw new TypeError(`${prefix}: Inconsistent 2d matrix value`);
  }
  if (dict.m11 === undefined) dict.m11 = dict.a ?? 1;
  if (dict.m12 === undefined) dict.m12 = dict.b ?? 0;
  if (dict.m21 === undefined) dict.m21 = dict.c ?? 0;
  if (dict.m22 === undefined) dict.m22 = dict.d ?? 1;
  if (dict.m41 === undefined) dict.m41 = dict.e ?? 0;
  if (dict.m42 === undefined) dict.m42 = dict.f ?? 0;
}

/**
 * https://drafts.fxtf.org/geometry/#matrix-validate-and-fixup
 * @param {object} dict
 * @param {string} prefix
 */
function validateAndFixupMatrixDictionary(dict, prefix) {
  validateAndFixup2DMatrixDictionary(dict, prefix);
  const is2DCanBeTrue = dict.m13 === 0 &&
    dict.m14 === 0 &&
    dict.m23 === 0 &&
    dict.m24 === 0 &&
    dict.m31 === 0 &&
    dict.m32 === 0 &&
    dict.m33 === 1 &&
    dict.m34 === 0 &&
    dict.m43 === 0 &&
    dict.m44 === 1;
  if (dict.is2D === true && !is2DCanBeTrue) {
    throw new TypeError(
      `${prefix}: is2D property is true but the input matrix is a 3d matrix`,
    );
  }
  if (dict.is2D === undefined) {
    dict.is2D = is2DCanBeTrue;
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
    const {
      m11,
      m12,
      m13,
      m14,
      m21,
      m22,
      m23,
      m24,
      m31,
      m32,
      m33,
      m34,
      m41,
      m42,
      m43,
      m44,
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
 * @param {object} target
 * @type {DOMMatrixReadOnly} matrix
 */
function initMatrixFromMatrix(target, matrix) {
  target[_raw] = new Float64Array(matrix[_raw]);
  target[_is2D] = matrix[_is2D];
}

/**
 * https://drafts.fxtf.org/geometry/#dom-dommatrixreadonly-isidentity
 * @param {DOMMatrixReadOnly} matrix
 */
function isIdentityMatrix(matrix) {
  const raw = matrix[_raw];
  return (
    raw[INDEX_M11] === 1 &&
    raw[INDEX_M12] === 0 &&
    raw[INDEX_M13] === 0 &&
    raw[INDEX_M14] === 0 &&
    raw[INDEX_M21] === 0 &&
    raw[INDEX_M22] === 1 &&
    raw[INDEX_M23] === 0 &&
    raw[INDEX_M24] === 0 &&
    raw[INDEX_M31] === 0 &&
    raw[INDEX_M32] === 0 &&
    raw[INDEX_M33] === 1 &&
    raw[INDEX_M34] === 0 &&
    raw[INDEX_M41] === 0 &&
    raw[INDEX_M42] === 0 &&
    raw[INDEX_M43] === 0 &&
    raw[INDEX_M44] === 1
  );
}

/**
 * CSS <transform-list> parser
 * @type {((transformList: string, prefix: string) => { matrix: Float64Array, is2D: boolean })}
 */
let parseTransformList;

/**
 * @param {(transformList: string, prefix: string) => { matrix: Float64Array, is2D: boolean }} transformListParser
 * @param {boolean} enableWindowFeatures
 */
function init(transformListParser, enableWindowFeatures) {
  parseTransformList = transformListParser;

  if (enableWindowFeatures) {
    // https://drafts.fxtf.org/geometry/#dommatrixreadonly-stringification-behavior
    ObjectDefineProperty(DOMMatrixReadOnlyPrototype, "toString", {
      value: function toString() {
        webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
        const raw = this[_raw];
        if (!TypedArrayPrototypeEvery(raw, (value) => NumberIsFinite(value))) {
          throw new DOMException(
            "Failed to execute 'toString' on 'DOMMatrixReadOnly': Cannot be serialized with NaN or Infinity values",
            "InvalidStateError",
          );
        }
        if (this[_is2D]) {
          return `matrix(${
            ArrayPrototypeJoin([
              raw[INDEX_A],
              raw[INDEX_B],
              raw[INDEX_C],
              raw[INDEX_D],
              raw[INDEX_E],
              raw[INDEX_F],
            ], ", ")
          })`;
        } else {
          return `matrix3d(${TypedArrayPrototypeJoin(raw, ", ")})`;
        }
      },
      writable: true,
      enumerable: true,
      configurable: true,
    });

    // https://drafts.fxtf.org/geometry/#dom-dommatrix-setmatrixvalue
    ObjectDefineProperty(DOMMatrixPrototype, "setMatrixValue", {
      value: function setMatrixValue(transformList) {
        webidl.assertBranded(this, DOMMatrixPrototype);
        const prefix = "Failed to execute 'setMatrixValue' on 'DOMMatrix'";
        webidl.requiredArguments(arguments.length, 1, prefix);
        transformList = webidl.converters.DOMString(
          transformList,
          prefix,
          "Argument 1",
        );
        const { matrix, is2D } = parseTransformList(transformList, prefix);
        this[_raw] = matrix;
        this[_is2D] = is2D;
      },
      writable: true,
      enumerable: true,
      configurable: true,
    });
  }
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
  init,
};
