// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { denoErrorToNodeError } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

const lazyStatUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/stat_utils.ts",
);
const lazyFsUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/utils.mjs",
);
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");

const {
  Error,
  PromisePrototypeThen,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

function lstat(
  path,
  optionsOrCallback,
  maybeCallback,
) {
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : { bigint: false };

  if (!callback) throw new Error("No callback function supplied");

  // Match Node: errors carry the requested path (see lib/fs.js lstat).
  const validatedPath = lazyFsUtils().getValidatedPathToString(path);
  PromisePrototypeThen(
    Deno.lstat(validatedPath),
    (stat) => callback(null, lazyStatUtils().CFISBIS(stat, options.bigint)),
    (err) => {
      // Match Node: `{ throwIfNoEntry: false }` suppresses ENOENT and yields
      // undefined stats (see lib/fs.js lstat()).
      if (
        options?.throwIfNoEntry === false &&
        ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
      ) {
        callback(null, undefined);
        return;
      }
      callback(
        denoErrorToNodeError(err, {
          syscall: "lstat",
          path: validatedPath,
        }),
      );
    },
  );
}

const lstatPromise = promisify(lstat);

function lstatSync(
  path,
  options,
) {
  const validatedPath = lazyFsUtils().getValidatedPathToString(path);
  try {
    const origin = Deno.lstatSync(validatedPath);
    return lazyStatUtils().CFISBIS(origin, options?.bigint || false);
  } catch (err) {
    if (
      options?.throwIfNoEntry === false &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    throw denoErrorToNodeError(err, {
      syscall: "lstat",
      path: validatedPath,
    });
  }
}

return { lstat, lstatPromise, lstatSync };
})();
