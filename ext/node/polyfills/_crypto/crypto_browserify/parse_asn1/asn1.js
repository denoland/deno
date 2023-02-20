// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// from https://github.com/crypto-browserify/parse-asn1/blob/fbd70dca8670d17955893e083ca69118908570be/asn1.js

import asn1 from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/mod.js";
import certificate from "internal:deno_node/polyfills/_crypto/crypto_browserify/parse_asn1/certificate.js";
export { certificate };

export const RSAPrivateKey = asn1.define("RSAPrivateKey", function () {
  this.seq().obj(
    this.key("version").int(),
    this.key("modulus").int(),
    this.key("publicExponent").int(),
    this.key("privateExponent").int(),
    this.key("prime1").int(),
    this.key("prime2").int(),
    this.key("exponent1").int(),
    this.key("exponent2").int(),
    this.key("coefficient").int(),
  );
});

export const RSAPublicKey = asn1.define("RSAPublicKey", function () {
  this.seq().obj(
    this.key("modulus").int(),
    this.key("publicExponent").int(),
  );
});

export const PublicKey = asn1.define("SubjectPublicKeyInfo", function () {
  this.seq().obj(
    this.key("algorithm").use(AlgorithmIdentifier),
    this.key("subjectPublicKey").bitstr(),
  );
});

const AlgorithmIdentifier = asn1.define("AlgorithmIdentifier", function () {
  this.seq().obj(
    this.key("algorithm").objid(),
    this.key("none").null_().optional(),
    this.key("curve").objid().optional(),
    this.key("params").seq().obj(
      this.key("p").int(),
      this.key("q").int(),
      this.key("g").int(),
    ).optional(),
  );
});

export const PrivateKey = asn1.define("PrivateKeyInfo", function () {
  this.seq().obj(
    this.key("version").int(),
    this.key("algorithm").use(AlgorithmIdentifier),
    this.key("subjectPrivateKey").octstr(),
  );
});
export const EncryptedPrivateKey = asn1.define(
  "EncryptedPrivateKeyInfo",
  function () {
    this.seq().obj(
      this.key("algorithm").seq().obj(
        this.key("id").objid(),
        this.key("decrypt").seq().obj(
          this.key("kde").seq().obj(
            this.key("id").objid(),
            this.key("kdeparams").seq().obj(
              this.key("salt").octstr(),
              this.key("iters").int(),
            ),
          ),
          this.key("cipher").seq().obj(
            this.key("algo").objid(),
            this.key("iv").octstr(),
          ),
        ),
      ),
      this.key("subjectPrivateKey").octstr(),
    );
  },
);

export const DSAPrivateKey = asn1.define("DSAPrivateKey", function () {
  this.seq().obj(
    this.key("version").int(),
    this.key("p").int(),
    this.key("q").int(),
    this.key("g").int(),
    this.key("pub_key").int(),
    this.key("priv_key").int(),
  );
});

export const DSAparam = asn1.define("DSAparam", function () {
  this.int();
});

export const ECPrivateKey = asn1.define("ECPrivateKey", function () {
  this.seq().obj(
    this.key("version").int(),
    this.key("privateKey").octstr(),
    this.key("parameters").optional().explicit(0).use(ECParameters),
    this.key("publicKey").optional().explicit(1).bitstr(),
  );
});

const ECParameters = asn1.define("ECParameters", function () {
  this.choice({
    namedCurve: this.objid(),
  });
});

export const signature = asn1.define("signature", function () {
  this.seq().obj(
    this.key("r").int(),
    this.key("s").int(),
  );
});
