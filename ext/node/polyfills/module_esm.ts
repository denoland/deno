// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

/**
 * NOTE(bartlomieju):
 * Functionality of this file is ported in Rust in `cli/compat/esm_resolver.ts`.
 * Unfortunately we have no way to call ESM resolution in Rust from TypeScript code.
 */

import {
  fileURLToPath,
  pathToFileURL,
} from "internal:deno_node/polyfills/url.ts";
import {
  ERR_INVALID_MODULE_SPECIFIER,
  ERR_INVALID_PACKAGE_CONFIG,
  ERR_INVALID_PACKAGE_TARGET,
  ERR_MODULE_NOT_FOUND,
  ERR_PACKAGE_IMPORT_NOT_DEFINED,
  ERR_PACKAGE_PATH_NOT_EXPORTED,
  NodeError,
} from "internal:deno_node/polyfills/internal/errors.ts";

const { hasOwn } = Object;

export const encodedSepRegEx = /%2F|%2C/i;

function throwInvalidSubpath(
  subpath: string,
  packageJSONUrl: string,
  internal: boolean,
  base: string,
) {
  const reason = `request is not a valid subpath for the "${
    internal ? "imports" : "exports"
  }" resolution of ${fileURLToPath(packageJSONUrl)}`;
  throw new ERR_INVALID_MODULE_SPECIFIER(
    subpath,
    reason,
    base && fileURLToPath(base),
  );
}

function throwInvalidPackageTarget(
  subpath: string,
  // deno-lint-ignore no-explicit-any
  target: any,
  packageJSONUrl: string,
  internal: boolean,
  base: string,
) {
  if (typeof target === "object" && target !== null) {
    target = JSON.stringify(target, null, "");
  } else {
    target = `${target}`;
  }
  throw new ERR_INVALID_PACKAGE_TARGET(
    fileURLToPath(new URL(".", packageJSONUrl)),
    subpath,
    target,
    internal,
    base && fileURLToPath(base),
  );
}

function throwImportNotDefined(
  specifier: string,
  packageJSONUrl: URL | undefined,
  base: string | URL,
): TypeError & { code: string } {
  throw new ERR_PACKAGE_IMPORT_NOT_DEFINED(
    specifier,
    packageJSONUrl && fileURLToPath(new URL(".", packageJSONUrl)),
    fileURLToPath(base),
  );
}

function throwExportsNotFound(
  subpath: string,
  packageJSONUrl: string,
  base?: string,
): Error & { code: string } {
  throw new ERR_PACKAGE_PATH_NOT_EXPORTED(
    subpath,
    fileURLToPath(new URL(".", packageJSONUrl)),
    base && fileURLToPath(base),
  );
}

function patternKeyCompare(a: string, b: string): number {
  const aPatternIndex = a.indexOf("*");
  const bPatternIndex = b.indexOf("*");
  const baseLenA = aPatternIndex === -1 ? a.length : aPatternIndex + 1;
  const baseLenB = bPatternIndex === -1 ? b.length : bPatternIndex + 1;
  if (baseLenA > baseLenB) return -1;
  if (baseLenB > baseLenA) return 1;
  if (aPatternIndex === -1) return 1;
  if (bPatternIndex === -1) return -1;
  if (a.length > b.length) return -1;
  if (b.length > a.length) return 1;
  return 0;
}

function fileExists(url: string | URL): boolean {
  try {
    const info = Deno.statSync(url);
    return info.isFile;
  } catch {
    return false;
  }
}

function tryStatSync(path: string): { isDirectory: boolean } {
  try {
    const info = Deno.statSync(path);
    return { isDirectory: info.isDirectory };
  } catch {
    return { isDirectory: false };
  }
}

