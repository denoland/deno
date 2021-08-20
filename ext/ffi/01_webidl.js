// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;

  webidl.converters.NativeType = webidl.createEnumConverter("NativeType", [
    "void",
    "u8",
    "i8",
    "u16",
    "i16",
    "u32",
    "i32",
    "u64",
    "i64",
    "usize",
    "isize",
    "f32",
    "f64",
  ]);

  webidl.converters["sequence<NativeType>"] = webidl.createSequenceConverter(
    webidl.converters.NativeType
  );

  webidl.converters.ForeignFunction = webidl.createDictionaryConverter(
    "ForeignFunction",
    [
      {
        key: "parameters",
        converter: webidl.converters["sequence<NativeType>"],
        required: true,
      },
      {
        key: "result",
        converter: webidl.converters.NativeType,
        required: true,
      },
    ]
  );

  webidl.converters.DLSymbols = webidl.createRecordConverter(
    webidl.converters.USVString,
    webidl.converters.ForeignFunction
  );
})(this);
