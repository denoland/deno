// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { TextDecoder } from "ext:deno_web/08_text_encoding.js";
import { Blob } from "ext:deno_web/09_file.js";
import { Buffer } from "node:buffer";
"use strict";

const {
  JSONParse,
} = primordials;

/**
 * @typedef {import('../internal/webstreams/readablestream').ReadableStream
 * } ReadableStream
 * @typedef {import('../internal/streams/readable')} Readable
 */

/**
 * @param {AsyncIterable|ReadableStream|Readable} stream
 * @returns {Promise<Blob>}
 */
async function blob(stream) {
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(chunk);
  }
  return new Blob(chunks);
}

/**
 * @param {AsyncIterable|ReadableStream|Readable} stream
 * @returns {Promise<ArrayBuffer>}
 */
async function arrayBuffer(stream) {
  const ret = await blob(stream);
  return ret.arrayBuffer();
}

/**
 * @param {AsyncIterable|ReadableStream|Readable} stream
 * @returns {Promise<Buffer>}
 */
async function buffer(stream) {
  return Buffer.from(await arrayBuffer(stream));
}

/**
 * @param {AsyncIterable|ReadableStream|Readable} stream
 * @returns {Promise<string>}
 */
async function text(stream) {
  const dec = new TextDecoder();
  let str = "";
  for await (const chunk of stream) {
    if (typeof chunk === "string") {
      str += chunk;
    } else {
      str += dec.decode(chunk, { stream: true });
    }
  }
  // Flush the streaming TextDecoder so that any pending
  // incomplete multibyte characters are handled.
  str += dec.decode(undefined, { stream: false });
  return str;
}

/**
 * @param {AsyncIterable|ReadableStream|Readable} stream
 * @returns {Promise<any>}
 */
async function json(stream) {
  const str = await text(stream);
  return JSONParse(str);
}

const _defaultExport1 = {
  arrayBuffer,
  blob,
  buffer,
  text,
  json,
};

export default _defaultExport1;
export { arrayBuffer, blob, buffer, json, text };
