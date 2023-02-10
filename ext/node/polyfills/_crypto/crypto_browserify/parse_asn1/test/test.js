// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// from https://github.com/crypto-browserify/parse-asn1/blob/fbd70dca8670d17955893e083ca69118908570be/test/index.js

import fs from "../../../../fs.ts";
import path from "../../../../path.ts";
import parseKey from "../mod.js";
import { assert } from "../../../../../testing/asserts.ts";

function loadPath(str) {
  return fs.readFileSync(path.fromFileUrl(new URL("." + str, import.meta.url)));
}
const rsa1024 = {
  private: loadPath("/rsa.1024.priv"),
  public: loadPath("/rsa.1024.pub"),
};
const rsa2028 = {
  private: loadPath("/rsa.2028.priv"),
  public: loadPath("/rsa.2028.pub"),
};
const nonrsa1024 = {
  private: loadPath("/1024.priv"),
  public: loadPath("/1024.pub"),
};
const pass1024 = {
  private: {
    passphrase: "fooo",
    key: loadPath("/pass.1024.priv"),
  },
  public: loadPath("/pass.1024.pub"),
};
const ec = {
  private: loadPath("/ec.priv"),
  public: loadPath("/ec.pub"),
};
const ecpass = {
  private: {
    key: loadPath("/ec.pass.priv"),
    passphrase: "bard",
  },
  public: loadPath("/ec.pub"),
};
const dsa = {
  private: loadPath("/dsa.1024.priv"),
  public: loadPath("/dsa.1024.pub"),
};
const dsa2 = {
  private: loadPath("/dsa.2048.priv"),
  public: loadPath("/dsa.2048.pub"),
};
const dsapass = {
  private: {
    key: loadPath("/pass.dsa.1024.priv"),
    passphrase: "password",
  },
  public: loadPath("/pass.dsa.1024.pub"),
};
const dsapass2 = {
  private: {
    key: loadPath("/pass2.dsa.1024.priv"),
    passphrase: "password",
  },
  public: loadPath("/pass2.dsa.1024.pub"),
};
const rsapass = {
  private: {
    key: loadPath("/pass.rsa.1024.priv"),
    passphrase: "password",
  },
  public: loadPath("/pass.rsa.1024.pub"),
};
const rsapass2 = {
  private: {
    key: loadPath("/pass.rsa.2028.priv"),
    passphrase: "password",
  },
  public: loadPath("/pass.rsa.2028.pub"),
};
const cert = {
  private: loadPath("/rsa.1024.priv"),
  public: loadPath("/node.cert"),
};
const cert2 = {
  private: loadPath("/cert.priv"),
  public: loadPath("/cert.pub"),
};
let i = 0;
function testIt(keys) {
  Deno.test("key " + (++i), function () {
    assert(parseKey(keys.public), "public key");
    assert(parseKey(keys.private), "private key");
  });
}

testIt(dsa);
testIt(dsa2);
testIt(rsa1024);
testIt(ec);
testIt(rsa2028);
testIt(nonrsa1024);
testIt(ecpass);
testIt(dsapass);
testIt(dsapass2);
testIt(rsapass);
testIt(rsapass2);
testIt(pass1024);
testIt(pass1024);
testIt(cert);
testIt(cert2);
