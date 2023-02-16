// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import * as ECB from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/ecb.js";
import * as CBC from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/cbc.js";
import * as CFB from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/cfb.js";
import * as CFB8 from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/cfb8.js";
import * as CFB1 from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/cfb1.js";
import * as OFB from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/ofb.js";
import * as CTR from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/ctr.js";

const GCM = CTR;

const modeModules = {
  ECB,
  CBC,
  CFB,
  CFB8,
  CFB1,
  OFB,
  CTR,
  GCM,
};

export const MODES = {
  "aes-128-ecb": {
    "cipher": "AES",
    "key": 128,
    "iv": 0,
    "mode": "ECB",
    "type": "block",
  },
  "aes-192-ecb": {
    "cipher": "AES",
    "key": 192,
    "iv": 0,
    "mode": "ECB",
    "type": "block",
  },
  "aes-256-ecb": {
    "cipher": "AES",
    "key": 256,
    "iv": 0,
    "mode": "ECB",
    "type": "block",
  },
  "aes-128-cbc": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes-192-cbc": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes-256-cbc": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes128": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes192": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes256": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CBC",
    "type": "block",
  },
  "aes-128-cfb": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CFB",
    "type": "stream",
  },
  "aes-192-cfb": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CFB",
    "type": "stream",
  },
  "aes-256-cfb": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CFB",
    "type": "stream",
  },
  "aes-128-cfb8": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CFB8",
    "type": "stream",
  },
  "aes-192-cfb8": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CFB8",
    "type": "stream",
  },
  "aes-256-cfb8": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CFB8",
    "type": "stream",
  },
  "aes-128-cfb1": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CFB1",
    "type": "stream",
  },
  "aes-192-cfb1": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CFB1",
    "type": "stream",
  },
  "aes-256-cfb1": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CFB1",
    "type": "stream",
  },
  "aes-128-ofb": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "OFB",
    "type": "stream",
  },
  "aes-192-ofb": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "OFB",
    "type": "stream",
  },
  "aes-256-ofb": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "OFB",
    "type": "stream",
  },
  "aes-128-ctr": {
    "cipher": "AES",
    "key": 128,
    "iv": 16,
    "mode": "CTR",
    "type": "stream",
  },
  "aes-192-ctr": {
    "cipher": "AES",
    "key": 192,
    "iv": 16,
    "mode": "CTR",
    "type": "stream",
  },
  "aes-256-ctr": {
    "cipher": "AES",
    "key": 256,
    "iv": 16,
    "mode": "CTR",
    "type": "stream",
  },
  "aes-128-gcm": {
    "cipher": "AES",
    "key": 128,
    "iv": 12,
    "mode": "GCM",
    "type": "auth",
  },
  "aes-192-gcm": {
    "cipher": "AES",
    "key": 192,
    "iv": 12,
    "mode": "GCM",
    "type": "auth",
  },
  "aes-256-gcm": {
    "cipher": "AES",
    "key": 256,
    "iv": 12,
    "mode": "GCM",
    "type": "auth",
  },
};

for (const mode of Object.values(MODES)) {
  mode.module = modeModules[mode.mode];
}
