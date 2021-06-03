// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  webidl.converters["AlgorithmIdentifier"] = (V, opts) => {
    // Union for (DOMString or Algorithm)
    if (V === null || V === undefined) {
      return webidl.converters["Algorithm"](V, opts);
    }

    if (typeof V == "object") {
      return webidl.converters["Algorithm"](V, opts);
    }

    return webidl.converters["DOMString"](V, opts);
  };

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
})(this);
