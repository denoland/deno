// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { CryptoKey } = window.__bootstrap.crypto;
  const {
    ArrayBufferIsView,
    ArrayBufferPrototype,
    ObjectPrototypeIsPrototypeOf,
  } = window.__bootstrap.primordials;

  webidl.converters.AlgorithmIdentifier = (V, opts) => {
    // Union for (object or DOMString)
    if (webidl.type(V) == "Object") {
      return webidl.converters.object(V, opts);
    }
    return webidl.converters.DOMString(V, opts);
  };

  webidl.converters["BufferSource or JsonWebKey"] = (V, opts) => {
    // Union for (BufferSource or JsonWebKey)
    if (
      ArrayBufferIsView(V) ||
      ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, V)
    ) {
      return webidl.converters.BufferSource(V, opts);
    }
    return webidl.converters.JsonWebKey(V, opts);
  };

  webidl.converters.KeyType = webidl.createEnumConverter("KeyType", [
    "public",
    "private",
    "secret",
  ]);

  webidl.converters.KeyFormat = webidl.createEnumConverter("KeyFormat", [
    "raw",
    "pkcs8",
    "spki",
    "jwk",
  ]);

  webidl.converters.KeyUsage = webidl.createEnumConverter("KeyUsage", [
    "encrypt",
    "decrypt",
    "sign",
    "verify",
    "deriveKey",
    "deriveBits",
    "wrapKey",
    "unwrapKey",
  ]);

  webidl.converters["sequence<KeyUsage>"] = webidl.createSequenceConverter(
    webidl.converters.KeyUsage,
  );

  webidl.converters.HashAlgorithmIdentifier =
    webidl.converters.AlgorithmIdentifier;

  /** @type {__bootstrap.webidl.Dictionary} */
  const dictAlgorithm = [{
    key: "name",
    converter: webidl.converters.DOMString,
    required: true,
  }];

  webidl.converters.Algorithm = webidl
    .createDictionaryConverter("Algorithm", dictAlgorithm);

  webidl.converters.BigInteger = webidl.converters.Uint8Array;

  /** @type {__bootstrap.webidl.Dictionary} */
  const dictRsaKeyGenParams = [
    ...dictAlgorithm,
    {
      key: "modulusLength",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
      required: true,
    },
    {
      key: "publicExponent",
      converter: webidl.converters.BigInteger,
      required: true,
    },
  ];

  webidl.converters.RsaKeyGenParams = webidl
    .createDictionaryConverter("RsaKeyGenParams", dictRsaKeyGenParams);

  const dictRsaHashedKeyGenParams = [
    ...dictRsaKeyGenParams,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
  ];

  webidl.converters.RsaHashedKeyGenParams = webidl.createDictionaryConverter(
    "RsaHashedKeyGenParams",
    dictRsaHashedKeyGenParams,
  );

  const dictRsaHashedImportParams = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
  ];

  webidl.converters.RsaHashedImportParams = webidl.createDictionaryConverter(
    "RsaHashedImportParams",
    dictRsaHashedImportParams,
  );

  webidl.converters.NamedCurve = webidl.converters.DOMString;

  const dictEcKeyImportParams = [
    ...dictAlgorithm,
    {
      key: "namedCurve",
      converter: webidl.converters.NamedCurve,
      required: true,
    },
  ];

  webidl.converters.EcKeyImportParams = webidl.createDictionaryConverter(
    "EcKeyImportParams",
    dictEcKeyImportParams,
  );

  const dictEcKeyGenParams = [
    ...dictAlgorithm,
    {
      key: "namedCurve",
      converter: webidl.converters.NamedCurve,
      required: true,
    },
  ];

  webidl.converters.EcKeyGenParams = webidl
    .createDictionaryConverter("EcKeyGenParams", dictEcKeyGenParams);

  const dictEcImportParams = [
    ...dictAlgorithm,
    {
      key: "namedCurve",
      converter: webidl.converters.NamedCurve,
      required: true,
    },
  ];

  webidl.converters.EcImportParams = webidl
    .createDictionaryConverter("EcImportParams", dictEcImportParams);

  const dictAesKeyGenParams = [
    ...dictAlgorithm,
    {
      key: "length",
      converter: (V, opts) =>
        webidl.converters["unsigned short"](V, { ...opts, enforceRange: true }),
      required: true,
    },
  ];

  webidl.converters.AesKeyGenParams = webidl
    .createDictionaryConverter("AesKeyGenParams", dictAesKeyGenParams);

  const dictHmacKeyGenParams = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
    {
      key: "length",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
    },
  ];

  webidl.converters.HmacKeyGenParams = webidl
    .createDictionaryConverter("HmacKeyGenParams", dictHmacKeyGenParams);

  const dictRsaPssParams = [
    ...dictAlgorithm,
    {
      key: "saltLength",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
      required: true,
    },
  ];

  webidl.converters.RsaPssParams = webidl
    .createDictionaryConverter("RsaPssParams", dictRsaPssParams);

  const dictRsaOaepParams = [
    ...dictAlgorithm,
    {
      key: "label",
      converter: webidl.converters["BufferSource"],
    },
  ];

  webidl.converters.RsaOaepParams = webidl
    .createDictionaryConverter("RsaOaepParams", dictRsaOaepParams);

  const dictEcdsaParams = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
  ];

  webidl.converters["EcdsaParams"] = webidl
    .createDictionaryConverter("EcdsaParams", dictEcdsaParams);

  const dictHmacImportParams = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
    {
      key: "length",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
    },
  ];

  webidl.converters.HmacImportParams = webidl
    .createDictionaryConverter("HmacImportParams", dictHmacImportParams);

  const dictRsaOtherPrimesInfo = [
    {
      key: "r",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "d",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "t",
      converter: webidl.converters["DOMString"],
    },
  ];

  webidl.converters.RsaOtherPrimesInfo = webidl.createDictionaryConverter(
    "RsaOtherPrimesInfo",
    dictRsaOtherPrimesInfo,
  );
  webidl.converters["sequence<RsaOtherPrimesInfo>"] = webidl
    .createSequenceConverter(
      webidl.converters.RsaOtherPrimesInfo,
    );

  const dictJsonWebKey = [
    // Sections 4.2 and 4.3 of RFC7517.
    // https://datatracker.ietf.org/doc/html/rfc7517#section-4
    {
      key: "kty",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "use",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "key_ops",
      converter: webidl.converters["sequence<DOMString>"],
    },
    {
      key: "alg",
      converter: webidl.converters["DOMString"],
    },
    // JSON Web Key Parameters Registration
    {
      key: "ext",
      converter: webidl.converters["boolean"],
    },
    // Section 6 of RFC7518 JSON Web Algorithms
    // https://datatracker.ietf.org/doc/html/rfc7518#section-6
    {
      key: "crv",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "x",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "y",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "d",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "n",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "e",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "p",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "q",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "dp",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "dq",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "qi",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "oth",
      converter: webidl.converters["sequence<RsaOtherPrimesInfo>"],
    },
    {
      key: "k",
      converter: webidl.converters["DOMString"],
    },
  ];

  webidl.converters.JsonWebKey = webidl.createDictionaryConverter(
    "JsonWebKey",
    dictJsonWebKey,
  );

  const dictHkdfParams = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
    {
      key: "salt",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
    {
      key: "info",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
  ];

  webidl.converters.HkdfParams = webidl
    .createDictionaryConverter("HkdfParams", dictHkdfParams);

  const dictPbkdf2Params = [
    ...dictAlgorithm,
    {
      key: "hash",
      converter: webidl.converters.HashAlgorithmIdentifier,
      required: true,
    },
    {
      key: "iterations",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
      required: true,
    },
    {
      key: "salt",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
  ];

  webidl.converters.Pbkdf2Params = webidl
    .createDictionaryConverter("Pbkdf2Params", dictPbkdf2Params);

  const dictAesDerivedKeyParams = [
    ...dictAlgorithm,
    {
      key: "length",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
      required: true,
    },
  ];

  const dictAesCbcParams = [
    ...dictAlgorithm,
    {
      key: "iv",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
  ];

  const dictAesGcmParams = [
    ...dictAlgorithm,
    {
      key: "iv",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
    {
      key: "tagLength",
      converter: (V, opts) =>
        webidl.converters["unsigned long"](V, { ...opts, enforceRange: true }),
    },
    {
      key: "additionalData",
      converter: webidl.converters["BufferSource"],
    },
  ];

  const dictAesCtrParams = [
    ...dictAlgorithm,
    {
      key: "counter",
      converter: webidl.converters["BufferSource"],
      required: true,
    },
    {
      key: "length",
      converter: (V, opts) =>
        webidl.converters["unsigned short"](V, { ...opts, enforceRange: true }),
      required: true,
    },
  ];

  webidl.converters.AesDerivedKeyParams = webidl
    .createDictionaryConverter("AesDerivedKeyParams", dictAesDerivedKeyParams);

  webidl.converters.AesCbcParams = webidl
    .createDictionaryConverter("AesCbcParams", dictAesCbcParams);

  webidl.converters.AesGcmParams = webidl
    .createDictionaryConverter("AesGcmParams", dictAesGcmParams);

  webidl.converters.AesCtrParams = webidl
    .createDictionaryConverter("AesCtrParams", dictAesCtrParams);

  webidl.converters.CryptoKey = webidl.createInterfaceConverter(
    "CryptoKey",
    CryptoKey.prototype,
  );

  const dictCryptoKeyPair = [
    {
      key: "publicKey",
      converter: webidl.converters.CryptoKey,
    },
    {
      key: "privateKey",
      converter: webidl.converters.CryptoKey,
    },
  ];

  webidl.converters.CryptoKeyPair = webidl
    .createDictionaryConverter("CryptoKeyPair", dictCryptoKeyPair);

  const dictEcdhKeyDeriveParams = [
    ...dictAlgorithm,
    {
      key: "public",
      converter: webidl.converters.CryptoKey,
      required: true,
    },
  ];

  webidl.converters.EcdhKeyDeriveParams = webidl
    .createDictionaryConverter("EcdhKeyDeriveParams", dictEcdhKeyDeriveParams);
})(this);
