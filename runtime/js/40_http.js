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

  const connErrorSymbol = Symbol("connError");

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
        // A connection error seen here would cause disrupted responses to throw
        // a generic `BadResource` error. Instead store this error and replace
        // those with it.
        this[connErrorSymbol] = error;
        if (error instanceof errors.BadResource) {
          return null;
        } else if (error instanceof errors.Interrupted) {
          return null;
        } else if (error.message.includes("connection closed")) {
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
        url,
        headersList,
        body !== null ? new InnerBody(body) : null,
      );
      const request = fromInnerRequest(innerRequest, "immutable");

      const respondWith = createRespondWith(this, responseSenderRid);

      return { request, respondWith };
    }

    /** @returns {void} */
    close() {
      core.close(this.#rid);
    }

    [Symbol.asyncIterator]() {
      // deno-lint-ignore no-this-alias
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

  function createRespondWith(httpConn, responseSenderRid) {
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
      /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
      let respBody = null;
      if (innerResp.body !== null) {
        if (innerResp.body.unusable()) throw new TypeError("Body is unusable.");
        if (innerResp.body.streamOrStatic instanceof ReadableStream) {
          if (innerResp.body.length === null) {
            respBody = innerResp.body.stream;
          } else {
            const reader = innerResp.body.stream.getReader();
            const r1 = await reader.read();
            if (r1.done) {
              respBody = new Uint8Array(0);
            } else {
              respBody = r1.value;
              const r2 = await reader.read();
              if (!r2.done) throw new TypeError("Unreachable");
            }
          }
        } else {
          innerResp.body.streamOrStatic.consumed = true;
          respBody = innerResp.body.streamOrStatic.body;
        }
      } else {
        respBody = new Uint8Array(0);
      }

      let responseBodyRid;
      try {
        responseBodyRid = await Deno.core.opAsync("op_http_response", [
          responseSenderRid,
          innerResp.status ?? 200,
          innerResp.headerList,
        ], respBody instanceof Uint8Array ? respBody : null);
      } catch (error) {
        const connError = httpConn[connErrorSymbol];
        if (error instanceof errors.BadResource && connError != null) {
          // deno-lint-ignore no-ex-assign
          error = new connError.constructor(connError.message);
        }
        if (respBody !== null && respBody instanceof ReadableStream) {
          await respBody.cancel(error);
        }
        throw error;
      }

      // If `respond` returns a responseBodyRid, we should stream the body
      // to that resource.
      if (responseBodyRid !== null) {
        try {
          if (respBody === null || !(respBody instanceof ReadableStream)) {
            throw new TypeError("Unreachable");
          }
          const reader = respBody.getReader();
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            if (!(value instanceof Uint8Array)) {
              await reader.cancel(new TypeError("Value not a Uint8Array"));
              break;
            }
            try {
              await Deno.core.opAsync(
                "op_http_response_write",
                responseBodyRid,
                value,
              );
            } catch (error) {
              const connError = httpConn[connErrorSymbol];
              if (error instanceof errors.BadResource && connError != null) {
                // deno-lint-ignore no-ex-assign
                error = new connError.constructor(connError.message);
              }
              await reader.cancel(error);
              throw error;
            }
          }
        } finally {
          // Once all chunks are sent, and the request body is closed, we can
          // close the response body.
          try {
            await Deno.core.opAsync("op_http_response_close", responseBodyRid);
          } catch { /* pass */ }
        }
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
