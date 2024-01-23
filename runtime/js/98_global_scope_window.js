// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  op_bootstrap_language,
  op_bootstrap_numcpus,
  op_bootstrap_user_agent,
} = core.ensureFastOps(true);
const {
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

import * as util from "ext:runtime/06_util.js";
import * as location from "ext:deno_web/12_location.js";
import * as console from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as webStorage from "ext:deno_webstorage/01_webstorage.js";
import * as prompt from "ext:runtime/41_prompt.js";
import { loadWebGPU, webgpu } from "ext:deno_webgpu/00_init.js";

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
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return language();
    },
  },
  languages: {
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
  window: util.getterOnly(() => {
    internals.warnOnDeprecatedApi(
      "window",
      new Error().stack,
      "Use `globalThis` or `self` instead.",
      "You can provide `window` in the current scope, using `const window = globalThis`.",
    );
    globalThis;
  }),
  self: util.getterOnly(() => globalThis),
  Navigator: util.nonEnumerable(Navigator),
  navigator: util.getterOnly(() => navigator),
  alert: util.writable(prompt.alert),
  confirm: util.writable(prompt.confirm),
  prompt: util.writable(prompt.prompt),
  localStorage: util.getterOnly(webStorage.localStorage),
  sessionStorage: util.getterOnly(webStorage.sessionStorage),
  Storage: util.nonEnumerable(webStorage.Storage),
};

export { mainRuntimeGlobalProperties, memoizeLazy };
