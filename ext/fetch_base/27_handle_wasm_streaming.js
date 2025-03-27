import { core, primordials } from "ext:core/mod.js";
import {
  op_wasm_streaming_feed,
  op_wasm_streaming_set_url,
} from "ext:core/ops";
const {
  PromisePrototypeThen,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  TypeError,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";


/**
 * Handle the Response argument to the WebAssembly streaming APIs, after
 * resolving if it was passed as a promise. This function should be registered
 * through `Deno.core.setWasmStreamingCallback`.
 *
 * @param {any} source The source parameter that the WebAssembly streaming API
 * was called with. If it was called with a Promise, `source` is the resolved
 * value of that promise.
 * @param {number} rid An rid that represents the wasm streaming resource.
 */
function handleWasmStreaming(source, rid) {
  // This implements part of
  // https://webassembly.github.io/spec/web-api/#compile-a-potential-webassembly-response
  try {
    const res = webidl.converters["Response"](
      source,
      "Failed to execute 'WebAssembly.compileStreaming'",
      "Argument 1",
    );

    // 2.3.
    // The spec is ambiguous here, see
    // https://github.com/WebAssembly/spec/issues/1138. The WPT tests expect
    // the raw value of the Content-Type attribute lowercased. We ignore this
    // for file:// because file fetches don't have a Content-Type.
    if (!StringPrototypeStartsWith(res.url, "file://")) {
      const contentType = res.headers.get("Content-Type");
      if (
        typeof contentType !== "string" ||
        StringPrototypeToLowerCase(contentType) !== "application/wasm"
      ) {
        throw new TypeError("Invalid WebAssembly content type");
      }
    }

    // 2.5.
    if (!res.ok) {
      throw new TypeError(
        `Failed to receive WebAssembly content: HTTP status code ${res.status}`,
      );
    }

    // Pass the resolved URL to v8.
    op_wasm_streaming_set_url(rid, res.url);

    if (res.body !== null) {
      // 2.6.
      // Rather than consuming the body as an ArrayBuffer, this passes each
      // chunk to the feed as soon as it's available.
      PromisePrototypeThen(
        (async () => {
          const reader = res.body.getReader();
          while (true) {
            const { value: chunk, done } = await reader.read();
            if (done) break;
            op_wasm_streaming_feed(rid, chunk);
          }
        })(),
        // 2.7
        () => core.close(rid),
        // 2.8
        (err) => core.abortWasmStreaming(rid, err),
      );
    } else {
      // 2.7
      core.close(rid);
    }
  } catch (err) {
    // 2.8
    core.abortWasmStreaming(rid, err);
  }
}

export { handleWasmStreaming }
