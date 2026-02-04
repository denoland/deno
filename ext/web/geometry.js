// Copyright 2018-2026 the Deno authors. MIT license.

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
  op_geometry_matrix_set_matrix_value,
  op_geometry_matrix_to_string,
} from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_web/01_console.js";

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
webidl.configureInterface(DOMPoint);
webidl.configureInterface(DOMPointReadOnly);

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
webidl.configureInterface(DOMRect);
webidl.configureInterface(DOMRectReadOnly);

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
webidl.configureInterface(DOMQuad);

const DOMMatrixPrototype = DOMMatrix.prototype;
const DOMMatrixReadOnlyPrototype = DOMMatrixReadOnly.prototype;
ObjectDefineProperty(
  DOMMatrixReadOnlyPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
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
);

if (op_geometry_get_enable_window_features()) {
  // https://drafts.csswg.org/geometry/#dommatrixreadonly-stringification-behavior
  ObjectDefineProperty(DOMMatrixReadOnlyPrototype, "toString", {
    __proto__: null,
    value: function toString() {
      return op_geometry_matrix_to_string(this);
    },
    writable: true,
    enumerable: true,
    configurable: true,
  });

  // https://drafts.csswg.org/geometry/#dom-dommatrix-setmatrixvalue
  ObjectDefineProperty(DOMMatrixPrototype, "setMatrixValue", {
    __proto__: null,
    value: function setMatrixValue(transformList) {
      const prefix = "Failed to execute 'setMatrixValue' on 'DOMMatrix'";
      webidl.requiredArguments(arguments.length, 1, prefix);
      op_geometry_matrix_set_matrix_value(this, transformList);
      return this;
    },
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

webidl.configureInterface(DOMMatrix);
webidl.configureInterface(DOMMatrixReadOnly);

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
