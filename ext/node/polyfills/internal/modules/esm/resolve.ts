// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  op_require_path_resolve,
  op_require_stat,
} = core.ops;

const { ObjectFreeze, ObjectPrototypeIsPrototypeOf } = primordials;

const assert = core.loadExtScript("ext:deno_node/internal/assert.mjs");
const {
  ERR_ACCESS_DENIED,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_URL,
  ERR_MODULE_NOT_FOUND,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { isURL } = core.loadExtScript("ext:deno_node/internal/url.ts");
const lazyUrl = core.createLazyLoader("node:url");
const lazyPath = core.createLazyLoader("node:path");

// Extensions tried during legacy main resolution. Mirrors Node's
// `node_file.cc` `legacy_main_extensions` array.
//
// 0-6: appended to `path.resolve(pkgPath, main)` - used when `main` is set.
// 7-9: appended to `path.resolve(pkgPath, "./index")` - package fallback,
//      used when `main` is unset or its candidates all miss.
const legacyMainExtensions = ObjectFreeze([
  "",
  ".js",
  ".json",
  ".node",
  "/index.js",
  "/index.json",
  "/index.node",
  ".js",
  ".json",
  ".node",
]);

const kResolvedByMainIndexNodeEnd = 7; // exclusive
const kResolvedByPackageFallbackEnd = 10; // exclusive

function fileExistsOrAccessDenied(path: string): boolean {
  // op_require_stat returns 0 if file, 1 if directory, -1 if not found. When
  // Deno's permission system denies the read, the op throws a NotCapable
  // error which we translate to Node's ERR_ACCESS_DENIED with `resource` set
  // to the namespaced path - matching what Node's permission-aware
  // `legacyMainResolve` raises.
  try {
    return op_require_stat(path) === 0;
  } catch (err) {
    if (
      ObjectPrototypeIsPrototypeOf(core.NotCapable.prototype, err)
    ) {
      const resource = lazyPath().toNamespacedPath(path);
      throw new ERR_ACCESS_DENIED(
        `FileSystemRead in "${path}"`,
        "fs-read",
        resource,
      );
    }
    throw err;
  }
}

function pathResolve(...parts: string[]): string {
  return op_require_path_resolve(parts);
}

/**
 * Legacy CommonJS main resolution mirroring Node's
 * `internal/modules/esm/resolve` `legacyMainResolve`:
 *   1. let M = pkg_url + (json main field)
 *   2. TRY(M, M.js, M.json, M.node)
 *   3. TRY(M/index.js, M/index.json, M/index.node)
 *   4. TRY(pkg_url/index.js, pkg_url/index.json, pkg_url/index.node)
 *   5. NOT_FOUND
 */
function legacyMainResolve(
  packageJSONUrl: URL,
  packageConfig: { main?: string | null | undefined },
  base: string | URL | undefined,
): URL {
  assert(isURL(packageJSONUrl));
  const url = lazyUrl();
  const pkgPath = url.fileURLToPath(new URL(".", packageJSONUrl));

  const baseStringified = isURL(base) ? base.href : base;

  const main = packageConfig.main;
  let resolvedPath: string | undefined;
  let packageInitialFile: string | undefined;

  if (typeof main === "string") {
    const initialFilePath = pathResolve(pkgPath, main);
    packageInitialFile = initialFilePath;
    for (let i = 0; i < kResolvedByMainIndexNodeEnd; i++) {
      const filePath = initialFilePath + legacyMainExtensions[i];
      if (fileExistsOrAccessDenied(filePath)) {
        resolvedPath = filePath;
        break;
      }
    }
  }

  if (resolvedPath === undefined) {
    const initialFilePath = pathResolve(pkgPath, "./index");
    if (packageInitialFile === undefined) {
      packageInitialFile = initialFilePath + ".js";
    }
    for (
      let i = kResolvedByMainIndexNodeEnd;
      i < kResolvedByPackageFallbackEnd;
      i++
    ) {
      const filePath = initialFilePath + legacyMainExtensions[i];
      if (fileExistsOrAccessDenied(filePath)) {
        resolvedPath = filePath;
        break;
      }
    }
  }

  if (resolvedPath === undefined) {
    // Validation mirrors Node's `node_file.cc` LegacyMainResolve:
    //   - `base` must be a string (or URL whose href we already stringified).
    //   - That string must parse as a valid URL (Node parses with ada).
    if (typeof baseStringified !== "string") {
      throw new ERR_INVALID_ARG_TYPE("base", ["string", "URL"], base);
    }
    let moduleBase: string;
    try {
      moduleBase = url.fileURLToPath(new URL(baseStringified));
    } catch {
      throw new ERR_INVALID_URL(baseStringified);
    }
    throw new ERR_MODULE_NOT_FOUND(packageInitialFile!, moduleBase);
  }

  return url.pathToFileURL(resolvedPath);
}

return {
  legacyMainResolve,
};
})();