/**
 * Legacy CommonJS main resolution:
 * 1. let M = pkg_url + (json main field)
 * 2. TRY(M, M.js, M.json, M.node)
 * 3. TRY(M/index.js, M/index.json, M/index.node)
 * 4. TRY(pkg_url/index.js, pkg_url/index.json, pkg_url/index.node)
 * 5. NOT_FOUND
 */
function legacyMainResolve(
  packageJSONUrl: URL,
  packageConfig: PackageConfig,
  base: string | URL,
): URL {
  let guess;
  if (packageConfig.main !== undefined) {
    // Note: fs check redundances will be handled by Descriptor cache here.
    if (
      fileExists(guess = new URL(`./${packageConfig.main}`, packageJSONUrl))
    ) {
      return guess;
    } else if (
      fileExists(guess = new URL(`./${packageConfig.main}.js`, packageJSONUrl))
    ) {
      // pass
    } else if (
      fileExists(
        guess = new URL(`./${packageConfig.main}.json`, packageJSONUrl),
      )
    ) {
      // pass
    } else if (
      fileExists(
        guess = new URL(`./${packageConfig.main}.node`, packageJSONUrl),
      )
    ) {
      // pass
    } else if (
      fileExists(
        guess = new URL(`./${packageConfig.main}/index.js`, packageJSONUrl),
      )
    ) {
      // pass
    } else if (
      fileExists(
        guess = new URL(`./${packageConfig.main}/index.json`, packageJSONUrl),
      )
    ) {
      // pass
    } else if (
      fileExists(
        guess = new URL(`./${packageConfig.main}/index.node`, packageJSONUrl),
      )
    ) {
      // pass
    } else guess = undefined;
    if (guess) {
      // TODO(bartlomieju):
      // emitLegacyIndexDeprecation(guess, packageJSONUrl, base,
      //                            packageConfig.main);
      return guess;
    }
    // Fallthrough.
  }
  if (fileExists(guess = new URL("./index.js", packageJSONUrl))) {
    // pass
  } // So fs.
  else if (fileExists(guess = new URL("./index.json", packageJSONUrl))) {
    // pass
  } else if (fileExists(guess = new URL("./index.node", packageJSONUrl))) {
    // pass
  } else guess = undefined;
  if (guess) {
    // TODO(bartlomieju):
    // emitLegacyIndexDeprecation(guess, packageJSONUrl, base, packageConfig.main);
    return guess;
  }
  // Not found.
  throw new ERR_MODULE_NOT_FOUND(
    fileURLToPath(new URL(".", packageJSONUrl)),
    fileURLToPath(base),
  );
}

function parsePackageName(
  specifier: string,
  base: string | URL,
): { packageName: string; packageSubpath: string; isScoped: boolean } {
  let separatorIndex = specifier.indexOf("/");
  let validPackageName = true;
  let isScoped = false;
  if (specifier[0] === "@") {
    isScoped = true;
    if (separatorIndex === -1 || specifier.length === 0) {
      validPackageName = false;
    } else {
      separatorIndex = specifier.indexOf("/", separatorIndex + 1);
    }
  }

  const packageName = separatorIndex === -1
    ? specifier
    : specifier.slice(0, separatorIndex);

  // Package name cannot have leading . and cannot have percent-encoding or
  // separators.
  for (let i = 0; i < packageName.length; i++) {
    if (packageName[i] === "%" || packageName[i] === "\\") {
      validPackageName = false;
      break;
    }
  }

  if (!validPackageName) {
    throw new ERR_INVALID_MODULE_SPECIFIER(
      specifier,
      "is not a valid package name",
      fileURLToPath(base),
    );
  }

  const packageSubpath = "." +
    (separatorIndex === -1 ? "" : specifier.slice(separatorIndex));

  return { packageName, packageSubpath, isScoped };
}

