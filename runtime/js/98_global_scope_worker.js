// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
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

function memoizeLazy(f) {
  let v_ = null;
  return () => {
    if (v_ === null) {
      v_ = f();
    }
    return v_;
  };
}

const numCpus = memoizeLazy(() => ops.op_bootstrap_numcpus());
const userAgent = memoizeLazy(() => ops.op_bootstrap_user_agent());
const language = memoizeLazy(() => ops.op_bootstrap_language());

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
        ],
      }),
      inspectOptions,
    );
  }
}

const workerNavigator = webidl.createBranded(WorkerNavigator);

ObjectDefineProperties(WorkerNavigator.prototype, {
  gpu: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return language();
    },
  },
  languages: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return [language()];
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
  WorkerNavigator: util.nonEnumerable(WorkerNavigator),
  navigator: util.getterOnly(() => workerNavigator),
  self: util.getterOnly(() => globalThis),
};

export { workerRuntimeGlobalProperties };
