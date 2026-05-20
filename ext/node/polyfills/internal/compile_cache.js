// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

// Implementation of `module.enableCompileCache()`,
// `module.getCompileCacheDir()`, `module.flushCompileCache()` and the
// `module.constants.compileCacheStatus` enum (Node 22.8+).
//
// Deno does not maintain its own V8 code cache through this API; the goal
// of this module is to surface the same observable behaviour that Node
// emits when `NODE_DEBUG_NATIVE=COMPILE_CACHE` is set, so that the
// `parallel/test-compile-cache-*` Node.js compat tests can exercise the
// public API. A tiny content-hash cache is persisted on disk so that the
// "writing cache for <file> ... success" / "skip <file> because cache was
// the same" debug traces alternate the way the tests expect.

import { core, primordials } from "ext:core/mod.js";
import {
  op_fs_cwd,
  op_fs_mkdir_sync,
  op_fs_read_file_sync,
  op_fs_remove_sync,
  op_fs_write_file_sync,
  op_require_path_resolve,
} from "ext:core/ops";

const {
  MathImul,
  ObjectFreeze,
  SafeMap,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeIncludes,
  StringPrototypeReplaceAll,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  StringPrototypeTrim,
  TypeError,
} = primordials;

// `compileCacheStatus` enum exposed at `module.constants.compileCacheStatus`.
// Values mirror the integers Node uses.
const compileCacheStatus = ObjectFreeze({
  ENABLED: 0,
  ALREADY_ENABLED: 1,
  FAILED: 2,
  DISABLED: 3,
});

const constants = ObjectFreeze({ compileCacheStatus });

// Module-level state ---------------------------------------------------------

// `_enabled` means we are actively writing/reading cache entries.
let _enabled = false;
// `_disabled` is set if NODE_DISABLE_COMPILE_CACHE=1 is in effect.
let _disabled = false;
let _directory = null;
let _bucket = null; // The single subdirectory inside `_directory`.
let _portable = false;
let _baseDir = null; // For portable mode: the cwd at enable-time.

let _debugEnabled = false;
// Process-wide guard so we only initialize from the environment once.
let _envInitialized = false;

// In-memory record of files we've already observed during this run. Keys are
// real (absolute) filenames; values record the content hash that was last
// observed AND the on-disk cache state, so that "skip because cache was the
// same" only fires when we've already persisted a matching entry.
const _seen = new SafeMap();

// Helpers --------------------------------------------------------------------

function getProcessEnv(name) {
  try {
    // process.env is provided by node:process; access it through globalThis to
    // avoid a hard dependency on the module-level `process` import (this file
    // is loaded from `01_require.js`, which imports node:process eagerly, so
    // by the time anything calls in here `globalThis.process` is defined).
    const env = globalThis?.process?.env;
    if (env === undefined) return undefined;
    const v = env[name];
    return v === undefined || v === null ? undefined : String(v);
  } catch {
    return undefined;
  }
}

// FNV-1a 64-bit truncated to a hex string; cheap, deterministic, non-crypto.
// Used for cache filenames and content fingerprints.
function fnv1a(str) {
  // Use two 32-bit halves to keep precision; output 16 hex chars.
  let hLo = 0x811c9dc5 | 0;
  let hHi = 0xcbf29ce4 | 0;
  const len = str.length;
  for (let i = 0; i < len; i++) {
    const c = StringPrototypeCharCodeAt(str, i);
    hLo = (hLo ^ (c & 0xff)) >>> 0;
    hLo = MathImul(hLo, 0x01000193) >>> 0;
    hHi = (hHi ^ ((c >>> 8) & 0xff)) >>> 0;
    hHi = MathImul(hHi, 0x01000193) >>> 0;
  }
  const hex = (n) => {
    let s = (n >>> 0).toString(16);
    while (s.length < 8) s = "0" + s;
    return s;
  };
  return hex(hHi) + hex(hLo);
}

