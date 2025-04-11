// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  DOMMatrixInner,
  DOMPoint,
  DOMPointReadOnly,
  DOMQuad,
  DOMRect,
  DOMRectReadOnly,
} from "ext:core/ops";
const {
  ArrayPrototypeJoin,
  Float32Array,
  Float64Array,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
  TypedArrayPrototypeJoin,
} = primordials;

import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";

const DOMPointPrototype = DOMPoint.prototype;
const DOMPointReadOnlyPrototype = DOMPointReadOnly.prototype;
ObjectDefineProperty(DOMPointReadOnlyPrototype, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value: function customInspect(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          DOMPointReadOnlyPrototype,
          this,
        ),
        keys: ["x", "y", "z", "w"],
      }),
      inspectOptions,
    );
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

const DOMRectPrototype = DOMRect.prototype;
const DOMRectReadOnlyPrototype = DOMRectReadOnly.prototype;
ObjectDefineProperty(DOMRectReadOnlyPrototype, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value: function customInspect(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          DOMRectReadOnlyPrototype,
          this,
        ),
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
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

const DOMQuadPrototype = DOMQuad.prototype;
ObjectDefineProperty(DOMQuadPrototype, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value: function customInspect(inspect, inspectOptions) {
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
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

const _inner = Symbol("[[inner]]");
// Property to prevent writing values when an immutable instance is changed to
// a mutable instance by Object.setPrototypeOf
// TODO(petamoriken): Implementing resistance to Object.setPrototypeOf in the WebIDL layer
const _writable = Symbol("[[writable]]");
const _brand = webidl.brand;

class DOMMatrixReadOnly {
  [_writable] = false;
  /** @type {DOMMatrixInner} */
  [_inner];

  constructor(init = undefined) {
    const prefix = `Failed to construct '${this.constructor.name}'`;
    this[_brand] = _brand;
    if (init === undefined) {
      this[_inner] = DOMMatrixInner.identity();
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
      this[_inner] = new DOMMatrixInner(matrix, is2D);
    }
  }

  static fromMatrix(other = { __proto__: null }) {
    const matrix = webidl.createBranded(DOMMatrixReadOnly);
    matrix[_writable] = false;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (
      other !== null &&
      ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)
    ) {
      matrix[_inner] = other[_inner].clone();
    } else {
      matrix[_inner] = DOMMatrixInner.fromMatrix(other);
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
    return this[_inner].a;
  }

  get b() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].b;
  }

  get c() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].c;
  }

  get d() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].d;
  }

  get e() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].e;
  }

  get f() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].f;
  }

  get m11() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m11;
  }

  get m12() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m12;
  }

  get m13() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m13;
  }

  get m14() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m14;
  }

  get m21() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m21;
  }

  get m22() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m22;
  }

  get m23() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m23;
  }

  get m24() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m24;
  }

  get m31() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m31;
  }

  get m32() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m32;
  }

  get m33() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m33;
  }

  get m34() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m34;
  }

  get m41() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m41;
  }

  get m42() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m42;
  }

  get m43() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m43;
  }

  get m44() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].m44;
  }

  get is2D() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].is2D;
  }

  get isIdentity() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].isIdentity;
  }

  translate(tx = 0, ty = 0, tz = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].translate(tx, ty, tz);
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
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    if (originX === 0 && originY === 0 && originZ === 0) {
      matrix[_inner] = this[_inner].scaleWithoutOrigin(
        scaleX,
        scaleY,
        scaleZ,
      );
    } else {
      matrix[_inner] = this[_inner].scaleWithOrigin(
        scaleX,
        scaleY,
        scaleZ,
        originX,
        originY,
        originZ,
      );
    }
    return matrix;
  }

  scaleNonUniform(scaleX = 1, scaleY = 1) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].scaleWithoutOrigin(
      scaleX,
      scaleY,
      1,
    );
    return matrix;
  }

  scale3d(scale = 1, originX = 0, originY = 0, originZ = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    if (originX === 0 && originY === 0 && originZ === 0) {
      matrix[_inner] = this[_inner].scaleWithoutOrigin(
        scale,
        scale,
        scale,
      );
    } else {
      matrix[_inner] = this[_inner].scaleWithOrigin(
        scale,
        scale,
        scale,
        originX,
        originY,
        originZ,
      );
    }
    return matrix;
  }

  rotate(rotX = 0, rotY, rotZ) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    if (rotY === undefined && rotZ === undefined) {
      rotZ = rotX;
      rotX = 0;
      rotY = 0;
    } else {
      rotY = rotY !== undefined ? rotY : 0;
      rotZ = rotZ !== undefined ? rotZ : 0;
    }
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].rotate(
      rotX,
      rotY,
      rotZ,
    );
    return matrix;
  }

  rotateFromVector(x = 0, y = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].rotateFromVector(x, y);
    return matrix;
  }

  rotateAxisAngle(x = 0, y = 0, z = 0, angle = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].rotateAxisAngle(
      x,
      y,
      z,
      angle,
    );
    return matrix;
  }

  skewX(sx = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].skewX(sx);
    return matrix;
  }

  skewY(sy = 0) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].skewY(sy);
    return matrix;
  }

  multiply(other = { __proto__: null }) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    let otherInner;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (
      other !== null &&
      ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)
    ) {
      otherInner = other[_inner];
    } else {
      otherInner = DOMMatrixInner.fromMatrix(other);
    }
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].multiply(otherInner);
    return matrix;
  }

  flipX() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].flipX();
    return matrix;
  }

  flipY() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].flipY();
    return matrix;
  }

  inverse() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    matrix[_inner] = this[_inner].inverse();
    return matrix;
  }

  transformPoint(point = { __proto__: null }) {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    let pointInner;
    // fast path for DOMPoint or DOMPointReadOnly
    if (
      point !== null &&
      ObjectPrototypeIsPrototypeOf(DOMPointReadOnlyPrototype, point)
    ) {
      pointInner = point[_inner];
    } else {
      pointInner = DOMPointInner.fromPoint(point);
    }
    const result = webidl.createBranded(DOMPoint);
    result[_writable] = true;
    result[_inner] = this[_inner].transformPoint(pointInner);
    return result;
  }

  toFloat32Array() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return new Float32Array(
      new Float64Array(
        this[_inner].toBuffer(),
      ),
    );
  }

  toFloat64Array() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return new Float64Array(this[_inner].toBuffer());
  }

  toJSON() {
    webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
    return this[_inner].toJSON();
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

  static fromMatrix(other = { __proto__: null }) {
    const matrix = webidl.createBranded(DOMMatrix);
    matrix[_writable] = true;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (
      other !== null &&
      ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)
    ) {
      matrix[_inner] = other[_inner].clone();
    } else {
      matrix[_inner] = DOMMatrixInner.fromMatrix(other);
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
    return this[_inner].a;
  }
  set a(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].a = value;
  }

  get b() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].b;
  }
  set b(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].b = value;
  }

  get c() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].c;
  }
  set c(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].c = value;
  }

  get d() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].d;
  }
  set d(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].d = value;
  }

  get e() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].e;
  }
  set e(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].e = value;
  }

  get f() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].f;
  }
  set f(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].f = value;
  }

  get m11() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m11;
  }
  set m11(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m11 = value;
  }

  get m12() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m12;
  }
  set m12(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m12 = value;
  }

  get m13() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m13;
  }
  set m13(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m13 = value;
  }

  get m14() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m14;
  }
  set m14(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m14 = value;
  }

  get m21() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m21;
  }
  set m21(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m21 = value;
  }

  get m22() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m22;
  }
  set m22(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m22 = value;
  }

  get m23() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m23;
  }
  set m23(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m23 = value;
  }

  get m24() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m24;
  }
  set m24(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m24 = value;
  }

  get m31() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m31;
  }
  set m31(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m31 = value;
  }

  get m32() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m32;
  }
  set m32(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m32 = value;
  }

  get m33() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m33;
  }
  set m33(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m33 = value;
  }

  get m34() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m34;
  }
  set m34(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m34 = value;
  }

  get m41() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m41;
  }
  set m41(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m41 = value;
  }

  get m42() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m42;
  }
  set m42(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m42 = value;
  }

  get m43() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m43;
  }
  set m43(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m43 = value;
  }

  get m44() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    return this[_inner].m44;
  }
  set m44(value) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].m44 = value;
  }

  multiplySelf(other = { __proto__: null }) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    let otherInner;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (
      other !== null &&
      ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)
    ) {
      otherInner = other[_inner];
    } else {
      otherInner = DOMMatrixInner.fromMatrix(other);
    }
    this[_inner].multiplySelf(otherInner);
    return this;
  }

  preMultiplySelf(other = { __proto__: null }) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    let otherInner;
    // fast path for DOMMatrix or DOMMatrixReadOnly
    if (
      other !== null &&
      ObjectPrototypeIsPrototypeOf(DOMMatrixReadOnlyPrototype, other)
    ) {
      otherInner = other[_inner];
    } else {
      otherInner = DOMMatrixInner.fromMatrix(other);
    }
    this[_inner].preMultiplySelf(otherInner);
    return this;
  }

  translateSelf(tx = 0, ty = 0, tz = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].translateSelf(tx, ty, tz);
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
    if (originX === 0 && originY === 0 && originZ === 0) {
      this[_inner].scaleWithoutOriginSelf(scaleX, scaleY, scaleZ);
    } else {
      this[_inner].scaleWithOriginSelf(
        scaleX,
        scaleY,
        scaleZ,
        originX,
        originY,
        originZ,
      );
    }
    return this;
  }

  scale3dSelf(scale = 1, originX = 0, originY = 0, originZ = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (originX === 0 && originY === 0 && originZ === 0) {
      this[_inner].scaleWithoutOriginSelf(scale, scale, scale);
    } else {
      this[_inner].scaleWithOriginSelf(
        scale,
        scale,
        scale,
        originX,
        originY,
        originZ,
      );
    }
    return this;
  }

  rotateSelf(rotX = 0, rotY, rotZ) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    if (rotY === undefined && rotZ === undefined) {
      rotZ = rotX;
      rotX = 0;
      rotY = 0;
    } else {
      rotY = rotY !== undefined ? rotY : 0;
      rotZ = rotZ !== undefined ? rotZ : 0;
    }
    this[_inner].rotateSelf(
      rotX,
      rotY,
      rotZ,
    );
    return this;
  }

  rotateFromVectorSelf(x = 0, y = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].rotateFromVectorSelf(x, y);
    return this;
  }

  rotateAxisAngleSelf(x = 0, y = 0, z = 0, angle = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].rotateAxisAngleSelf(
      x,
      y,
      z,
      angle,
    );
    return this;
  }

  skewXSelf(sx = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].skewXSelf(sx);
    return this;
  }

  skewYSelf(sy = 0) {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].skewYSelf(sy);
    return this;
  }

  invertSelf() {
    webidl.assertBranded(this, DOMMatrixPrototype);
    assertWritable(this);
    this[_inner].invertSelf();
    return this;
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
 * @param {object} target
 * @param {number[] | Float32Array | Float64Array} seq
 * @param {string} prefix
 */
function initMatrixFromSequence(target, seq, prefix) {
  if (seq.length === 6) {
    const { 0: a, 1: b, 2: c, 3: d, 4: e, 5: f } = seq;
    // deno-fmt-ignore
    target[_inner] = new DOMMatrixInner(new Float64Array([
      a, b, 0, 0,
      c, d, 0, 0,
      0, 0, 1, 0,
      e, f, 0, 1,
    ]), /* is2D */ true);
  } else if (seq.length === 16) {
    target[_inner] = new DOMMatrixInner(
      new Float64Array(seq),
      /* is2D */ false,
    );
  } else {
    throw new TypeError(
      `${prefix}: The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix`,
    );
  }
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
      __proto__: null,
      value: function toString() {
        webidl.assertBranded(this, DOMMatrixReadOnlyPrototype);
        const inner = this[_inner];
        if (!inner.isFinite) {
          throw new DOMException(
            "Failed to execute 'toString' on 'DOMMatrixReadOnly': Cannot be serialized with NaN or Infinity values",
            "InvalidStateError",
          );
        }
        if (inner.is2D) {
          return `matrix(${
            ArrayPrototypeJoin([
              inner.a,
              inner.b,
              inner.c,
              inner.d,
              inner.e,
              inner.f,
            ], ", ")
          })`;
        } else {
          return `matrix3d(${
            TypedArrayPrototypeJoin(new Float64Array(inner.toBuffer()), ", ")
          })`;
        }
      },
      writable: true,
      enumerable: true,
      configurable: true,
    });

    // https://drafts.fxtf.org/geometry/#dom-dommatrix-setmatrixvalue
    ObjectDefineProperty(DOMMatrixPrototype, "setMatrixValue", {
      __proto__: null,
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
        this[_inner] = new DOMMatrixInner(matrix, is2D);
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
