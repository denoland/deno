// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  DOMMatrix,
  DOMMatrixReadOnly,
  DOMPoint,
  DOMPointReadOnly,
  DOMQuad,
  DOMRect,
  DOMRectReadOnly,
  op_geometry_get_enable_css_parser_features,
  op_geometry_matrix_set_matrix_value,
  op_geometry_matrix_to_string,
} = core.ops;
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

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

const geometryCloneTypes = [
  [DOMPointReadOnlyPrototype, "DOMPointReadOnly", DOMPointReadOnly],
  [DOMPointPrototype, "DOMPoint", DOMPoint],
  [DOMRectReadOnlyPrototype, "DOMRectReadOnly", DOMRectReadOnly],
  [DOMRectPrototype, "DOMRect", DOMRect],
  [DOMQuadPrototype, "DOMQuad", DOMQuad],
  [DOMMatrixReadOnlyPrototype, "DOMMatrixReadOnly", DOMMatrixReadOnly],
  [DOMMatrixPrototype, "DOMMatrix", DOMMatrix],
];

for (let i = 0; i < geometryCloneTypes.length; i++) {
  const [prototype, type, Constructor] = geometryCloneTypes[i];
  const toJSON = prototype.toJSON;
  ObjectDefineProperty(prototype, core.hostObjectBrand, {
    __proto__: null,
    value: function () {
      return { type, value: ReflectApply(toJSON, this, []) };
    },
  });
  core.registerCloneableResource(type, (data) => {
    const value = data.value;
    if (type === "DOMPoint" || type === "DOMPointReadOnly") {
      return new Constructor(value.x, value.y, value.z, value.w);
    }
    if (type === "DOMRect" || type === "DOMRectReadOnly") {
      return new Constructor(value.x, value.y, value.width, value.height);
    }
    if (type === "DOMQuad") {
      return new DOMQuad(
        new DOMPoint(value.p1.x, value.p1.y, value.p1.z, value.p1.w),
        new DOMPoint(value.p2.x, value.p2.y, value.p2.z, value.p2.w),
        new DOMPoint(value.p3.x, value.p3.y, value.p3.z, value.p3.w),
        new DOMPoint(value.p4.x, value.p4.y, value.p4.z, value.p4.w),
      );
    }
    return new Constructor(
      value.is2D
        ? [
          value.a,
          value.b,
          value.c,
          value.d,
          value.e,
          value.f,
        ]
        : [
          value.m11,
          value.m12,
          value.m13,
          value.m14,
          value.m21,
          value.m22,
          value.m23,
          value.m24,
          value.m31,
          value.m32,
          value.m33,
          value.m34,
          value.m41,
          value.m42,
          value.m43,
          value.m44,
        ],
    );
  });
}

if (op_geometry_get_enable_css_parser_features()) {
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

return {
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
})();
