// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Provides the {@linkcode KeyStack} class which implements the
 * {@linkcode KeyRing} interface for managing rotatable keys.
 *
 * @module
 */

import { timingSafeEqual } from "./timing_safe_equal.ts";
import * as base64url from "../encoding/base64url.ts";

/** Types of data that can be signed cryptographically. */
export type Data = string | number[] | ArrayBuffer | Uint8Array;

/** Types of keys that can be used to sign data. */
export type Key = string | number[] | ArrayBuffer | Uint8Array;

const encoder = new TextEncoder();

function importKey(key: Key): Promise<CryptoKey> {
  if (typeof key === "string") {
    key = encoder.encode(key);
  } else if (Array.isArray(key)) {
    key = new Uint8Array(key);
  }
  return crypto.subtle.importKey(
    "raw",
    key,
    {
      name: "HMAC",
      hash: { name: "SHA-256" },
    },
    true,
    ["sign", "verify"],
  );
}

function sign(data: Data, key: CryptoKey): Promise<ArrayBuffer> {
  if (typeof data === "string") {
    data = encoder.encode(data);
  } else if (Array.isArray(data)) {
    data = Uint8Array.from(data);
  }
  return crypto.subtle.sign("HMAC", key, data);
}

/** Compare two strings, Uint8Arrays, ArrayBuffers, or arrays of numbers in a
 * way that avoids timing based attacks on the comparisons on the values.
 *
 * The function will return `true` if the values match, or `false`, if they
 * do not match.
 *
 * This was inspired by https://github.com/suryagh/tsscmp which provides a
 * timing safe string comparison to avoid timing attacks as described in
 * https://codahale.com/a-lesson-in-timing-attacks/.
 */
async function compare(a: Data, b: Data): Promise<boolean> {
  const key = new Uint8Array(32);
  globalThis.crypto.getRandomValues(key);
  const cryptoKey = await importKey(key);
  const ah = await sign(a, cryptoKey);
  const bh = await sign(b, cryptoKey);
  return timingSafeEqual(ah, bh);
}

/** A cryptographic key chain which allows signing of data to prevent tampering,
 * but also allows for easy key rotation without needing to re-sign the data.
 *
 * Data is signed as SHA256 HMAC.
 *
 * This was inspired by [keygrip](https://github.com/crypto-utils/keygrip/).
 *
 * @example
 * ```ts
 * import { KeyStack } from "https://deno.land/std@$STD_VERSION/crypto/unstable_keystack.ts";
 *
 * const keyStack = new KeyStack(["hello", "world"]);
 * const digest = await keyStack.sign("some data");
 *
 * const rotatedStack = new KeyStack(["deno", "says", "hello", "world"]);
 * await rotatedStack.verify("some data", digest); // true
 * ```
 */
export class KeyStack {
  #cryptoKeys = new Map<Key, CryptoKey>();
  #keys: Key[];

  async #toCryptoKey(key: Key): Promise<CryptoKey> {
    if (!this.#cryptoKeys.has(key)) {
      this.#cryptoKeys.set(key, await importKey(key));
    }
    return this.#cryptoKeys.get(key)!;
  }

  get length(): number {
    return this.#keys.length;
  }

  /** A class which accepts an array of keys that are used to sign and verify
   * data and allows easy key rotation without invalidation of previously signed
   * data.
   *
   * @param keys An iterable of keys, of which the index 0 will be used to sign
   *             data, but verification can happen against any key.
   */
  constructor(keys: Iterable<Key>) {
    const values = Array.isArray(keys) ? keys : [...keys];
    if (!(values.length)) {
      throw new TypeError("keys must contain at least one value");
    }
    this.#keys = values;
  }

  /** Take `data` and return a SHA256 HMAC digest that uses the current 0 index
   * of the `keys` passed to the constructor.  This digest is in the form of a
   * URL safe base64 encoded string. */
  async sign(data: Data): Promise<string> {
    const key = await this.#toCryptoKey(this.#keys[0]);
    return base64url.encode(await sign(data, key));
  }

  /** Given `data` and a `digest`, verify that one of the `keys` provided the
   * constructor was used to generate the `digest`.  Returns `true` if one of
   * the keys was used, otherwise `false`. */
  async verify(data: Data, digest: string): Promise<boolean> {
    return (await this.indexOf(data, digest)) > -1;
  }

  /** Given `data` and a `digest`, return the current index of the key in the
   * `keys` passed the constructor that was used to generate the digest.  If no
   * key can be found, the method returns `-1`. */
  async indexOf(data: Data, digest: string): Promise<number> {
    for (let i = 0; i < this.#keys.length; i++) {
      const cryptoKey = await this.#toCryptoKey(this.#keys[i]);
      if (
        await compare(digest, base64url.encode(await sign(data, cryptoKey)))
      ) {
        return i;
      }
    }
    return -1;
  }

  [Symbol.for("Deno.customInspect")](inspect: (value: unknown) => string) {
    const { length } = this;
    return `${this.constructor.name} ${inspect({ length })}`;
  }

  [Symbol.for("nodejs.util.inspect.custom")](
    depth: number,
    // deno-lint-ignore no-explicit-any
    options: any,
    inspect: (value: unknown, options?: unknown) => string,
  ) {
    if (depth < 0) {
      return options.stylize(`[${this.constructor.name}]`, "special");
    }

    const newOptions = Object.assign({}, options, {
      depth: options.depth === null ? null : options.depth - 1,
    });
    const { length } = this;
    return `${options.stylize(this.constructor.name, "special")} ${
      inspect({ length }, newOptions)
    }`;
  }
}
