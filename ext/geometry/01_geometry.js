// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  DOMMatrix,
  DOMMatrixReadOnly,
  DOMPoint,
  DOMPointReadOnly,
  DOMQuad,
  DOMRect,
  DOMRectReadOnly,
  op_geometry_get_enable_window_features,
  op_geometry_matrix_to_buffer,
  op_geometry_matrix_to_string,
  op_geometry_set_matrix_value,
} from "ext:core/ops";
const {
  ArrayPrototypeJoin,
  Float32Array,
  Float64Array,
  ObjectDefineProperties,
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
ObjectDefineProperty(
  DOMPointReadOnlyPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
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
  },
);

const DOMRectPrototype = DOMRect.prototype;
const DOMRectReadOnlyPrototype = DOMRectReadOnly.prototype;
ObjectDefineProperty(
  DOMRectReadOnlyPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function customInspect(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(
            DOMRectReadOnlyPrototype,
            this,
          ),
          keys: ["x", "y", "width", "height", "top", "right", "bottom", "left"],
        }),
        inspectOptions,
      );
    },
    enumerable: false,
    writable: true,
    configurable: true,
  },
);

const DOMQuadPrototype = DOMQuad.prototype;
ObjectDefineProperty(DOMQuadPrototype, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value: function customInspect(inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(DOMQuadPrototype, this),
        keys: ["p1", "p2", "p3", "p4"],
      }),
      inspectOptions,
    );
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

const DOMMatrixReadOnlyPrototype = DOMMatrixReadOnly.prototype;
ObjectDefineProperties(DOMMatrixReadOnlyPrototype, {
  toFloat32Array: {
    __proto__: null,
    value: function toFloat32Array() {
      return new Float32Array(
        new Float64Array(op_geometry_matrix_to_buffer(this)),
      );
    },
    enumerable: false,
    writable: true,
    configurable: true,
  },
  toFloat64Array: {
    __proto__: null,
    value: function toFloat64Array() {
      return new Float64Array(op_geometry_matrix_to_buffer(this));
    },
    enumerable: false,
    writable: true,
    configurable: true,
  },
  [SymbolFor("Deno.privateCustomInspect")]: {
    __proto__: null,
    value: function customInspect(inspect, inspectOptions) {
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
    },
    enumerable: false,
    writable: true,
    configurable: true,
  },
});

const DOMMatrixPrototype = DOMMatrix.prototype;

if (op_geometry_get_enable_window_features()) {
  // https://drafts.fxtf.org/geometry/#dommatrixreadonly-stringification-behavior
  ObjectDefineProperty(DOMMatrixReadOnlyPrototype, "toString", {
    __proto__: null,
    value: function toString() {
      return op_geometry_matrix_to_string(this);
    },
    writable: true,
    enumerable: true,
    configurable: true,
  });

  // https://drafts.fxtf.org/geometry/#dom-dommatrix-setmatrixvalue
  ObjectDefineProperty(DOMMatrixPrototype, "setMatrixValue", {
    __proto__: null,
    value: function setMatrixValue(transformList) {
      op_geometry_set_matrix_value(this, transformList);
    },
    writable: true,
    enumerable: true,
    configurable: true,
  });
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
