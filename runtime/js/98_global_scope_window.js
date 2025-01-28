// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_bootstrap_language,
  op_bootstrap_numcpus,
  op_bootstrap_user_agent,
} from "ext:core/ops";
const {
  ArrayPrototypeMap,
  ArrayPrototypeSome,
  Float64Array,
  Number,
  NumberIsNaN,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  SafeRegExp,
  StringPrototypeMatch,
  StringPrototypeSplit,
  SymbolFor,
  TypeError,
} = primordials;

import * as location from "ext:deno_web/12_location.js";
import * as console from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as webStorage from "ext:deno_webstorage/01_webstorage.js";
import * as prompt from "ext:runtime/41_prompt.js";
import { loadWebGPU } from "ext:deno_webgpu/00_init.js";
import { createGeometryLoader } from "ext:deno_geometry/00_init.js";

const MATRIX_PATTERN = new SafeRegExp(
  /^\s*matrix(3d)?\(([-\+0-9.e,\s]+)\)\s*$/iu,
);

const loadGeometry = createGeometryLoader((transformList, prefix) => {
  if (transformList === "") {
    return {
      // deno-fmt-ignore
      matrix: new Float64Array([
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
      ]),
      is2D: true,
    };
  }

  // Currently only parsing of a single matrix, matrix3d function without units
  // as arguments is implemented
  // TODO(petamoriken): Add CSS parser such as lightningcss to support more cases
  const matrixMatch = StringPrototypeMatch(transformList, MATRIX_PATTERN);
  if (matrixMatch !== null) {
    const is2D = matrixMatch[1] === undefined;
    /** @type {number[]} */
    const seq = ArrayPrototypeMap(
      StringPrototypeSplit(matrixMatch[2], ","),
      (str) => Number(str),
    );
    if (
      is2D && seq.length !== 6 ||
      !is2D && seq.length !== 16 ||
      ArrayPrototypeSome(seq, (num) => NumberIsNaN(num))
    ) {
      throw new DOMException(
        `${prefix}: Failed to parse '${transformList}'`,
        "SyntaxError",
      );
    }
    if (is2D) {
      const { 0: a, 1: b, 2: c, 3: d, 4: e, 5: f } = seq;
      return {
        // deno-fmt-ignore
        matrix: new Float64Array([
          a, b, 0, 0,
          c, d, 0, 0,
          0, 0, 1, 0,
          e, f, 0, 1,
        ]),
        is2D,
      };
    } else {
      return {
        matrix: new Float64Array(seq),
        is2D,
      };
    }
  }

  throw new TypeError(
    `${prefix}: CSS <transform-list> parser is not fully implemented`,
  );
}, true);

class Navigator {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(NavigatorPrototype, this),
        keys: [
          "hardwareConcurrency",
          "userAgent",
          "language",
          "languages",
        ],
      }),
      inspectOptions,
    );
  }
}

const navigator = webidl.createBranded(Navigator);

function memoizeLazy(f) {
  let v_ = null;
  return () => {
    if (v_ === null) {
      v_ = f();
    }
    return v_;
  };
}

const numCpus = memoizeLazy(() => op_bootstrap_numcpus());
const userAgent = memoizeLazy(() => op_bootstrap_user_agent());
const language = memoizeLazy(() => op_bootstrap_language());

ObjectDefineProperties(Navigator.prototype, {
  gpu: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      const webgpu = loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return language();
    },
  },
  languages: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return [language()];
    },
  },
});
const NavigatorPrototype = Navigator.prototype;

const mainRuntimeGlobalProperties = {
  Location: location.locationConstructorDescriptor,
  location: location.locationDescriptor,
  Window: globalInterfaces.windowConstructorDescriptor,
  self: core.propGetterOnly(() => globalThis),
  Navigator: core.propNonEnumerable(Navigator),
  navigator: core.propGetterOnly(() => navigator),
  alert: core.propWritable(prompt.alert),
  confirm: core.propWritable(prompt.confirm),
  prompt: core.propWritable(prompt.prompt),
  localStorage: core.propGetterOnly(webStorage.localStorage),
  sessionStorage: core.propGetterOnly(webStorage.sessionStorage),
  Storage: core.propNonEnumerable(webStorage.Storage),
  DOMMatrix: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrix,
    loadGeometry,
  ),
  DOMMatrixReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrixReadOnly,
    loadGeometry,
  ),
  DOMPoint: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPoint,
    loadGeometry,
  ),
  DOMPointReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPointReadOnly,
    loadGeometry,
  ),
  DOMQuad: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMQuad,
    loadGeometry,
  ),
  DOMRect: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRect,
    loadGeometry,
  ),
  DOMRectReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRectReadOnly,
    loadGeometry,
  ),
};

export { mainRuntimeGlobalProperties, memoizeLazy };
