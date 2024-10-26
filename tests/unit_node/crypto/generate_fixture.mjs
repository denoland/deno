// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Run this file with `node` to regenerate the testdata/crypto_digest_fixtures.json file.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import crypto from "node:crypto";
import { Buffer } from "node:buffer";

const privateKey = readFileSync(
  join(import.meta.dirname, "..", "testdata", "rsa_private.pem"),
);

const fixtures = [];

const DATA = "Hello, world!";
const SALT = "salt";
const INFO = "info";
const ITERATIONS = 1000;
const KEY_LEN = 64;

for (const digest of crypto.getHashes()) {
  const hasher = crypto.createHash(digest);
  hasher.update(DATA);
  let hash;
  try {
    hash = hasher.digest().toString("hex");
  } catch {
    hash = null;
  }

  const sign = crypto.createSign(digest);
  sign.update(DATA);
  let signature;
  try {
    signature = sign.sign(privateKey).toString("hex");
  } catch {
    signature = null;
  }

  let pkdf2;
  try {
    pkdf2 = crypto.pbkdf2Sync(DATA, SALT, ITERATIONS, KEY_LEN, digest).toString(
      "hex",
    );
  } catch {
    pkdf2 = null;
  }

  let hkdf;
  try {
    hkdf = Buffer.from(crypto.hkdfSync(digest, DATA, SALT, INFO, KEY_LEN))
      .toString("hex");
  } catch {
    hkdf = null;
  }

  fixtures.push({
    digest,
    hash,
    signature,
    pkdf2,
    hkdf,
  });
}

writeFileSync(
  join(import.meta.dirname, "..", "testdata", "crypto_digest_fixtures.json"),
  JSON.stringify(fixtures, null, 2),
);