function packageResolve(
  specifier: string,
  base: string,
  conditions: Set<string>,
): URL | undefined {
  const { packageName, packageSubpath, isScoped } = parsePackageName(
    specifier,
    base,
  );

  // ResolveSelf
  const packageConfig = getPackageScopeConfig(base);
  if (packageConfig.exists) {
    const packageJSONUrl = pathToFileURL(packageConfig.pjsonPath);
    if (
      packageConfig.name === packageName &&
      packageConfig.exports !== undefined && packageConfig.exports !== null
    ) {
      return packageExportsResolve(
        packageJSONUrl.toString(),
        packageSubpath,
        packageConfig,
        base,
        conditions,
      );
    }
  }

  let packageJSONUrl = new URL(
    "./node_modules/" + packageName + "/package.json",
    base,
  );
  let packageJSONPath = fileURLToPath(packageJSONUrl);
  let lastPath;
  do {
    const stat = tryStatSync(
      packageJSONPath.slice(0, packageJSONPath.length - 13),
    );
    if (!stat.isDirectory) {
      lastPath = packageJSONPath;
      packageJSONUrl = new URL(
        (isScoped ? "../../../../node_modules/" : "../../../node_modules/") +
          packageName + "/package.json",
        packageJSONUrl,
      );
      packageJSONPath = fileURLToPath(packageJSONUrl);
      continue;
    }

    // Package match.
    const packageConfig = getPackageConfig(packageJSONPath, specifier, base);
    if (packageConfig.exports !== undefined && packageConfig.exports !== null) {
      return packageExportsResolve(
        packageJSONUrl.toString(),
        packageSubpath,
        packageConfig,
        base,
        conditions,
      );
    }
    if (packageSubpath === ".") {
      return legacyMainResolve(packageJSONUrl, packageConfig, base);
    }
    return new URL(packageSubpath, packageJSONUrl);
    // Cross-platform root check.
  } while (packageJSONPath.length !== lastPath.length);

  // TODO(bartlomieju): this is false positive
  // deno-lint-ignore no-unreachable
  throw new ERR_MODULE_NOT_FOUND(packageName, fileURLToPath(base));
}

const invalidSegmentRegEx = /(^|\\|\/)(\.\.?|node_modules)(\\|\/|$)/;
const patternRegEx = /\*/g;

function resolvePackageTargetString(
  target: string,
  subpath: string,
  match: string,
  packageJSONUrl: string,
  base: string,
  pattern: boolean,
  internal: boolean,
  conditions: Set<string>,
): URL | undefined {
  if (subpath !== "" && !pattern && target[target.length - 1] !== "/") {
    throwInvalidPackageTarget(match, target, packageJSONUrl, internal, base);
  }

  if (!target.startsWith("./")) {
    if (
      internal && !target.startsWith("../") &&
      !target.startsWith("/")
    ) {
      let isURL = false;
      try {
        new URL(target);
        isURL = true;
      } catch {
        // pass
      }
      if (!isURL) {
        const exportTarget = pattern
          ? target.replace(patternRegEx, () => subpath)
          : target + subpath;
        return packageResolve(exportTarget, packageJSONUrl, conditions);
      }
    }
    throwInvalidPackageTarget(match, target, packageJSONUrl, internal, base);
  }

  if (invalidSegmentRegEx.test(target.slice(2))) {
    throwInvalidPackageTarget(match, target, packageJSONUrl, internal, base);
  }

  const resolved = new URL(target, packageJSONUrl);
  const resolvedPath = resolved.pathname;
  const packagePath = new URL(".", packageJSONUrl).pathname;

  if (!resolvedPath.startsWith(packagePath)) {
    throwInvalidPackageTarget(match, target, packageJSONUrl, internal, base);
  }

  if (subpath === "") return resolved;

  if (invalidSegmentRegEx.test(subpath)) {
    const request = pattern
      ? match.replace("*", () => subpath)
      : match + subpath;
    throwInvalidSubpath(request, packageJSONUrl, internal, base);
  }

  if (pattern) {
    return new URL(resolved.href.replace(patternRegEx, () => subpath));
  }
  return new URL(subpath, resolved);
}

