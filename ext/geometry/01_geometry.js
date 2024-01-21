// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  MathMax,
  MathMin,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";

const _brand = webidl.brand;

const _x = Symbol("[[x]]");
const _y = Symbol("[[y]]");
const _z = Symbol("[[z]]");
const _w = Symbol("[[w]]");
const _width = Symbol("[[width]]");
const _height = Symbol("[[height]]");

class DOMPointReadOnly {
  [_x];
  [_y];
  [_z];
  [_w];

  constructor(x = 0, y = 0, z = 0, w = 1) {
    this[_x] = webidl.converters["unrestricted double"](x);
    this[_y] = webidl.converters["unrestricted double"](y);
    this[_z] = webidl.converters["unrestricted double"](z);
    this[_w] = webidl.converters["unrestricted double"](w);
    this[_brand] = _brand;
  }

  static fromPoint(other = {}) {
    return new DOMPointReadOnly(
      other.x,
      other.y,
      other.z,
      other.w,
    );
  }

  get x() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_x];
  }
  get y() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_y];
  }
  get z() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_z];
  }
  get w() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return this[_w];
  }

  // TODO
  matrixTransform() {}

  toJSON() {
    webidl.assertBranded(this, DOMPointReadOnlyPrototype);
    return {
      x: this[_x],
      y: this[_y],
      z: this[_z],
      w: this[_w],
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
    return new DOMPoint(
      other.x,
      other.y,
      other.z,
      other.w,
    );
  }

  get x() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_x];
  }
  set x(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_x] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_y];
  }
  set y(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_y] = webidl.converters["unrestricted double"](value);
  }
  get z() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_z];
  }
  set z(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_z] = webidl.converters["unrestricted double"](value);
  }
  get w() {
    webidl.assertBranded(this, DOMPointPrototype);
    return this[_w];
  }
  set w(value) {
    webidl.assertBranded(this, DOMPointPrototype);
    this[_w] = webidl.converters["unrestricted double"](value);
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
  [_x];
  [_y];
  [_width];
  [_height];

  constructor(x = 0, y = 0, width = 0, height = 0) {
    this[_x] = webidl.converters["unrestricted double"](x);
    this[_y] = webidl.converters["unrestricted double"](y);
    this[_width] = webidl.converters["unrestricted double"](width);
    this[_height] = webidl.converters["unrestricted double"](height);
    this[_brand] = _brand;
  }

  static fromRect(other = {}) {
    return new DOMRectReadOnly(
      other.x,
      other.y,
      other.width,
      other.height,
    );
  }

  get x() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_x];
  }
  get y() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_y];
  }
  get width() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_width];
  }
  get height() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return this[_height];
  }
  get top() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return MathMin(this[_y], this[_y] + this[_height]);
  }
  get right() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return MathMax(this[_x], this[_x] + this[_width]);
  }
  get bottom() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return MathMax(this[_y], this[_y] + this[_height]);
  }
  get left() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return MathMin(this[_x], this[_x] + this[_width]);
  }

  toJSON() {
    webidl.assertBranded(this, DOMRectReadOnlyPrototype);
    return {
      x: this[_x],
      y: this[_y],
      width: this[_width],
      height: this[_height],
      top: MathMin(this[_y], this[_y] + this[_height]),
      right: MathMax(this[_x], this[_x] + this[_width]),
      bottom: MathMax(this[_y], this[_y] + this[_height]),
      left: MathMin(this[_x], this[_x] + this[_width]),
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
    return new DOMRect(
      other.x,
      other.y,
      other.width,
      other.height,
    );
  }

  get x() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_x];
  }
  set x(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_x] = webidl.converters["unrestricted double"](value);
  }
  get y() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_y];
  }
  set y(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_y] = webidl.converters["unrestricted double"](value);
  }
  get width() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_width];
  }
  set width(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_width] = webidl.converters["unrestricted double"](value);
  }
  get height() {
    webidl.assertBranded(this, DOMRectPrototype);
    return this[_height];
  }
  set height(value) {
    webidl.assertBranded(this, DOMRectPrototype);
    this[_height] = webidl.converters["unrestricted double"](value);
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
