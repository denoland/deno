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
