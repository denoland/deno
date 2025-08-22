// Copyright 2018-2025 the Deno authors. MIT license.
import { hkdf, hkdfSync } from "node:crypto";
import { assert, assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";
import nodeFixtures from "../testdata/crypto_digest_fixtures.json" with {
  type: "json",
};

Deno.test("crypto.hkdfSync - compare with node", async (t) => {
  const DATA = "Hello, world!";
  const SALT = "salt";
  const INFO = "info";
  const KEY_LEN = 64;

  for (const { digest, hkdf } of nodeFixtures) {
    await t.step({
      name: digest,
      ignore: digest.includes("blake"),
      fn() {
        let actual: string | null;
        try {
          actual = Buffer.from(hkdfSync(
            digest,
            DATA,
            SALT,
            INFO,
            KEY_LEN,
          )).toString("hex");
        } catch {
          actual = null;
        }
        assertEquals(actual, hkdf);
      },
    });
  }
});

Deno.test("crypto.hkdfSync - TypedArray byte representation fix", () => {
  const secret = "secret";
  const keyLen = 10;

  // 1. CORE BUG FIX: Different TypedArrays use different byte representations
  const stringResult = hkdfSync("sha256", secret, "salt", "info", keyLen);
  const uint16Result = hkdfSync(
    "sha256",
    secret,
    new Uint16Array(Buffer.from("salt")),
    new Uint16Array(Buffer.from("info")),
    keyLen,
  );

  const stringHex = Buffer.from(stringResult).toString("hex");
  const uint16Hex = Buffer.from(uint16Result).toString("hex");

  // Critical assertion: Uint16Array should produce different result than string
  assert(
    stringHex !== uint16Hex,
    `Uint16Array should use different byte representation than string. ` +
      `String: ${stringHex}, Uint16Array: ${uint16Hex}`,
  );

  // 2. BYTE-COMPATIBLE ARRAYS (should match string - 1 byte per element)
  const byteCompatibleTypes = [
    { name: "Uint8Array", ctor: Uint8Array, expected: "f6d2fcc47cb939deafe3" },
    { name: "Int8Array", ctor: Int8Array, expected: "f6d2fcc47cb939deafe3" },
  ];

  // 3. MULTI-BYTE ARRAYS (should differ from string - multiple bytes per element)
  const multiByteTypes = [
    {
      name: "Uint16Array",
      ctor: Uint16Array,
      expected: "db570fbe9a3a81e18bef",
    },
    { name: "Int16Array", ctor: Int16Array, expected: "db570fbe9a3a81e18bef" },
    {
      name: "Uint32Array",
      ctor: Uint32Array,
      expected: "5666e949b1b3c069b7fa",
    },
    { name: "Int32Array", ctor: Int32Array, expected: "5666e949b1b3c069b7fa" },
  ];

  // Test all TypedArray types systematically
  for (
    const { name, ctor, expected } of [
      ...byteCompatibleTypes,
      ...multiByteTypes,
    ]
  ) {
    const result = hkdfSync(
      "sha256",
      secret,
      new ctor(Buffer.from("salt")),
      new ctor(Buffer.from("info")),
      keyLen,
    );
    const resultHex = Buffer.from(result).toString("hex");

    // Verify correct length
    assertEquals(
      result.byteLength,
      keyLen,
      `${name} should produce correct length result`,
    );

    // Verify Node.js-compatible result
    assertEquals(
      resultHex,
      expected,
      `${name} should produce Node.js-compatible result`,
    );

    // Verify TypedArray matches its underlying ArrayBuffer
    const ta = new ctor(Buffer.from("salt"));
    const saltSlice = new Uint8Array(ta.buffer, ta.byteOffset, ta.byteLength);
    const taInfo = new ctor(Buffer.from("info"));
    const infoSlice = new Uint8Array(
      taInfo.buffer,
      taInfo.byteOffset,
      taInfo.byteLength,
    );

    const bufferResult = hkdfSync(
      "sha256",
      secret,
      saltSlice,
      infoSlice,
      keyLen,
    );
    const bufferHex = Buffer.from(bufferResult).toString("hex");

    assertEquals(
      resultHex,
      bufferHex,
      `${name} should match its underlying ArrayBuffer representation`,
    );
  }

  // 4. VERIFY BYTE-COMPATIBLE ARRAYS MATCH STRING
  for (const { name, ctor } of byteCompatibleTypes) {
    const result = hkdfSync(
      "sha256",
      secret,
      new ctor(Buffer.from("salt")),
      new ctor(Buffer.from("info")),
      keyLen,
    );
    const resultHex = Buffer.from(result).toString("hex");

    assertEquals(
      resultHex,
      stringHex,
      `${name} should match string result (same byte representation)`,
    );
  }
});

Deno.test("crypto.hkdfSync - DataView inputs", () => {
  const secret = "secret";
  const saltBuffer = Buffer.from("salt");
  const infoBuffer = Buffer.from("info");
  const keyLen = 10;

  // Test DataView inputs (should behave like Uint8Array)
  const dataViewResult = hkdfSync(
    "sha256",
    secret,
    new DataView(
      saltBuffer.buffer,
      saltBuffer.byteOffset,
      saltBuffer.byteLength,
    ),
    new DataView(
      infoBuffer.buffer,
      infoBuffer.byteOffset,
      infoBuffer.byteLength,
    ),
    keyLen,
  );

  assertEquals(
    dataViewResult.byteLength,
    keyLen,
    "DataView result should have correct length",
  );

  // DataView should match string inputs since both represent the same bytes
  const stringResult = hkdfSync("sha256", secret, "salt", "info", keyLen);
  const dataViewHex = Buffer.from(dataViewResult).toString("hex");
  const stringHex = Buffer.from(stringResult).toString("hex");

  assertEquals(dataViewHex, stringHex, "DataView should match string result");
  assertEquals(
    dataViewHex,
    "f6d2fcc47cb939deafe3",
    "DataView should produce expected Node.js result",
  );

  // Test DataView with different underlying buffer scenarios
  const largerBuffer = new ArrayBuffer(16);
  const view = new Uint8Array(largerBuffer);
  view.set([115, 97, 108, 116], 4); // "salt" at offset 4

  const offsetDataView = new DataView(largerBuffer, 4, 4);
  const offsetResult = hkdfSync(
    "sha256",
    secret,
    offsetDataView,
    "info",
    keyLen,
  );

  assertEquals(
    Buffer.from(offsetResult).toString("hex"),
    stringHex,
    "DataView with offset should still produce correct result",
  );
});

Deno.test("crypto.hkdfSync - edge cases and error conditions", () => {
  const secret = "secret";
  const keyLen = 10;

  // Test empty TypedArrays
  const emptyResult = hkdfSync(
    "sha256",
    secret,
    new Uint8Array(0), // Empty salt
    new Uint8Array(0), // Empty info
    keyLen,
  );
  assertEquals(emptyResult.byteLength, keyLen, "Empty TypedArrays should work");

  // Test single-byte TypedArrays
  const singleByteResult = hkdfSync(
    "sha256",
    secret,
    new Uint8Array([65]), // Single byte 'A'
    new Uint8Array([66]), // Single byte 'B'
    keyLen,
  );
  assertEquals(
    singleByteResult.byteLength,
    keyLen,
    "Single-byte TypedArrays should work",
  );

  // Test maximum allowed info length (1024 bytes)
  const maxInfo = new Uint8Array(1024).fill(42);
  const maxInfoResult = hkdfSync("sha256", secret, "salt", maxInfo, keyLen);
  assertEquals(
    maxInfoResult.byteLength,
    keyLen,
    "Maximum info length should work",
  );

  // Test info length exceeding limit should throw
  const oversizedInfo = new Uint8Array(1025).fill(42);
  let threwError = false;
  try {
    hkdfSync("sha256", secret, "salt", oversizedInfo, keyLen);
  } catch (error) {
    threwError = true;
    assertEquals(
      (error as Error).message.includes(
        "must not contain more than 1024 bytes",
      ),
      true,
      "Should throw specific error for oversized info",
    );
  }
  assertEquals(threwError, true, "Oversized info should throw error");

  // Test various key lengths
  const keyLengths = [1, 32, 64, 255];
  for (const len of keyLengths) {
    const result = hkdfSync("sha256", secret, "salt", "info", len);
    assertEquals(result.byteLength, len, `Key length ${len} should work`);
  }

  // Test different digest algorithms with TypedArrays
  const digests = ["sha1", "sha256", "sha384", "sha512"];
  for (const digest of digests) {
    const result = hkdfSync(
      digest,
      secret,
      new Uint16Array(Buffer.from("salt")),
      new Uint16Array(Buffer.from("info")),
      keyLen,
    );
    assertEquals(
      result.byteLength,
      keyLen,
      `${digest} with TypedArrays should work`,
    );
  }
});

// === Async parity helpers ===
function hkdfAsyncP(
  digest: string,
  secret: string,
  salt: unknown,
  info: unknown,
  length: number,
): Promise<ArrayBuffer | Uint8Array> {
  return new Promise((resolve, reject) => {
    // deno-lint-ignore no-explicit-any
    hkdf(digest, secret, salt as any, info as any, length, (err, derived) => {
      if (err) return reject(err);
      resolve(derived!);
    });
  });
}

function asHex(ab: ArrayBuffer | Uint8Array): string {
  const u8 = ab instanceof Uint8Array ? ab : new Uint8Array(ab);
  return Buffer.from(u8).toString("hex");
}

Deno.test("crypto.hkdf (async) - strings match sync", async () => {
  const secret = "secret", keyLen = 10;
  const syncHex = asHex(hkdfSync("sha256", secret, "salt", "info", keyLen));
  const asyncHex = asHex(
    await hkdfAsyncP("sha256", secret, "salt", "info", keyLen),
  );
  assertEquals(asyncHex, syncHex);
});

Deno.test("crypto.hkdf (async) - TypedArray array-like inputs match sync", async () => {
  const secret = "secret", keyLen = 10;
  const cases = [
    {
      name: "Int8Array",
      salt: new Int8Array(Buffer.from("salt")),
      info: new Int8Array(Buffer.from("info")),
    },
    {
      name: "Uint8Array",
      salt: new Uint8Array(Buffer.from("salt")),
      info: new Uint8Array(Buffer.from("info")),
    },
    {
      name: "Int16Array",
      salt: new Int16Array(Buffer.from("salt")),
      info: new Int16Array(Buffer.from("info")),
    },
    {
      name: "Uint16Array",
      salt: new Uint16Array(Buffer.from("salt")),
      info: new Uint16Array(Buffer.from("info")),
    },
    {
      name: "Int32Array",
      salt: new Int32Array(Buffer.from("salt")),
      info: new Int32Array(Buffer.from("info")),
    },
    {
      name: "Uint32Array",
      salt: new Uint32Array(Buffer.from("salt")),
      info: new Uint32Array(Buffer.from("info")),
    },
  ];
  for (const { name, salt, info } of cases) {
    const syncHex = asHex(hkdfSync("sha256", secret, salt, info, keyLen));
    const asyncHex = asHex(
      await hkdfAsyncP("sha256", secret, salt, info, keyLen),
    );
    assertEquals(
      asyncHex,
      syncHex,
      `${name} async should equal sync for identical bytes`,
    );
  }
});

Deno.test("crypto.hkdf (async) - mixed TypedArray types match sync", async () => {
  const secret = "secret", keyLen = 10;

  // Uint16 salt + Uint8 info (array-like)
  const saltU16 = new Uint16Array(Buffer.from("salt"));
  const infoU8 = new Uint8Array(Buffer.from("info"));

  const syncHex = asHex(hkdfSync("sha256", secret, saltU16, infoU8, keyLen));
  const asyncHex = asHex(
    await hkdfAsyncP("sha256", secret, saltU16, infoU8, keyLen),
  );
  assertEquals(
    asyncHex,
    syncHex,
    "mixed (Uint16 + Uint8) async should equal sync",
  );

  // A couple more combos
  const combos = [
    {
      salt: new Int8Array(Buffer.from("salt")),
      info: new Uint16Array(Buffer.from("info")),
      label: "Int8 + Uint16",
    },
    {
      salt: new Uint32Array(Buffer.from("salt")),
      info: new Uint8Array(Buffer.from("info")),
      label: "Uint32 + Uint8",
    },
    {
      salt: new Int16Array(Buffer.from("salt")),
      info: new Int32Array(Buffer.from("info")),
      label: "Int16 + Int32",
    },
  ];
  for (const { salt, info, label } of combos) {
    const sHex = asHex(hkdfSync("sha256", secret, salt, info, keyLen));
    const aHex = asHex(await hkdfAsyncP("sha256", secret, salt, info, keyLen));
    assertEquals(aHex, sHex, `${label} async should equal sync`);
  }
});

Deno.test("crypto.hkdf (async) - DataView inputs match sync", async () => {
  const secret = "secret", keyLen = 10;
  const salt = Buffer.from("salt");
  const info = Buffer.from("info");
  const dvSalt = new DataView(salt.buffer, salt.byteOffset, salt.byteLength);
  const dvInfo = new DataView(info.buffer, info.byteOffset, info.byteLength);

  const syncHex = asHex(hkdfSync("sha256", secret, dvSalt, dvInfo, keyLen));
  const asyncHex = asHex(
    await hkdfAsyncP("sha256", secret, dvSalt, dvInfo, keyLen),
  );
  assertEquals(asyncHex, syncHex);
});

Deno.test("crypto.hkdf (async) - matches underlying ArrayBuffer bytes", async () => {
  const secret = "secret", keyLen = 10;
  const saltTA = new Uint16Array(Buffer.from("salt"));
  const infoTA = new Uint16Array(Buffer.from("info"));

  const asyncHexTA = asHex(
    await hkdfAsyncP("sha256", secret, saltTA, infoTA, keyLen),
  );

  const saltBytes = new Uint8Array(
    saltTA.buffer,
    saltTA.byteOffset,
    saltTA.byteLength,
  );
  const infoBytes = new Uint8Array(
    infoTA.buffer,
    infoTA.byteOffset,
    infoTA.byteLength,
  );
  const asyncHexBuf = asHex(
    await hkdfAsyncP("sha256", secret, saltBytes, infoBytes, keyLen),
  );

  assertEquals(
    asyncHexTA,
    asyncHexBuf,
    "async TA result should equal async bytes result",
  );
});

Deno.test("crypto.hkdf (async) - error cases (invalid digest, oversized info)", async () => {
  const secret = "secret", keyLen = 10;

  // Invalid digest -> reject
  await (async () => {
    let rejected = false;
    try {
      await hkdfAsyncP("sha256-bogus", secret, "salt", "info", keyLen);
    } catch {
      rejected = true;
    }
    assertEquals(rejected, true, "async hkdf should reject on invalid digest");
  })();

  // info > 1024 bytes -> reject
  const oversizedInfo = new Uint8Array(1025).fill(1);
  await (async () => {
    let rejected = false;
    try {
      await hkdfAsyncP("sha256", secret, "salt", oversizedInfo, keyLen);
    } catch {
      rejected = true;
    }
    assertEquals(
      rejected,
      true,
      "async hkdf should reject when info > 1024 bytes",
    );
  })();
});
