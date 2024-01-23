// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  Float64Array,
  MathMax,
  MathMin,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";

webidl.converters["DOMPointInit"] = webidl.createDictionaryConverter(
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

webidl.converters["DOMRectInit"] = webidl.createDictionaryConverter(
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
    other = webidl.converters["DOMPointInit"](
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
    other = webidl.converters["DOMPointInit"](
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
    other = webidl.converters["DOMRectInit"](
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
    other = webidl.converters["DOMRectInit"](
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

export {
  DOMPoint,
  DOMPointPrototype,
  DOMPointReadOnly,
  DOMPointReadOnlyPrototype,
  DOMRect,
  DOMRectPrototype,
  DOMRectReadOnly,
  DOMRectReadOnlyPrototype,
};
