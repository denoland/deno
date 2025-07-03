// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ObjectFreeze,
  ReflectApply,
  RegExpPrototypeTest,
  SafeRegExp,
  SafeSet,
  SetPrototypeEntries,
  SetPrototypeValues,
  StringPrototypeReplace,
  Symbol,
  SymbolIterator,
} = primordials;

const kInternal = Symbol("internal properties");

const replaceUnderscoresRegex = new SafeRegExp(/_/g);
const leadingDashesRegex = new SafeRegExp(/^--?/);
const trailingValuesRegex = new SafeRegExp(/=.*$/);

// This builds the initial process.allowedNodeEnvironmentFlags
// from data in the config binding.
export function buildAllowedFlags() {
  const allowedNodeEnvironmentFlags = [
    "--track-heap-objects",
    "--no-track-heap-objects",
    "--node-snapshot",
    "--no-node-snapshot",
    "--require",
    "--max-old-space-size",
    "--trace-exit",
    "--no-trace-exit",
    "--disallow-code-generation-from-strings",
    "--experimental-json-modules",
    "--no-experimental-json-modules",
    "--interpreted-frames-native-stack",
    "--inspect-brk",
    "--no-inspect-brk",
    "--trace-tls",
    "--no-trace-tls",
    "--stack-trace-limit",
    "--experimental-repl-await",
    "--no-experimental-repl-await",
    "--preserve-symlinks",
    "--no-preserve-symlinks",
    "--report-uncaught-exception",
    "--no-report-uncaught-exception",
    "--experimental-modules",
    "--no-experimental-modules",
    "--report-signal",
    "--jitless",
    "--inspect-port",
    "--heapsnapshot-near-heap-limit",
    "--tls-keylog",
    "--force-context-aware",
    "--no-force-context-aware",
    "--napi-modules",
    "--abort-on-uncaught-exception",
    "--diagnostic-dir",
    "--verify-base-objects",
    "--no-verify-base-objects",
    "--unhandled-rejections",
    "--perf-basic-prof",
    "--trace-atomics-wait",
    "--no-trace-atomics-wait",
    "--deprecation",
    "--no-deprecation",
    "--perf-basic-prof-only-functions",
    "--perf-prof",
    "--max-http-header-size",
    "--report-on-signal",
    "--no-report-on-signal",
    "--throw-deprecation",
    "--no-throw-deprecation",
    "--warnings",
    "--no-warnings",
    "--force-fips",
    "--no-force-fips",
    "--pending-deprecation",
    "--no-pending-deprecation",
    "--input-type",
    "--tls-max-v1.3",
    "--no-tls-max-v1.3",
    "--tls-min-v1.2",
    "--no-tls-min-v1.2",
    "--inspect",
    "--no-inspect",
    "--heapsnapshot-signal",
    "--trace-warnings",
    "--no-trace-warnings",
    "--trace-event-categories",
    "--experimental-worker",
    "--tls-max-v1.2",
    "--no-tls-max-v1.2",
    "--perf-prof-unwinding-info",
    "--preserve-symlinks-main",
    "--no-preserve-symlinks-main",
    "--policy-integrity",
    "--experimental-wasm-modules",
    "--no-experimental-wasm-modules",
    "--node-memory-debug",
    "--inspect-publish-uid",
    "--tls-min-v1.3",
    "--no-tls-min-v1.3",
    "--experimental-specifier-resolution",
    "--secure-heap",
    "--tls-min-v1.0",
    "--no-tls-min-v1.0",
    "--redirect-warnings",
    "--experimental-report",
    "--trace-event-file-pattern",
    "--trace-uncaught",
    "--no-trace-uncaught",
    "--experimental-loader",
    "--http-parser",
    "--dns-result-order",
    "--trace-sigint",
    "--no-trace-sigint",
    "--secure-heap-min",
    "--enable-fips",
    "--no-enable-fips",
    "--enable-source-maps",
    "--no-enable-source-maps",
    "--insecure-http-parser",
    "--no-insecure-http-parser",
    "--use-openssl-ca",
    "--no-use-openssl-ca",
    "--tls-cipher-list",
    "--experimental-top-level-await",
    "--no-experimental-top-level-await",
    "--openssl-config",
    "--icu-data-dir",
    "--v8-pool-size",
    "--report-on-fatalerror",
    "--no-report-on-fatalerror",
    "--title",
    "--tls-min-v1.1",
    "--no-tls-min-v1.1",
    "--report-filename",
    "--trace-deprecation",
    "--no-trace-deprecation",
    "--report-compact",
    "--no-report-compact",
    "--experimental-policy",
    "--experimental-import-meta-resolve",
    "--no-experimental-import-meta-resolve",
    "--zero-fill-buffers",
    "--no-zero-fill-buffers",
    "--report-dir",
    "--use-bundled-ca",
    "--no-use-bundled-ca",
    "--experimental-vm-modules",
    "--no-experimental-vm-modules",
    "--force-async-hooks-checks",
    "--no-force-async-hooks-checks",
    "--frozen-intrinsics",
    "--no-frozen-intrinsics",
    "--huge-max-old-generation-size",
    "--disable-proto",
    "--debug-arraybuffer-allocations",
    "--no-debug-arraybuffer-allocations",
    "--conditions",
    "--experimental-wasi-unstable-preview1",
    "--no-experimental-wasi-unstable-preview1",
    "--trace-sync-io",
    "--no-trace-sync-io",
    "--use-largepages",
    "--experimental-abortcontroller",
    "--debug-port",
    "--es-module-specifier-resolution",
    "--prof-process",
    "-C",
    "--loader",
    "--report-directory",
    "-r",
    "--trace-events-enabled",
  ];

  /*
  function isAccepted(to) {
    if (!to.startsWith("-") || to === "--") return true;
    const recursiveExpansion = aliases.get(to);
    if (recursiveExpansion) {
      if (recursiveExpansion[0] === to) {
        recursiveExpansion.splice(0, 1);
      }
      return recursiveExpansion.every(isAccepted);
    }
    return options.get(to).envVarSettings === kAllowedInEnvironment;
  }
  for (const { 0: from, 1: expansion } of aliases) {
    if (expansion.every(isAccepted)) {
      let canonical = from;
      if (canonical.endsWith("=")) {
        canonical = canonical.slice(0, canonical.length - 1);
      }
      if (canonical.endsWith(" <arg>")) {
        canonical = canonical.slice(0, canonical.length - 4);
      }
      allowedNodeEnvironmentFlags.push(canonical);
    }
  }
  */

  const trimLeadingDashes = (flag) =>
    StringPrototypeReplace(flag, leadingDashesRegex, "");

  // Save these for comparison against flags provided to
  // process.allowedNodeEnvironmentFlags.has() which lack leading dashes.
  const nodeFlags = ArrayPrototypeMap(
    allowedNodeEnvironmentFlags,
    trimLeadingDashes,
  );

  // Ignoring primordial lint. Extending from SafeSet prevents augmenting `keys`
  // and [SymbolIterator] instance methods.
  // deno-lint-ignore prefer-primordials
  class NodeEnvironmentFlagsSet extends Set {
    constructor(array) {
      super();
      this[kInternal] = { array };
    }

    add() {
      // No-op, `Set` API compatible
      return this;
    }

    delete() {
      // No-op, `Set` API compatible
      return false;
    }

    clear() {
      // No-op, `Set` API compatible
    }

    has(key) {
      // This will return `true` based on various possible
      // permutations of a flag, including present/missing leading
      // dash(es) and/or underscores-for-dashes.
      // Strips any values after `=`, inclusive.
      // TODO(addaleax): It might be more flexible to run the option parser
      // on a dummy option set and see whether it rejects the argument or
      // not.
      if (typeof key === "string") {
        key = StringPrototypeReplace(key, replaceUnderscoresRegex, "-");
        if (RegExpPrototypeTest(leadingDashesRegex, key)) {
          key = StringPrototypeReplace(key, trailingValuesRegex, "");
          return ArrayPrototypeIncludes(this[kInternal].array, key);
        }
        return ArrayPrototypeIncludes(nodeFlags, key);
      }
      return false;
    }

    entries() {
      this[kInternal].set ??= new SafeSet(this[kInternal].array);
      return SetPrototypeEntries(this[kInternal].set);
    }

    forEach(callback, thisArg = undefined) {
      ArrayPrototypeForEach(
        this[kInternal].array,
        (v) => ReflectApply(callback, thisArg, [v, v, this]),
      );
    }

    get size() {
      return this[kInternal].array.length;
    }

    values() {
      this[kInternal].set ??= new SafeSet(this[kInternal].array);
      return SetPrototypeValues(this[kInternal].set);
    }
  }
  NodeEnvironmentFlagsSet.prototype.keys =
    NodeEnvironmentFlagsSet
      .prototype[SymbolIterator] =
      NodeEnvironmentFlagsSet.prototype.values;

  ObjectFreeze(NodeEnvironmentFlagsSet.prototype.constructor);
  ObjectFreeze(NodeEnvironmentFlagsSet.prototype);

  return ObjectFreeze(
    new NodeEnvironmentFlagsSet(
      allowedNodeEnvironmentFlags,
    ),
  );
}
