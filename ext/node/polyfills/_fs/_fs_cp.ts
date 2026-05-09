// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;

const lazyFsUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/utils.mjs",
);
const { cpFn, throwCpError: _throwCpError } = core.loadExtScript(
  "ext:deno_node/_fs/cp/cp.ts",
);
const { cpSyncFn } = core.loadExtScript("ext:deno_node/_fs/cp/cp_sync.ts");
const { makeCallback } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_common.ts",
);

const { PromisePrototypeThen } = primordials;

function cpSync(
  src,
  dest,
  options,
) {
  options = lazyFsUtils().validateCpOptions(options);
  const srcPath = lazyFsUtils().getValidatedPathToString(src, "src");
  const destPath = lazyFsUtils().getValidatedPathToString(dest, "dest");

  cpSyncFn(srcPath, destPath, options);
}

function cp(
  src,
  dest,
  options,
  callback,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  callback = makeCallback(callback);
  options = lazyFsUtils().validateCpOptions(options);
  const srcPath = lazyFsUtils().getValidatedPathToString(src, "src");
  const destPath = lazyFsUtils().getValidatedPathToString(dest, "dest");

  PromisePrototypeThen(
    cpFn(srcPath, destPath, options),
    () => callback(null),
    callback,
  );
}

async function cpPromise(
  src,
  dest,
  options,
) {
  options = lazyFsUtils().validateCpOptions(options);
  const srcPath = lazyFsUtils().getValidatedPathToString(src, "src");
  const destPath = lazyFsUtils().getValidatedPathToString(dest, "dest");
  return await cpFn(srcPath, destPath, options);
}

return { cpSync, cp, cpPromise };
})();
