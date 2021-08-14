// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { CryptoKey } = window.__bootstrap.crypto;

  webidl.converters.AlgorithmIdentifier = (V, opts) => {
    // Union for (object or DOMString)
    if (webidl.type(V) == "Object") {
      return webidl.converters.object(V, opts);
    }
    return webidl.converters.DOMString(V, opts);
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

  webidl.converters.NamedCurve = webidl.converters.DOMString;

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

  webidl.converters.CryptoKey = webidl.createInterfaceConverter(
    "CryptoKey",
    CryptoKey,
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
})(this);
