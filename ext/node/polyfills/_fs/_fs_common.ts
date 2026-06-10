// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayPrototypeSlice,
  ArrayPrototypeUnshift,
  PromisePrototypeThen,
  ReflectApply,
} = primordials;
const { validateFunction } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);

const lazyFsUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/utils.mjs",
);

function isFileOptions(
  fileOptions,
) {
  if (!fileOptions) return false;

  return (
    fileOptions.encoding != undefined ||
    fileOptions.flag != undefined ||
    fileOptions.signal != undefined ||
    fileOptions.mode != undefined
  );
}

function getValidatedEncoding(
  optOrCallback,
) {
  const encoding = getEncoding(optOrCallback);
  if (encoding) {
    lazyFsUtils().assertEncoding(encoding);
  }
  return encoding;
}

function getEncoding(
  optOrCallback,
) {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  }

  const encoding = typeof optOrCallback === "string"
    ? optOrCallback
    : optOrCallback.encoding;
  if (!encoding) return null;
  return encoding;
}

function getSignal(optOrCallback) {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  }

  const signal = typeof optOrCallback === "object" && optOrCallback.signal
    ? optOrCallback.signal
    : null;

  return signal;
}

const __reexport = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const isFd = __reexport.isUint32;

function maybeCallback(cb) {
  validateFunction(cb, "cb");

  return cb;
}

// Ensure that callbacks run in the global context. Only use this function
// for callbacks that are passed to the binding layer, callbacks that are
// invoked from JS already run in the proper scope.
function makeCallback(cb) {
  validateFunction(cb, "cb");
  // Callbacks run with `this` = undefined, matching Node.js ESM strict-mode
  // behavior (the original code was an ESM arrow function capturing `this`
  // from makeCallback's call site, which is undefined in strict mode).
  return (...args) => ReflectApply(cb, undefined, args);
}

// Wraps a promise-returning op as a node-style callback function: the op runs
// with the leading args (its eager validation throws synchronously, before the
// callback check, like node), then the resolved value is passed as the second
// callback argument. `nargs` is the op's fixed leading-argument count (the
// callback sits at that index, so omitting it still validates the args first).
// Shared by fs.ts and the eager _fs_*.ts wrappers so each `const f =
// callbackify(op, n)` shares one SFI instead of baking a wrapper per function.
function callbackify(op, nargs, defaultCb) {
  return function (...args) {
    const promise = ReflectApply(
      op,
      undefined,
      ArrayPrototypeSlice(args, 0, nargs),
    );
    let callback;
    try {
      // JS default-parameter semantics: only `undefined` falls back to the
      // default callback (e.g. close's defaultCloseCallback); `null` does not.
      const cb = args[nargs] === undefined ? defaultCb : args[nargs];
      callback = makeCallback(cb);
    } catch (e) {
      PromisePrototypeThen(promise, undefined, () => {});
      throw e;
    }
    // These wrappers are all void in node (callback receives only `err`).
    return PromisePrototypeThen(promise, () => callback(null), callback);
  };
}

// Like `callbackify`, but for `f(...leading, ...optional?, cb)` shapes that
// resolve a value: `nLeading` required args followed by any number of optional
// middle args (the op supplies defaults for omitted ones), then the callback
// (the first trailing function). Op-first, so the op's synchronous validation
// runs before the callback is validated -- matching node's "validate the inputs
// before the callback" order. A null/undefined resolution invokes the callback
// with exactly `(null)` -- node's void-op oncomplete arity (test-fs-access
// deepStrictEquals the callback arguments) -- otherwise `(null, result)`.
//
// `cbAtEnd` selects node's positional-callback variant (e.g. `symlink`, whose
// `makeCallback(arguments.length === 3 ? type_ : callback_)` treats whatever
// follows the leading args as the callback by POSITION, so
// `symlink(t, p, "dir")` throws "Received type string ('dir')"). In that mode
// the callback is also validated BEFORE the op runs, like node, so a bad
// callback never starts the I/O.
function callbackifyOpt(op, nLeading = 1, cbAtEnd = false) {
  return function (...args) {
    let cbIdx = args.length;
    if (cbAtEnd) {
      if (args.length > nLeading) {
        cbIdx = args.length - 1;
      }
      const callback = makeCallback(
        cbIdx === args.length ? undefined : args[cbIdx],
      );
      return PromisePrototypeThen(
        ReflectApply(op, undefined, ArrayPrototypeSlice(args, 0, cbIdx)),
        (result) => result == null ? callback(null) : callback(null, result),
        callback,
      );
    }
    for (let i = nLeading; i < args.length; i++) {
      if (typeof args[i] === "function") {
        cbIdx = i;
        break;
      }
    }
    const promise = ReflectApply(
      op,
      undefined,
      ArrayPrototypeSlice(args, 0, cbIdx),
    );
    let callback;
    try {
      callback = makeCallback(cbIdx === args.length ? undefined : args[cbIdx]);
    } catch (e) {
      PromisePrototypeThen(promise, undefined, () => {});
      throw e;
    }
    return PromisePrototypeThen(
      promise,
      (result) => result == null ? callback(null) : callback(null, result),
      callback,
    );
  };
}

// Shared by `fs.write`/`fs.writev`: the op resolves the overload + validates
// synchronously (so bad args throw at the call site like node) and writes
// asynchronously, returning the bytes written. The whole argument list -- the
// callback included -- is forwarded to the op, because `op_node_fs_write_v`
// uses the trailing callback slots to disambiguate the string overload
// (`write(fd, str, position, cb)` vs `write(fd, str, position, encoding, cb)`).
// On completion the callback is invoked exactly like node's `wrapper`:
// `(err, written || 0, buffer)` on both paths, with the original buffer/buffers
// re-attached so it can't be GC'ed too soon.
function callbackifyWrite(op) {
  return function (fd, buffer, ...rest) {
    let cb;
    for (let i = 0; i < rest.length; i++) {
      if (typeof rest[i] === "function") {
        cb = rest[i];
        break;
      }
    }
    cb = maybeCallback(cb);
    // Forward `op(fd, buffer, ...rest)` -- the callback included, since the op
    // uses the trailing slots to disambiguate the string overload.
    ArrayPrototypeUnshift(rest, fd, buffer);
    return PromisePrototypeThen(
      ReflectApply(op, undefined, rest),
      (written) => cb(null, written || 0, buffer),
      (err) => cb(err, 0, buffer),
    );
  };
}

return {
  isFileOptions,
  getValidatedEncoding,
  getEncoding,
  getSignal,
  isFd,
  maybeCallback,
  makeCallback,
  callbackify,
  callbackifyOpt,
  callbackifyWrite,
};
})();