function isArrayIndex(key: string): boolean {
  const keyNum = +key;
  if (`${keyNum}` !== key) return false;
  return keyNum >= 0 && keyNum < 0xFFFF_FFFF;
}

function resolvePackageTarget(
  packageJSONUrl: string,
  // deno-lint-ignore no-explicit-any
  target: any,
  subpath: string,
  packageSubpath: string,
  base: string,
  pattern: boolean,
  internal: boolean,
  conditions: Set<string>,
): URL | undefined {
  if (typeof target === "string") {
    return resolvePackageTargetString(
      target,
      subpath,
      packageSubpath,
      packageJSONUrl,
      base,
      pattern,
      internal,
      conditions,
    );
  } else if (Array.isArray(target)) {
    if (target.length === 0) {
      return undefined;
    }

    let lastException;
    for (let i = 0; i < target.length; i++) {
      const targetItem = target[i];
      let resolved;
      try {
        resolved = resolvePackageTarget(
          packageJSONUrl,
          targetItem,
          subpath,
          packageSubpath,
          base,
          pattern,
          internal,
          conditions,
        );
      } catch (e: unknown) {
        lastException = e;
        if (e instanceof NodeError && e.code === "ERR_INVALID_PACKAGE_TARGET") {
          continue;
        }
        throw e;
      }
      if (resolved === undefined) {
        continue;
      }
      if (resolved === null) {
        lastException = null;
        continue;
      }
      return resolved;
    }
    if (lastException === undefined || lastException === null) {
      return undefined;
    }
    throw lastException;
  } else if (typeof target === "object" && target !== null) {
    const keys = Object.getOwnPropertyNames(target);
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      if (isArrayIndex(key)) {
        throw new ERR_INVALID_PACKAGE_CONFIG(
          fileURLToPath(packageJSONUrl),
          base,
          '"exports" cannot contain numeric property keys.',
        );
      }
    }
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      if (key === "default" || conditions.has(key)) {
        const conditionalTarget = target[key];
        const resolved = resolvePackageTarget(
          packageJSONUrl,
          conditionalTarget,
          subpath,
          packageSubpath,
          base,
          pattern,
          internal,
          conditions,
        );
        if (resolved === undefined) {
          continue;
        }
        return resolved;
      }
    }
    return undefined;
  } else if (target === null) {
    return undefined;
  }
  throwInvalidPackageTarget(
    packageSubpath,
    target,
    packageJSONUrl,
    internal,
    base,
  );
}

export function packageExportsResolve(
  packageJSONUrl: string,
  packageSubpath: string,
  packageConfig: PackageConfig,
  base: string,
  conditions: Set<string>,
  // @ts-ignore `URL` needs to be forced due to control flow
): URL {
  let exports = packageConfig.exports;
  if (isConditionalExportsMainSugar(exports, packageJSONUrl, base)) {
    exports = { ".": exports };
  }

  if (
    hasOwn(exports, packageSubpath) &&
    !packageSubpath.includes("*") &&
    !packageSubpath.endsWith("/")
  ) {
    const target = exports[packageSubpath];
    const resolved = resolvePackageTarget(
      packageJSONUrl,
      target,
      "",
      packageSubpath,
      base,
      false,
      false,
      conditions,
    );
    if (resolved === null || resolved === undefined) {
      throwExportsNotFound(packageSubpath, packageJSONUrl, base);
    }
    return resolved!;
  }

  let bestMatch = "";
  let bestMatchSubpath = "";
  const keys = Object.getOwnPropertyNames(exports);
  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    const patternIndex = key.indexOf("*");
    if (
      patternIndex !== -1 &&
      packageSubpath.startsWith(key.slice(0, patternIndex))
    ) {
      // When this reaches EOL, this can throw at the top of the whole function:
      //
      // if (StringPrototypeEndsWith(packageSubpath, '/'))
      //   throwInvalidSubpath(packageSubpath)
      //
      // To match "imports" and the spec.
      if (packageSubpath.endsWith("/")) {
        // TODO(@bartlomieju):
        // emitTrailingSlashPatternDeprecation(
        //   packageSubpath,
        //   packageJSONUrl,
        //   base,
        // );
      }
      const patternTrailer = key.slice(patternIndex + 1);
      if (
        packageSubpath.length >= key.length &&
        packageSubpath.endsWith(patternTrailer) &&
        patternKeyCompare(bestMatch, key) === 1 &&
        key.lastIndexOf("*") === patternIndex
      ) {
        bestMatch = key;
        bestMatchSubpath = packageSubpath.slice(
          patternIndex,
          packageSubpath.length - patternTrailer.length,
        );
      }
    }
  }

  if (bestMatch) {
    const target = exports[bestMatch];
    const resolved = resolvePackageTarget(
      packageJSONUrl,
      target,
      bestMatchSubpath,
      bestMatch,
      base,
      true,
      false,
      conditions,
    );
    if (resolved === null || resolved === undefined) {
      throwExportsNotFound(packageSubpath, packageJSONUrl, base);
    }
    return resolved!;
  }

  throwExportsNotFound(packageSubpath, packageJSONUrl, base);
}

