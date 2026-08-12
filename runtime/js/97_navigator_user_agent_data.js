// Copyright 2018-2026 the Deno authors. MIT license.

// Implements the `NavigatorUAData` interface (the User-Agent Client Hints API)
// exposed via `navigator.userAgentData`. This is shared between the window and
// worker global scopes.
//
// https://wicg.github.io/ua-client-hints/

import { core, primordials } from "ext:core/mod.js";
import { op_bootstrap_user_agent } from "ext:core/ops";
const {
  ArrayPrototypeMap,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  PromiseReject,
  PromiseResolve,
  SafeArrayIterator,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  SymbolFor,
} = primordials;

const console = core.loadExtScript("ext:deno_web/01_console.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");

/**
 * Maps Deno's `core.build.os` value to the OS name reported by
 * `navigator.userAgentData.platform`, mirroring the values browsers return.
 * @param {string} os
 * @returns {string}
 */
function getPlatform(os) {
  switch (os) {
    case "darwin":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    case "android":
      return "Android";
    case "freebsd":
      return "FreeBSD";
    case "openbsd":
      return "OpenBSD";
    case "netbsd":
      return "NetBSD";
    case "solaris":
    case "illumos":
      return "SunOS";
    case "aix":
      return "AIX";
    default:
      return "Unknown";
  }
}

/**
 * Maps Deno's `core.build.arch` value to the architecture reported by the
 * `architecture` high-entropy hint, mirroring the values browsers return.
 * @param {string} arch
 * @returns {string}
 */
function getArchitecture(arch) {
  switch (arch) {
    case "x86_64":
      return "x86";
    case "aarch64":
      return "arm";
    default:
      return arch;
  }
}

// `op_bootstrap_user_agent()` returns a string of the form `Deno/<version>`.
// Derive the version once, lazily.
let fullVersion_ = null;
function fullVersion() {
  if (fullVersion_ === null) {
    const ua = op_bootstrap_user_agent();
    fullVersion_ = StringPrototypeStartsWith(ua, "Deno/")
      ? StringPrototypeSlice(ua, "Deno/".length)
      : ua;
  }
  return fullVersion_;
}

// The major version, used for the low-entropy `brands` list.
function majorVersion() {
  const version = fullVersion();
  const dot = StringPrototypeIndexOf(version, ".");
  return dot === -1 ? version : StringPrototypeSlice(version, 0, dot);
}

function lowEntropyBrands() {
  return [{ brand: "Deno", version: majorVersion() }];
}

function fullVersionList() {
  return [{ brand: "Deno", version: fullVersion() }];
}

const highEntropyValues = {
  architecture: () => getArchitecture(core.build.arch),
  bitness: () => "64",
  brands: () => lowEntropyBrands(),
  formFactors: () => ["Desktop"],
  fullVersionList: () => fullVersionList(),
  mobile: () => false,
  model: () => "",
  platform: () => getPlatform(core.build.os),
  platformVersion: () => "",
  uaFullVersion: () => fullVersion(),
  wow64: () => false,
};

class NavigatorUAData {
  constructor() {
    webidl.illegalConstructor();
  }

  get brands() {
    webidl.assertBranded(this, NavigatorUADataPrototype);
    // Return fresh frozen records so callers can't mutate shared state.
    return ArrayPrototypeMap(
      lowEntropyBrands(),
      (b) => ObjectFreeze({ brand: b.brand, version: b.version }),
    );
  }

  get mobile() {
    webidl.assertBranded(this, NavigatorUADataPrototype);
    return false;
  }

  get platform() {
    webidl.assertBranded(this, NavigatorUADataPrototype);
    return getPlatform(core.build.os);
  }

  getHighEntropyValues(hints) {
    // This operation returns a promise, so per WebIDL any error raised while
    // validating/converting arguments must be turned into a rejected promise
    // rather than thrown synchronously.
    try {
      webidl.assertBranded(this, NavigatorUADataPrototype);
      const prefix =
        "Failed to execute 'getHighEntropyValues' on 'NavigatorUAData'";
      webidl.requiredArguments(arguments.length, 1, prefix);
      hints = webidl.converters["sequence<DOMString>"](
        hints,
        prefix,
        "Argument 1",
      );

      // The low-entropy values are always included.
      const result = {
        brands: this.brands,
        mobile: false,
        platform: getPlatform(core.build.os),
      };

      for (const hint of new SafeArrayIterator(hints)) {
        const getter = highEntropyValues[hint];
        if (getter !== undefined) {
          result[hint] = getter();
        }
      }

      return PromiseResolve(result);
    } catch (err) {
      return PromiseReject(err);
    }
  }

  toJSON() {
    webidl.assertBranded(this, NavigatorUADataPrototype);
    return {
      brands: this.brands,
      mobile: false,
      platform: getPlatform(core.build.os),
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(NavigatorUADataPrototype, this),
        keys: [
          "brands",
          "mobile",
          "platform",
        ],
      }),
      inspectOptions,
    );
  }
}
const NavigatorUADataPrototype = NavigatorUAData.prototype;

const navigatorUAData = webidl.createBranded(NavigatorUAData);

export { NavigatorUAData, navigatorUAData };
