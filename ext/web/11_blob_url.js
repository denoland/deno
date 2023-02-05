// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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

import { ops } from "deno:core/01_core.js";
import * as webidl from "deno:ext/webidl/00_webidl.js";
import { getParts } from "deno:ext/web/09_file.js";
import { URL } from "deno:ext/url/00_url.js";

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

  return ops.op_blob_create_object_url(blob.type, getParts(blob));
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