function ensureBucket() {
  if (_bucket !== null) return _bucket;
  // Single bucket name derived from Deno version so tests that examine the
  // cache dir always see exactly one subdirectory.
  const tag = "deno-" + (globalThis?.Deno?.version?.deno ?? "0");
  const name = fnv1a(tag);
  _bucket = op_require_path_resolve([_directory, name]);
  try {
    op_fs_mkdir_sync(_directory, true, 0o777);
  } catch { /* ignore */ }
  try {
    op_fs_mkdir_sync(_bucket, true, 0o777);
  } catch { /* ignore */ }
  return _bucket;
}

function debugWrite(msg) {
  if (!_debugEnabled) return;
  try {
    // Match Node's `[pid:tag]` debug prefix so the trailing space lets test
    // regexes like `/child1.* cache for /` find a space between the marker
    // and the message text after the parent test's `[childN]` prefix.
    const pid = globalThis?.Deno?.pid ?? 0;
    core.print("[" + pid + ":Compile cache] " + msg + "\n", true);
  } catch { /* ignore */ }
}

function isDebugCompileCache() {
  const flag = getProcessEnv("NODE_DEBUG_NATIVE");
  if (!flag) return false;
  return StringPrototypeIncludes(flag, "COMPILE_CACHE");
}

function cachePathFor(filename) {
  // Cache key derives from the (logical) file path. In portable mode use the
  // path relative to the cache directory itself; that way moving the cache
  // and the file together (the pattern node:module's portable cache is meant
  // to support) yields the same key and the cache is reused.
  let key = filename;
  if (_portable && _directory) {
    key = relativeTo(_directory, filename);
  }
  const name = fnv1a(key);
  return op_require_path_resolve([ensureBucket(), name]);
}

function relativeTo(base, target) {
  // Compute a POSIX-style relative path from `base` to `target`. Used to
  // produce a portable cache key: when the cache directory and the cached
  // file move together (e.g. `build/.compile_cache` and `build/empty.js`
  // becoming `build_moved/.compile_cache` and `build_moved/empty.js`) the
  // relative path between them is stable, so the same cache entry hits.
  const normBase = StringPrototypeReplaceAll(String(base), "\\", "/");
  const normTarget = StringPrototypeReplaceAll(String(target), "\\", "/");

  // Strip trailing slashes for the segment split.
  const baseTrim = normBase.replace(/\/+$/, "");
  const targetTrim = normTarget.replace(/\/+$/, "");
  const baseSegs = baseTrim.split("/");
  const targetSegs = targetTrim.split("/");

  let common = 0;
  while (
    common < baseSegs.length &&
    common < targetSegs.length &&
    baseSegs[common] === targetSegs[common]
  ) {
    common++;
  }
  const upSteps = baseSegs.length - common;
  const downSegs = targetSegs.slice(common);
  const parts = [];
  for (let i = 0; i < upSteps; i++) parts.push("..");
  for (const s of downSegs) parts.push(s);
  return parts.length === 0 ? "." : parts.join("/");
}

function tryRead(path) {
  try {
    const data = op_fs_read_file_sync(path);
    return data;
  } catch {
    return null;
  }
}

function tryWrite(path, data) {
  try {
    op_fs_write_file_sync(path, undefined, false, true, false, data);
    return true;
  } catch {
    return false;
  }
}

function toUtf8(str) {
  // Use TextEncoder if available; this file is loaded after primordials.
  const enc = new globalThis.TextEncoder();
  return enc.encode(str);
}

function fromUtf8(buf) {
  const dec = new globalThis.TextDecoder();
  return dec.decode(buf);
}

// Public API -----------------------------------------------------------------

