// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { TextDecoder } = core.loadExtScript("ext:deno_web/08_text_encoding.js");
const { Blob } = core.loadExtScript("ext:deno_web/09_file.js");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_STATE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  ArrayBufferIsView,
  JSONParse,
} = primordials;

function validateStreamIterator(stream) {
  try {
    return stream[Symbol.asyncIterator]();
  } catch (e) {
    if (e instanceof TypeError && e.message === "ReadableStream is locked") {
      throw new ERR_INVALID_STATE(e.message);
    } else {
      throw e;
    }
  }
}

async function blob(stream) {
  const chunks = [];
  const iter = validateStreamIterator(stream);
  for await (const chunk of iter) {
    chunks.push(chunk);
  }
  return new Blob(chunks);
}

async function arrayBuffer(stream) {
  const ret = await blob(stream);
  return ret.arrayBuffer();
}

async function buffer(stream) {
  return Buffer.from(await arrayBuffer(stream));
}

async function bytes(stream) {
  return new Uint8Array(await arrayBuffer(stream));
}

async function text(stream) {
  const dec = new TextDecoder();
  let str = "";
  const iter = validateStreamIterator(stream);
  for await (const chunk of iter) {
    if (typeof chunk === "string") {
      str += chunk;
    } else {
      if (!core.isAnyArrayBuffer(chunk) && !ArrayBufferIsView(chunk)) {
        throw new ERR_INVALID_ARG_TYPE("input", [
          "SharedArrayBuffer",
          "ArrayBuffer",
          "ArrayBufferView",
        ], chunk);
      }
      str += dec.decode(chunk, { stream: true });
    }
  }
  // Flush the streaming TextDecoder so that any pending
  // incomplete multibyte characters are handled.
  str += dec.decode(undefined, { stream: false });
  return str;
}

async function json(stream) {
  const str = await text(stream);
  return JSONParse(str);
}

return {
  arrayBuffer,
  blob,
  buffer,
  bytes,
  text,
  json,
};
})();
