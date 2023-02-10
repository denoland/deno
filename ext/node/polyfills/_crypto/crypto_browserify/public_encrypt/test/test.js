// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import fs from "../../../../fs.ts";
import path from "../../../../path.ts";
import { Buffer } from "../../../../buffer.ts";
import parseKeys from "../../parse_asn1/mod.js";
import * as myCrypto from "../mod.js";
import { assertEquals } from "../../../../../testing/asserts.ts";

function load(filename) {
  return fs.readFileSync(path.fromFileUrl(new URL(filename, import.meta.url)));
}
const rsa1024 = {
  private: load("rsa.1024.priv"),
  public: load("rsa.1024.pub"),
};
const rsa1024priv = {
  private: load("rsa.1024.priv"),
  public: load("rsa.1024.priv"),
};

const rsa2028 = {
  private: load("rsa.2028.priv"),
  public: load("rsa.2028.pub"),
};
const nonrsa1024 = {
  private: load("1024.priv"),
  public: load("1024.pub"),
};
const nonrsa1024str = {
  private: load("1024.priv").toString(),
  public: load("1024.pub").toString(),
};
const pass1024 = {
  private: {
    passphrase: "fooo",
    key: load("pass.1024.priv"),
  },
  public: load("pass.1024.pub"),
};
const pass2028 = {
  private: {
    passphrase: "password",
    key: load("rsa.pass.priv"),
  },
  public: load("rsa.pass.pub"),
};

async function _testIt(keys, message, t) {
  const pub = keys.public;
  const priv = keys.private;
  await t.step(message.toString(), function () {
    let myEnc = myCrypto.publicEncrypt(pub, message);
    assertEquals(
      myCrypto.privateDecrypt(priv, myEnc).toString("hex"),
      message.toString("hex"),
      "my decrypter my message",
    );
    myEnc = myCrypto.privateEncrypt(priv, message);
    assertEquals(
      myCrypto.publicDecrypt(pub, myEnc).toString("hex"),
      message.toString("hex"),
      "reverse methods my decrypter my message",
    );
  });
}
async function testIt(keys, message, t) {
  await _testIt(keys, message, t);
  await _testIt(
    paddingObject(keys, 1),
    Buffer.concat([message, Buffer.from(" with RSA_PKCS1_PADDING")]),
    t,
  );
  const parsedKey = parseKeys(keys.public);
  const k = parsedKey.modulus.byteLength();
  const zBuf = Buffer.alloc(k);
  const msg = Buffer.concat([zBuf, message, Buffer.from(" with no padding")])
    .slice(-k);
  await _testIt(paddingObject(keys, 3), msg, t);
}
function paddingObject(keys, padding) {
  return {
    public: addPadding(keys.public, padding),
    private: addPadding(keys.private, padding),
  };
}
function addPadding(key, padding) {
  if (typeof key === "string" || Buffer.isBuffer(key)) {
    return {
      key: key,
      padding: padding,
    };
  }
  const out = {
    key: key.key,
    padding: padding,
  };
  if ("passphrase" in key) {
    out.passphrase = key.passphrase;
  }
  return out;
}
function testRun(i) {
  Deno.test("run " + i, async function (t) {
    await testIt(rsa1024priv, Buffer.from("1024 2 private keys"), t);
    await testIt(rsa1024, Buffer.from("1024 keys"), t);
    await testIt(rsa2028, Buffer.from("2028 keys"), t);
    await testIt(nonrsa1024, Buffer.from("1024 keys non-rsa key"), t);
    await testIt(pass1024, Buffer.from("1024 keys and password"), t);
    await testIt(
      nonrsa1024str,
      Buffer.from("1024 keys non-rsa key as a string"),
      t,
    );
    await testIt(
      pass2028,
      Buffer.from("2028 rsa key with variant passwords"),
      t,
    );
  });
}

let i = 0;
const num = 20;
while (++i <= num) {
  testRun(i);
}
