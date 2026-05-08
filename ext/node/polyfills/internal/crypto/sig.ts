// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core, primordials } = globalThis.__bootstrap;

const {
  SymbolSpecies,
} = primordials;

const {
  op_node_create_private_key,
  op_node_create_public_key,
  op_node_derive_public_key_from_private_key,
  op_node_get_asymmetric_key_details,
  op_node_get_asymmetric_key_type,
  op_node_sign,
  op_node_sign_ed25519,
  op_node_sign_ed448,
  op_node_verify,
  op_node_verify_ed25519,
  op_node_verify_ed448,
} = core.ops;

const {
  validateFunction,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

const lazyWritable = core.createLazyLoader("node:_stream_writable");

const {
  kConsumePrivate,
  kConsumePublic,
  KeyObject,
  prepareAsymmetricKey,
  PrivateKeyObject,
  PublicKeyObject,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const { createHash } = core.loadExtScript(
  "ext:deno_node/internal/crypto/hash.ts",
);
const {
  ERR_CRYPTO_SIGN_KEY_REQUIRED,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const FastBuffer = Buffer[SymbolSpecies];

function getPadding(options) {
  return getIntOption("padding", options);
}

function getSaltLength(options) {
  return getIntOption("saltLength", options);
}

function getDSASignatureEncoding(options) {
  if (typeof options === "object") {
    const { dsaEncoding = "der" } = options;
    if (dsaEncoding === "der") {
      return 0;
    } else if (dsaEncoding === "ieee-p1363") {
      return 1;
    }
    throw new ERR_INVALID_ARG_VALUE("options.dsaEncoding", dsaEncoding);
  }

  return 0;
}

function getIntOption(name, options) {
  const value = options[name];
  if (value !== undefined) {
    if (value === value >> 0) {
      return value;
    }
    throw new ERR_INVALID_ARG_VALUE(`options.${name}`, value);
  }
  return undefined;
}

// Private key types that need to be converted to public keys for verification
const PRIVATE_KEY_TYPES = ["pkcs8", "sec1"];

function isPrivateKeyType(type: string | undefined): boolean {
  return type !== undefined && PRIVATE_KEY_TYPES.includes(type);
}

function isPrivateKeyPem(data: ArrayBuffer | ArrayBufferView): boolean {
  const bytes = data instanceof ArrayBuffer
    ? new Uint8Array(data, 0, Math.min(data.byteLength, 100))
    : new Uint8Array(
      (data as ArrayBufferView).buffer,
      (data as ArrayBufferView).byteOffset,
      Math.min((data as ArrayBufferView).byteLength, 100),
    );
  const prefix = String.fromCharCode(...bytes);
  return prefix.includes("PRIVATE KEY");
}

let Writable;
function getWritable() {
  if (!Writable) Writable = lazyWritable().default;
  return Writable;
}

class SignImpl {
  hash: any;
  #digestType: string;

  constructor(algorithm: string, _options?: any) {
    validateString(algorithm, "algorithm");

    ensureSignProtoSetup();
    const W = getWritable();
    W.call(this, {
      write(chunk, enc, callback) {
        this.update(chunk, enc);
        callback();
      },
    });

    algorithm = algorithm.toLowerCase();

    this.#digestType = algorithm;
    try {
      this.hash = createHash(this.#digestType);
    } catch {
      throw new Error(`Invalid digest: ${algorithm}`);
    }
  }

  sign(
    privateKey: any,
    encoding?: any,
  ): Buffer | string {
    if (!privateKey) {
      throw new ERR_CRYPTO_SIGN_KEY_REQUIRED();
    }

    const res = prepareAsymmetricKey(privateKey, kConsumePrivate);

    // Options specific to RSA
    const rsaPadding = getPadding(privateKey);

    // Options specific to RSA-PSS
    const pssSaltLength = getSaltLength(privateKey);

    // Options specific to (EC)DSA
    const dsaSigEnc = getDSASignatureEncoding(privateKey);

    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else {
      try {
        handle = op_node_create_private_key(
          res.data,
          res.format,
          res.type ?? "",
          res.passphrase,
        );
      } catch (err) {
        // Trigger any prototype setter for `library` (Node.js compatibility)
        (err as Record<string, unknown>).library = "PEM routines";
        throw err;
      }
    }
    const ret = Buffer.from(op_node_sign(
      handle,
      this.hash.digest(),
      this.#digestType,
      pssSaltLength,
      rsaPadding,
      dsaSigEnc,
    ));
    return encoding && encoding !== "buffer" ? ret.toString(encoding) : ret;
  }

  update(
    data: any,
    encoding?: any,
  ): this {
    this.hash.update(data, encoding);
    return this;
  }
}

function Sign(algorithm: string, options?: any) {
  return new SignImpl(algorithm, options);
}

// Defer prototype setup
let _signProtoSetup = false;
function ensureSignProtoSetup() {
  if (_signProtoSetup) return;
  _signProtoSetup = true;
  const W = getWritable();
  Object.setPrototypeOf(SignImpl.prototype, W.prototype);
  Object.setPrototypeOf(SignImpl, W);
  Object.setPrototypeOf(VerifyImpl.prototype, W.prototype);
  Object.setPrototypeOf(VerifyImpl, W);
}

Sign.prototype = SignImpl.prototype;

class VerifyImpl {
  hash: any;
  #digestType: string;

  constructor(algorithm: string, _options?: any) {
    validateString(algorithm, "algorithm");

    ensureSignProtoSetup();
    const W = getWritable();
    W.call(this, {
      write(chunk, enc, callback) {
        this.update(chunk, enc);
        callback();
      },
    });

    algorithm = algorithm.toLowerCase();

    this.#digestType = algorithm;
    try {
      this.hash = createHash(this.#digestType);
    } catch {
      throw new Error(`Invalid digest: ${algorithm}`);
    }
  }

  update(data: any, encoding?: string): this {
    this.hash.update(data, encoding);
    return this;
  }

  verify(
    publicKey: any,
    signature: any,
    encoding?: any,
  ): boolean {
    if (
      typeof signature !== "string" &&
      !ArrayBuffer.isView(signature)
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "signature",
        ["Buffer", "TypedArray", "DataView"],
        signature,
      );
    }
    const res = prepareAsymmetricKey(publicKey, kConsumePublic);

    // Options specific to RSA
    const rsaPadding = getPadding(publicKey);

    // Options specific to RSA-PSS
    const pssSaltLength = getSaltLength(publicKey);

    // Options specific to (EC)DSA
    const dsaSigEnc = getDSASignatureEncoding(publicKey);

    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else if (
      isPrivateKeyType(res.type) ||
      (res.type === undefined && isPrivateKeyPem(res.data))
    ) {
      const privateHandle = op_node_create_private_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
      handle = op_node_derive_public_key_from_private_key(privateHandle);
    } else {
      handle = op_node_create_public_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    }
    return op_node_verify(
      handle,
      this.hash.digest(),
      this.#digestType,
      Buffer.from(signature, encoding),
      pssSaltLength,
      rsaPadding,
      dsaSigEnc,
    );
  }
}

function Verify(algorithm: string, options?: any) {
  return new VerifyImpl(algorithm, options);
}

Verify.prototype = VerifyImpl.prototype;

function signOneShot(
  algorithm: string | null | undefined,
  data: ArrayBufferView,
  key: any,
  callback?: (error: Error | null, data: Buffer) => void,
): Buffer | void {
  if (algorithm != null) {
    validateString(algorithm, "algorithm");
  }

  if (callback !== undefined) {
    validateFunction(callback, "callback");
  }

  if (!ArrayBuffer.isView(data) && typeof data !== "string") {
    throw new ERR_INVALID_ARG_TYPE(
      "data",
      ["Buffer", "TypedArray", "DataView"],
      data,
    );
  }

  if (!key) {
    throw new ERR_CRYPTO_SIGN_KEY_REQUIRED();
  }

  // Validate dsaEncoding early so it takes precedence over key errors
  if (typeof key === "object" && key !== null && !(key instanceof KeyObject)) {
    getDSASignatureEncoding(key);
  }

  // Normalize ArrayBufferView data to Uint8Array for Rust ops
  const dataBytes = ArrayBuffer.isView(data) && !(data instanceof Uint8Array)
    ? new Uint8Array(
      (data as ArrayBufferView).buffer,
      (data as ArrayBufferView).byteOffset,
      (data as ArrayBufferView).byteLength,
    )
    : data as ArrayBufferView | string;

  try {
    const res = prepareAsymmetricKey(key, kConsumePrivate);
    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else {
      handle = op_node_create_private_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    }

    let result: Buffer;
    const keyType = op_node_get_asymmetric_key_type(handle);
    if (keyType === "ed25519") {
      if (algorithm != null && algorithm !== "sha512") {
        throw new TypeError("Only 'sha512' is supported for Ed25519 keys");
      }
      result = new FastBuffer(64);
      op_node_sign_ed25519(handle, dataBytes, result);
    } else if (keyType === "ed448") {
      const keyOpts = typeof key === "object" && key !== null &&
          !(key instanceof KeyObject)
        ? key as Record<string, unknown>
        : null;
      const ctx = keyOpts?.context;
      if (ctx instanceof Uint8Array && ctx.length > 0) {
        throw new TypeError("Context parameter is unsupported");
      }
      result = new FastBuffer(114);
      op_node_sign_ed448(handle, dataBytes, result);
    } else {
      let digest = algorithm;
      if (digest == null) {
        if (keyType === "rsa-pss") {
          const details = op_node_get_asymmetric_key_details(handle);
          if (details.hashAlgorithm) {
            digest = details.hashAlgorithm;
          }
        }
        if (digest == null) {
          throw new TypeError(
            "Algorithm must be specified when using non-Ed25519 keys",
          );
        }
      }
      // Preserve padding/saltLength options from the original key
      const privateKeyObject = new PrivateKeyObject(handle);
      const signKey = typeof key === "object" && !(key instanceof KeyObject)
        ? { ...key, key: privateKeyObject }
        : privateKeyObject;
      result = Sign(digest).update(dataBytes)
        .sign(signKey);
    }

    if (callback) {
      setTimeout(() => callback(null, result));
    } else {
      return result;
    }
  } catch (err) {
    if (callback) {
      setTimeout(() => callback(err, Buffer.alloc(0)));
    } else {
      throw err;
    }
  }
}

function verifyOneShot(
  algorithm: string | null | undefined,
  data: any,
  key: any,
  signature: any,
  callback?: (error: Error | null, result: boolean) => void,
): boolean | void {
  if (algorithm != null) {
    validateString(algorithm, "algorithm");
  }

  if (callback !== undefined) {
    validateFunction(callback, "callback");
  }

  if (!ArrayBuffer.isView(data) && typeof data !== "string") {
    throw new ERR_INVALID_ARG_TYPE(
      "data",
      ["Buffer", "TypedArray", "DataView"],
      data,
    );
  }

  if (!ArrayBuffer.isView(signature) && typeof signature !== "string") {
    throw new ERR_INVALID_ARG_TYPE(
      "signature",
      ["Buffer", "TypedArray", "DataView"],
      signature,
    );
  }

  if (!key) {
    throw new ERR_CRYPTO_SIGN_KEY_REQUIRED();
  }

  // Normalize ArrayBufferView data to Uint8Array for Rust ops
  const dataBytes = ArrayBuffer.isView(data) && !(data instanceof Uint8Array)
    ? new Uint8Array(
      (data as ArrayBufferView).buffer,
      (data as ArrayBufferView).byteOffset,
      (data as ArrayBufferView).byteLength,
    )
    : data as ArrayBufferView | string;

  try {
    const res = prepareAsymmetricKey(key, kConsumePublic);
    let handle;
    if ("handle" in res) {
      handle = res.handle;
    } else if (
      isPrivateKeyType(res.type) ||
      (res.type === undefined && isPrivateKeyPem(res.data))
    ) {
      const privateHandle = op_node_create_private_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
      handle = op_node_derive_public_key_from_private_key(privateHandle);
    } else {
      handle = op_node_create_public_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    }

    let result: boolean;
    const keyType = op_node_get_asymmetric_key_type(handle);
    if (keyType === "ed25519") {
      if (algorithm != null && algorithm !== "sha512") {
        throw new TypeError("Only 'sha512' is supported for Ed25519 keys");
      }
      result = op_node_verify_ed25519(handle, dataBytes, signature);
    } else if (keyType === "ed448") {
      const keyOpts = typeof key === "object" && key !== null &&
          !(key instanceof KeyObject)
        ? key as Record<string, unknown>
        : null;
      const ctx = keyOpts?.context;
      if (ctx instanceof Uint8Array && ctx.length > 0) {
        throw new TypeError("Context parameter is unsupported");
      }
      result = op_node_verify_ed448(handle, dataBytes, signature);
    } else if (keyType === "x25519" || keyType === "x448" || keyType === "dh") {
      throw new TypeError(
        "operation not supported for this keytype",
      );
    } else {
      let digest = algorithm;
      if (digest == null) {
        if (keyType === "rsa-pss") {
          const details = op_node_get_asymmetric_key_details(handle);
          if (details.hashAlgorithm) {
            digest = details.hashAlgorithm;
          }
        }
        if (digest == null) {
          throw new TypeError("no default digest");
        }
      }
      // Preserve padding/saltLength options from the original key
      const publicKeyObject = new PublicKeyObject(handle);
      const verifyKey = typeof key === "object" && !(key instanceof KeyObject)
        ? { ...key, key: publicKeyObject }
        : publicKeyObject;
      result = Verify(digest).update(dataBytes)
        .verify(verifyKey, signature);
    }

    if (callback) {
      setTimeout(() => callback(null, result));
    } else {
      return result;
    }
  } catch (err) {
    if (callback) {
      setTimeout(() => callback(err, false));
    } else {
      throw err;
    }
  }
}

return {
  signOneShot,
  verifyOneShot,
  Sign,
  Verify,
  SignImpl,
  VerifyImpl,
  default: {
    signOneShot,
    verifyOneShot,
    Sign,
    Verify,
  },
};
})();