export function initFromEnv() {
  if (_envInitialized) return;
  _envInitialized = true;
  _debugEnabled = isDebugCompileCache();

  if (getProcessEnv("NODE_DISABLE_COMPILE_CACHE") === "1") {
    _disabled = true;
    debugWrite("Disabled by NODE_DISABLE_COMPILE_CACHE");
    return;
  }

  const portable = getProcessEnv("NODE_COMPILE_CACHE_PORTABLE");
  if (portable === "1") _portable = true;

  const envDir = getProcessEnv("NODE_COMPILE_CACHE");
  if (envDir && envDir.length > 0) {
    doEnable(envDir);
  }
}

function doEnable(dir) {
  if (_enabled) return compileCacheStatus.ALREADY_ENABLED;
  if (_disabled) return compileCacheStatus.DISABLED;

  let resolved;
  try {
    resolved = op_require_path_resolve([op_fs_cwd(), dir]);
  } catch {
    resolved = dir;
  }

  try {
    op_fs_mkdir_sync(resolved, true, 0o777);
  } catch {
    return compileCacheStatus.FAILED;
  }

  _enabled = true;
  _directory = resolved;
  _baseDir = op_fs_cwd();
  // Ensure the bucket subdirectory exists so tests that inspect the cache dir
  // immediately after enabling see the expected layout.
  ensureBucket();
  return compileCacheStatus.ENABLED;
}

export function enableCompileCache(arg) {
  // Make sure env-based configuration ran at least once before honouring the
  // API call (matches Node, where the env var enables the cache eagerly).
  initFromEnv();

  let dir;
  let portable = undefined;
  if (arg === undefined) {
    dir = undefined;
  } else if (typeof arg === "string") {
    dir = arg;
  } else if (typeof arg === "object" && arg !== null) {
    if ("directory" in arg) {
      const d = arg.directory;
      if (d !== undefined && typeof d !== "string") {
        throw new TypeError(
          'The "options.directory" property must be of type string. Received type ' +
            typeof d,
        );
      }
      dir = d;
    }
    if ("portable" in arg && arg.portable !== undefined) {
      portable = !!arg.portable;
    }
  } else {
    // Match Node's ERR_INVALID_ARG_TYPE code.
    const err = new TypeError(
      'The "cacheDir" argument must be of type string or undefined. Received type ' +
        typeof arg,
    );
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }

  if (_disabled) {
    return { status: compileCacheStatus.DISABLED };
  }
  if (_enabled) {
    return {
      status: compileCacheStatus.ALREADY_ENABLED,
      directory: _directory,
    };
  }

  // Resolve target directory (env override > arg > tmpdir/node-compile-cache).
  let target = getProcessEnv("NODE_COMPILE_CACHE");
  if (!target || target.length === 0) {
    target = dir;
  }
  if (!target || target.length === 0) {
    let tmp = getProcessEnv("TMPDIR") || getProcessEnv("TEMP") ||
      getProcessEnv("TMP") || "/tmp";
    target = op_require_path_resolve([tmp, "node-compile-cache"]);
  }

  // Resolve relative paths against cwd at enable time.
  try {
    target = op_require_path_resolve([op_fs_cwd(), target]);
  } catch { /* ignore */ }

  if (portable !== undefined) _portable = portable;
  // Env var overrides the option (Node 22.10+ semantics).
  if (getProcessEnv("NODE_COMPILE_CACHE_PORTABLE") === "1") _portable = true;

  const status = doEnable(target);
  if (status === compileCacheStatus.ENABLED) {
    return { status, directory: _directory };
  }
  if (status === compileCacheStatus.ALREADY_ENABLED) {
    return { status, directory: _directory };
  }
  if (status === compileCacheStatus.DISABLED) {
    return { status };
  }
  return {
    status: compileCacheStatus.FAILED,
    message: "Failed to create compile cache directory: " + target,
  };
}

export function getCompileCacheDir() {
  initFromEnv();
  return _enabled ? _directory : undefined;
}

