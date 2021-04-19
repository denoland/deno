// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { InnerBody } = window.__bootstrap.fetchBody;
  const { Response, fromInnerRequest, toInnerResponse, newInnerRequest } =
    window.__bootstrap.fetch;
  const errors = window.__bootstrap.errors.errors;
  const core = window.Deno.core;
  const { ReadableStream } = window.__bootstrap.streams;

  function serveHttp(conn) {
    const rid = Deno.core.opSync("op_http_start", conn.rid);
    return new HttpConn(rid);
  }

  class HttpConn {
    #rid = 0;

    constructor(rid) {
      this.#rid = rid;
    }

    /** @returns {number} */
    get rid() {
      return this.#rid;
    }

    /** @returns {Promise<ResponseEvent | null>} */
    async nextRequest() {
      let nextRequest;
      try {
        nextRequest = await Deno.core.opAsync(
          "op_http_request_next",
          this.#rid,
        );
      } catch (error) {
        if (error instanceof errors.BadResource) {
          return null;
        } else if (error instanceof errors.Interrupted) {
          return null;
        }
        throw error;
      }
      if (nextRequest === null) return null;

      const [
        requestBodyRid,
        responseSenderRid,
        method,
        headersList,
        url,
      ] = nextRequest;

      /** @type {ReadableStream<Uint8Array> | undefined} */
      let body = null;
      if (typeof requestBodyRid === "number") {
        body = createRequestBodyStream(requestBodyRid);
      }

      const innerRequest = newInnerRequest(
        method,
        new URL(url),
        headersList,
        body !== null ? new InnerBody(body) : null,
      );
      const request = fromInnerRequest(innerRequest, "immutable");

      const respondWith = createRespondWith(responseSenderRid, this.#rid);

      return { request, respondWith };
    }

    /** @returns {void} */
    close() {
      core.close(this.#rid);
    }

    [Symbol.asyncIterator]() {
      const httpConn = this;
      return {
        async next() {
          const reqEvt = await httpConn.nextRequest();
          // Change with caution, current form avoids a v8 deopt
          return { value: reqEvt, done: reqEvt === null };
        },
      };
    }
  }

  function readRequest(requestRid, zeroCopyBuf) {
    return Deno.core.opAsync(
      "op_http_request_read",
      requestRid,
      zeroCopyBuf,
    );
  }

  /** IMPORTANT: Equivalent to `Array.from(headers).flat()` but more performant.
   * Please preserve. */
  function flattenHeaders(headers) {
    const array = [];
    for (const pair of headers) {
      array.push(pair[0], pair[1]);
    }
    return array;
  }

  function createRespondWith(responseSenderRid) {
    return async function respondWith(resp) {
      if (resp instanceof Promise) {
        resp = await resp;
      }

      if (!(resp instanceof Response)) {
        throw new TypeError(
          "First argument to respondWith must be a Response or a promise resolving to a Response.",
        );
      }

      const innerResp = toInnerResponse(resp);

      // If response body length is known, it will be sent synchronously in a
      // single op, in other case a "response body" resource will be created and
      // we'll be streaming it.
      // TODO(lucacasonato): fast-path statically known body (somehow skip stream)
      /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
      let respBody = null;
      if (innerResp.body !== null) {
        if (innerResp.body.length === null) {
          respBody = innerResp.body.stream;
        } else {
          const reader = innerResp.body.stream.getReader();
          const r1 = await reader.read();
          if (r1.done) throw new TypeError("Unreachable");
          respBody = r1.value;
          const r2 = await reader.read();
          if (!r2.done) throw new TypeError("Unreachable");
        }
      }

      const responseBodyRid = await Deno.core.opAsync("op_http_response", [
        responseSenderRid,
        resp.status ?? 200,
        flattenHeaders(resp.headers),
      ], respBody instanceof Uint8Array ? respBody : null);

      // If `respond` returns a responseBodyRid, we should stream the body
      // to that resource.
      if (responseBodyRid !== null) {
        if (respBody === null || !(respBody instanceof ReadableStream)) {
          throw new TypeError("Unreachable");
        }
        const reader = respBody.getReader();
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          if (!(value instanceof Uint8Array)) {
            await reader.cancel("value not a Uint8Array");
            break;
          }
          try {
            await Deno.core.opAsync(
              "op_http_response_write",
              responseBodyRid,
              value,
            );
          } catch (err) {
            await reader.cancel(err);
            break;
          }
        }
        // Once all chunks are sent, and the request body is closed, we can close
        // the response body.
        await Deno.core.opAsync("op_http_response_close", responseBodyRid);
      }
    };
  }

  function createRequestBodyStream(requestBodyRid) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await readRequest(
            requestBodyRid,
            chunk,
          );
          if (read > 0) {
            // We read some data. Enqueue it onto the stream.
            controller.enqueue(chunk.subarray(0, read));
          } else {
            // We have reached the end of the body, so we close the stream.
            controller.close();
            core.close(requestBodyRid);
          }
        } catch (err) {
          // There was an error while reading a chunk of the body, so we
          // error.
          controller.error(err);
          controller.close();
          core.close(requestBodyRid);
        }
      },
      cancel() {
        core.close(requestBodyRid);
      },
    });
  }

  window.__bootstrap.http = {
    serveHttp,
  };
})(this);