export interface PackageConfig {
  pjsonPath: string;
  exists: boolean;
  name?: string;
  main?: string;
  // deno-lint-ignore no-explicit-any
  exports?: any;
  // deno-lint-ignore no-explicit-any
  imports?: any;
  type?: string;
}

const packageJSONCache = new Map(); /* string -> PackageConfig */

function getPackageConfig(
  path: string,
  specifier: string | URL,
  base?: string | URL,
): PackageConfig {
  const existing = packageJSONCache.get(path);
  if (existing !== undefined) {
    return existing;
  }

  let source: string | undefined;
  try {
    source = new TextDecoder().decode(
      Deno.readFileSync(path),
    );
  } catch {
    // pass
  }

  if (source === undefined) {
    const packageConfig = {
      pjsonPath: path,
      exists: false,
      main: undefined,
      name: undefined,
      type: "none",
      exports: undefined,
      imports: undefined,
    };
    packageJSONCache.set(path, packageConfig);
    return packageConfig;
  }

  let packageJSON;
  try {
    packageJSON = JSON.parse(source);
  } catch (error) {
    throw new ERR_INVALID_PACKAGE_CONFIG(
      path,
      (base ? `"${specifier}" from ` : "") + fileURLToPath(base || specifier),
      // @ts-ignore there's no assertion for type and `error` is thus `unknown`
      error.message,
    );
  }

  let { imports, main, name, type } = packageJSON;
  const { exports } = packageJSON;
  if (typeof imports !== "object" || imports === null) imports = undefined;
  if (typeof main !== "string") main = undefined;
  if (typeof name !== "string") name = undefined;
  // Ignore unknown types for forwards compatibility
  if (type !== "module" && type !== "commonjs") type = "none";

  const packageConfig = {
    pjsonPath: path,
    exists: true,
    main,
    name,
    type,
    exports,
    imports,
  };
  packageJSONCache.set(path, packageConfig);
  return packageConfig;
}

function getPackageScopeConfig(resolved: URL | string): PackageConfig {
  let packageJSONUrl = new URL("./package.json", resolved);
  while (true) {
    const packageJSONPath = packageJSONUrl.pathname;
    if (packageJSONPath.endsWith("node_modules/package.json")) {
      break;
    }
    const packageConfig = getPackageConfig(
      fileURLToPath(packageJSONUrl),
      resolved,
    );
    if (packageConfig.exists) return packageConfig;

    const lastPackageJSONUrl = packageJSONUrl;
    packageJSONUrl = new URL("../package.json", packageJSONUrl);

    // Terminates at root where ../package.json equals ../../package.json
    // (can't just check "/package.json" for Windows support).
    if (packageJSONUrl.pathname === lastPackageJSONUrl.pathname) break;
  }
  const packageJSONPath = fileURLToPath(packageJSONUrl);
  const packageConfig = {
    pjsonPath: packageJSONPath,
    exists: false,
    main: undefined,
    name: undefined,
    type: "none",
    exports: undefined,
    imports: undefined,
  };
  packageJSONCache.set(packageJSONPath, packageConfig);
  return packageConfig;
}