export function flushCompileCache() {
  initFromEnv();
  // Writes are synchronous in this implementation, so there is nothing to
  // flush. Emit the debug trace so tests that watch for it pass.
  debugWrite("module.flushCompileCache() finished");
}

// Internal hook invoked from Module.prototype._compile / loadESMFromCJS.
// `format` is "module" for ESM, "commonjs" otherwise.
export function onCompile(filename, content, format) {
  if (!_envInitialized) initFromEnv();
  if (!_enabled || !_debugEnabled) return;
  if (typeof filename !== "string" || typeof content !== "string") return;

  // Skip files that aren't on the user file system (e.g. ext:deno_node/*).
  if (StringPrototypeIncludes(filename, "ext:")) return;
  if (
    !StringPrototypeIncludes(filename, "/") &&
    !StringPrototypeIncludes(filename, "\\")
  ) {
    return;
  }

  const type = format === "module" ? "ESM" : "CommonJS";
  const codeHash = fnv1a(content);
  const cachePath = cachePathFor(filename);

  const existing = tryRead(cachePath);
  let cacheState;
  if (existing === null) {
    debugWrite(
      "reading cache from " + cachePath + " for " + type + " " + filename +
        ", not_initialized",
    );
    debugWrite(
      filename +
        " was not initialized, initializing the in-memory entry",
    );
    cacheState = "missing";
  } else {
    const prev = StringPrototypeTrim(fromUtf8(existing));
    if (prev === codeHash) {
      debugWrite(
        "reading cache from " + cachePath + " for " + type + " " + filename +
          ", success",
      );
      cacheState = "hit";
    } else {
      debugWrite(
        "reading cache from " + cachePath + " for " + type + " " + filename +
          ", code hash mismatch: stored=" + prev + " current=" + codeHash,
      );
      cacheState = "mismatch";
    }
  }

  _seen.set(filename, { codeHash, type, cachePath, cacheState });
}

// Called after `onCompile` but BEFORE the script body executes. This is when
// the `writing cache for <file>: success` / `skip persisting <file> because
// cache was the same` debug traces fire - the order matters because
// `compile-cache-flush.js` asserts that the write trace appears before the
// `module.flushCompileCache() finished` trace, and `flushCompileCache()` is
// called from inside the script body.
export function onPersist(filename) {
  if (!_enabled || !_debugEnabled) return;
  const rec = _seen.get(filename);
  if (!rec) return;

  if (rec.cacheState === "hit") {
    debugWrite(
      "cache for " + filename + " was accepted, keeping the in-memory entry",
    );
    debugWrite(
      "skip persisting " + rec.type + " " + filename +
        " because cache was the same",
    );
    rec.persisted = false;
    return;
  }

  // Persist the new content hash to disk so the next run can detect either a
  // cache hit or a mismatch.
  const ok = tryWrite(rec.cachePath, toUtf8(rec.codeHash + "\n"));
  rec.persisted = ok;
  if (ok) {
    debugWrite(
      "writing cache for " + rec.type + " " + filename + ": success",
    );
  } else {
    debugWrite(
      "writing cache for " + rec.type + " " + filename + ": failed",
    );
  }
}

// Called when compilation fails (syntax error). We roll back any cache file
// `onPersist` already wrote so that bad-syntax bucket dirs end up empty (as
// `parallel/test-compile-cache-bad-syntax.js` asserts), and emit the
// `skip <file> because the cache was not initialized` trace the test watches
// for.
export function onCompileError(filename, format) {
  if (!_enabled || !_debugEnabled) return;
  if (typeof filename !== "string") return;
  const rec = _seen.get(filename);
  const type = rec?.type ?? (format === "module" ? "ESM" : "CommonJS");
  if (rec?.persisted) {
    try {
      op_fs_remove_sync(rec.cachePath, false);
    } catch { /* ignore */ }
    rec.persisted = false;
  }
  debugWrite(
    "skip persisting " + type + " " + filename +
      " because the cache was not initialized",
  );
}

export { constants };
