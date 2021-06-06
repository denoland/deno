// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  webidl.converters["AlgorithmIdentifier"] = (V, opts) => {
    // Union for (object or DOMString)
    if (typeof V == "object") {
      return webidl.converters["object"](V, opts);
    }

    return webidl.converters["DOMString"](V, opts);
  };

  webidl.converters["KeyType"] = webidl.createEnumConverter("KeyType", [
    "public",
    "private",
    "secret",
  ]);

  webidl.converters["KeyUsage"] = webidl.createEnumConverter("KeyUsage", [
    "encrypt",
    "decrypt",
    "sign",
    "verify",
    "deriveKey",
    "deriveBits",
    "wrapKey",
    "unwrapKey",
  ]);

  const algorithmDictionary = [
    {
      key: "name",
      converter: webidl.converters["DOMString"],
    },
  ];

  webidl.converters["Algorithm"] = webidl.createDictionaryConverter(
    "Algorithm",
    algorithmDictionary,
  );

  const cryptoKeyDictionary = [
    {
      key: "type",
      converter: webidl.converters["KeyType"],
      required: true,
    },
    {
      key: "extractable",
      converter: webidl.converters["boolean"],
      required: true,
    },
    {
      key: "algorithm",
      converter: webidl.converters["DOMString"],
      required: true,
    },
    {
      key: "usages",
      converter: webidl.createSequenceConverter(
        webidl.converters["KeyUsage"],
      ),
      required: true,
    },
  ];

  webidl.converters["CryptoKey"] = webidl.createDictionaryConverter(
    "CryptoKey",
    cryptoKeyDictionary,
  );

  const cryptoKeyPairDictionary = [
    {
      key: "publicKey",
      converter: webidl.converters["CryptoKey"],
    },
    {
      key: "privateKey",
      converter: webidl.converters["CryptoKey"],
    },
  ];

  webidl.converters["CryptoKeyPair"] = webidl.createDictionaryConverter(
    "CryptoKeyPair",
    cryptoKeyPairDictionary,
  );
})(this);