export function packageImportsResolve(
  name: string,
  base: string,
  conditions: Set<string>,
  // @ts-ignore `URL` needs to be forced due to control flow
): URL {
  if (
    name === "#" || name.startsWith("#/") ||
    name.startsWith("/")
  ) {
    const reason = "is not a valid internal imports specifier name";
    throw new ERR_INVALID_MODULE_SPECIFIER(name, reason, fileURLToPath(base));
  }
  let packageJSONUrl;
  const packageConfig = getPackageScopeConfig(base);
  if (packageConfig.exists) {
    packageJSONUrl = pathToFileURL(packageConfig.pjsonPath);
    const imports = packageConfig.imports;
    if (imports) {
      if (
        hasOwn(imports, name) &&
        !name.includes("*")
      ) {
        const resolved = resolvePackageTarget(
          packageJSONUrl.toString(),
          imports[name],
          "",
          name,
          base,
          false,
          true,
          conditions,
        );
        if (resolved !== null && resolved !== undefined) {
          return resolved;
        }
      } else {
        let bestMatch = "";
        let bestMatchSubpath = "";
        const keys = Object.getOwnPropertyNames(imports);
        for (let i = 0; i < keys.length; i++) {
          const key = keys[i];
          const patternIndex = key.indexOf("*");
          if (
            patternIndex !== -1 &&
            name.startsWith(
              key.slice(0, patternIndex),
            )
          ) {
            const patternTrailer = key.slice(patternIndex + 1);
            if (
              name.length >= key.length &&
              name.endsWith(patternTrailer) &&
              patternKeyCompare(bestMatch, key) === 1 &&
              key.lastIndexOf("*") === patternIndex
            ) {
              bestMatch = key;
              bestMatchSubpath = name.slice(
                patternIndex,
                name.length - patternTrailer.length,
              );
            }
          }
        }

        if (bestMatch) {
          const target = imports[bestMatch];
          const resolved = resolvePackageTarget(
            packageJSONUrl.toString(),
            target,
            bestMatchSubpath,
            bestMatch,
            base,
            true,
            true,
            conditions,
          );
          if (resolved !== null && resolved !== undefined) {
            return resolved;
          }
        }
      }
    }
  }
  throwImportNotDefined(name, packageJSONUrl, base);
}

function isConditionalExportsMainSugar(
  // deno-lint-ignore no-explicit-any
  exports: any,
  packageJSONUrl: string,
  base: string,
): boolean {
  if (typeof exports === "string" || Array.isArray(exports)) return true;
  if (typeof exports !== "object" || exports === null) return false;

  const keys = Object.getOwnPropertyNames(exports);
  let isConditionalSugar = false;
  let i = 0;
  for (let j = 0; j < keys.length; j++) {
    const key = keys[j];
    const curIsConditionalSugar = key === "" || key[0] !== ".";
    if (i++ === 0) {
      isConditionalSugar = curIsConditionalSugar;
    } else if (isConditionalSugar !== curIsConditionalSugar) {
      const message =
        "\"exports\" cannot contain some keys starting with '.' and some not." +
        " The exports object must either be an object of package subpath keys" +
        " or an object of main entry condition name keys only.";
      throw new ERR_INVALID_PACKAGE_CONFIG(
        fileURLToPath(packageJSONUrl),
        base,
        message,
      );
    }
  }
  return isConditionalSugar;
}
