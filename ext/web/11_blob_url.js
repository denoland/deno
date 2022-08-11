// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../url/lib.deno_url.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = Deno.core;
  const ops = core.ops;
  const webidl = window.__bootstrap.webidl;
  const { getParts } = window.__bootstrap.file;
  const { URL } = window.__bootstrap.url;

  /**
   * @param {Blob} blob
   * @returns {string}
   */
  function createObjectURL(blob) {
    const prefix = "Failed to execute 'createObjectURL' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    blob = webidl.converters["Blob"](blob, {
      context: "Argument 1",
      prefix,
    });

    const url = ops.op_blob_create_object_url(
      blob.type,
      getParts(blob),
    );

    return url;
  }

  /**
   * @param {string} url
   * @returns {void}
   */
  function revokeObjectURL(url) {
    const prefix = "Failed to execute 'revokeObjectURL' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    url = webidl.converters["DOMString"](url, {
      context: "Argument 1",
      prefix,
    });

    ops.op_blob_revoke_object_url(url);
  }

  URL.createObjectURL = createObjectURL;
  URL.revokeObjectURL = revokeObjectURL;
})(globalThis);
