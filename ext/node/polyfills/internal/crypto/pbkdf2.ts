// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core } = globalThis.__bootstrap;
const {
  op_node_pbkdf2,
  op_node_pbkdf2_async,
  op_node_pbkdf2_validate,
} = core.ops;

const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  validateFunction,
  validateString,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { getArrayBufferOrView } = core.loadExtScript(
  "ext:deno_node/internal/crypto/keys.ts",
);
const {
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_OUT_OF_RANGE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  emitAfter,
  emitBefore,
  emitDestroy,
  emitInit,
  executionAsyncId,
  newAsyncId,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");

const lazyProcess = core.createLazyLoader("node:process");

const MAX_ALLOC = Math.pow(2, 30) - 1;
const MAX_I32 = 2 ** 31 - 1;

function check(
  password: any,
  salt: any,
  iterations: number,
  keylen: number,
  digest: string,
) {
  validateString(digest, "digest");
  password = getArrayBufferOrView(password, "password", "buffer");
  salt = getArrayBufferOrView(salt, "salt", "buffer");
  validateUint32(iterations, "iterations", true);
  validateUint32(keylen, "keylen");

  if (iterations > MAX_I32) {
    throw new ERR_OUT_OF_RANGE("iterations", `<= ${MAX_I32}`, iterations);
  }

  if (keylen > MAX_I32) {
    throw new ERR_OUT_OF_RANGE("keylen", `<= ${MAX_I32}`, keylen);
  }

  return { password, salt, iterations, keylen, digest };
}

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
function pbkdf2Sync(
  password: any,
  salt: any,
  iterations: number,
  keylen: number,
  digest: string,
): Buffer {
  ({ password, salt, iterations, keylen, digest } = check(
    password,
    salt,
    iterations,
    keylen,
    digest,
  ));

  digest = digest.toLowerCase();

  const DK = new Uint8Array(keylen);
  if (!op_node_pbkdf2(password, salt, iterations, digest, DK)) {
    throw new ERR_CRYPTO_INVALID_DIGEST(digest);
  }

  return Buffer.from(DK);
}

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
function pbkdf2(
  password: any,
  salt: any,
  iterations: number,
  keylen: number,
  digest: string,
  callback: (err: Error | null, derivedKey?: Buffer) => void,
) {
  if (typeof digest === "function") {
    callback = digest;
    digest = undefined as unknown as string;
  }

  ({ password, salt, iterations, keylen, digest } = check(
    password,
    salt,
    iterations,
    keylen,
    digest,
  ));

  validateFunction(callback, "callback");

  digest = digest.toLowerCase();
  op_node_pbkdf2_validate(digest);

  // Set up async_hooks tracking
  const asyncId = newAsyncId();
  const triggerAsyncId = executionAsyncId();
  const resource = {};
  emitInit(asyncId, "PBKDF2REQUEST", triggerAsyncId, resource);

  // Track if callback was already invoked (to avoid calling twice if callback throws)
  let callbackInvoked = false;

  const process = lazyProcess().default;

  op_node_pbkdf2_async(
    password,
    salt,
    iterations,
    digest,
    keylen,
  ).then(
    (DK) => {
      callbackInvoked = true;
      emitBefore(asyncId);
      try {
        callback(null, Buffer.from(DK));
      } catch (err) {
        // If there's an active domain, emit error to it
        if (process.domain && process.domain.listenerCount("error") > 0) {
          process.domain.emit("error", err);
        } else {
          throw err;
        }
      } finally {
        emitAfter(asyncId);
        emitDestroy(asyncId);
      }
    },
  )
    .catch((err) => {
      // Don't call callback again if error was thrown by the callback itself
      if (callbackInvoked) {
        // If there's an active domain, emit error to it
        if (process.domain && process.domain.listenerCount("error") > 0) {
          process.domain.emit("error", err);
        } else {
          throw err;
        }
        return;
      }
      emitBefore(asyncId);
      try {
        callback(err);
      } catch (callbackErr) {
        // If there's an active domain, emit error to it
        if (process.domain && process.domain.listenerCount("error") > 0) {
          process.domain.emit("error", callbackErr);
        } else {
          throw callbackErr;
        }
      } finally {
        emitAfter(asyncId);
        emitDestroy(asyncId);
      }
    });
}

return {
  MAX_ALLOC,
  MAX_I32,
  pbkdf2,
  pbkdf2Sync,
  default: {
    MAX_ALLOC,
    pbkdf2,
    pbkdf2Sync,
  },
};
})();
