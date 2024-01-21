// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
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

export {
  DOMPoint,
  DOMPointPrototype,
  DOMPointReadOnly,
  DOMPointReadOnlyPrototype,
};
