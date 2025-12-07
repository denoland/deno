// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_bootstrap_language,
  op_bootstrap_numcpus,
  op_bootstrap_user_agent,
} from "ext:core/ops";
const {
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  StringPrototypeSlice,
  StringPrototypeToUpperCase,
  SymbolFor,
} = primordials;

import * as location from "ext:deno_web/12_location.js";
import * as console from "ext:deno_web/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import { loadWebGPU } from "ext:deno_webgpu/00_init.js";

/**
 * @param {string} arch
 * @param {string} platform
 * @returns {string}
 */
function getNavigatorPlatform(arch, platform) {
  switch (platform) {
    case "darwin":
      // On macOS, modern browsers return 'MacIntel' even if running on Apple Silicon.
      return "MacIntel";

    case "windows":
      // On Windows, modern browsers return 'Win32' even if running on a 64-bit version of Windows.
      // https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#usage_notes
      return "Win32";

    case "linux":
      return `Linux ${arch}`;

    case "freebsd":
      if (arch === "x86_64") {
        return "FreeBSD amd64";
      }
      return `FreeBSD ${arch}`;

    case "solaris":
      return `SunOS ${arch}`;

    case "aix":
      return "AIX";

    default:
      return `${StringPrototypeToUpperCase(platform[0])}${
        StringPrototypeSlice(platform, 1)
      } ${arch}`;
  }
}

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
const platform = memoizeLazy(() =>
  getNavigatorPlatform(core.build.arch, core.build.os)
);

class WorkerNavigator {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WorkerNavigatorPrototype, this),
        keys: [
          "hardwareConcurrency",
          "userAgent",
          "language",
          "languages",
          "platform",
        ],
      }),
      inspectOptions,
    );
  }
}

const workerNavigator = webidl.createBranded(WorkerNavigator);

ObjectDefineProperties(WorkerNavigator.prototype, {
  gpu: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      const webgpu = loadWebGPU();
      webgpu.initGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return language();
    },
  },
  languages: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return [language()];
    },
  },
  platform: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return platform();
    },
  },
});
const WorkerNavigatorPrototype = WorkerNavigator.prototype;

const workerRuntimeGlobalProperties = {
  WorkerLocation: location.workerLocationConstructorDescriptor,
  location: location.workerLocationDescriptor,
  WorkerGlobalScope: globalInterfaces.workerGlobalScopeConstructorDescriptor,
  DedicatedWorkerGlobalScope:
    globalInterfaces.dedicatedWorkerGlobalScopeConstructorDescriptor,
  WorkerNavigator: core.propNonEnumerable(WorkerNavigator),
  navigator: core.propGetterOnly(() => workerNavigator),
  self: core.propGetterOnly(() => globalThis),
};

export { workerRuntimeGlobalProperties };
