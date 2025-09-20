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
  SymbolFor,
  StringPrototypeToUpperCase,
} = primordials;

import * as location from "ext:deno_web/12_location.js";
import * as console from "ext:deno_console/01_console.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as webStorage from "ext:deno_webstorage/01_webstorage.js";
import * as prompt from "ext:runtime/41_prompt.js";
import { loadWebGPU } from "ext:deno_webgpu/00_init.js";
import process from "node:process";

/**
 * @param {string} arch
 * @param {string} platform
 * @returns {string}
 */
function getNavigatorPlatform(arch, platform) {
  if (platform === "darwin") {
    // On macOS, modern browsers return 'MacIntel' even if running on Apple Silicon.
    return "MacIntel";
  } else if (platform === "win32") {
    // On Windows, modern browsers return 'Win32' even if running on a 64-bit version of Windows.
    // https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform#usage_notes
    return "Win32";
  } else if (platform === "linux") {
    if (arch === "ia32") {
      return "Linux i686";
    } else if (arch === "x64") {
      return "Linux x86_64";
    }
    return `Linux ${arch}`;
  } else if (platform === "freebsd") {
    if (arch === "ia32") {
      return "FreeBSD i386";
    } else if (arch === "x64") {
      return "FreeBSD amd64";
    }
    return `FreeBSD ${arch}`;
  } else if (platform === "openbsd") {
    if (arch === "ia32") {
      return "OpenBSD i386";
    } else if (arch === "x64") {
      return "OpenBSD amd64";
    }
    return `OpenBSD ${arch}`;
  } else if (platform === "sunos") {
    if (arch === "ia32") {
      return "SunOS i86pc";
    }
    return `SunOS ${arch}`;
  } else if (platform === "aix") {
    return "AIX";
  }
  return `${StringPrototypeToUpperCase(platform[0])}${
    StringPrototypeSlice(platform, 1)
  } ${arch}`;
}

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
          "platform",
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
      webgpu.initGPU();
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
  platform: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return [getNavigatorPlatform(process.arch, process.platform)];
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
};

export { mainRuntimeGlobalProperties, memoizeLazy };
