// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { createResolvable, notImplemented, isTypedArray } from "./util.ts";
import * as body from "./body.ts";
import * as domTypes from "./dom_types.ts";
import { TextEncoder } from "./text_encoding.ts";
import { DenoBlob, bytesSymbol as blobBytesSymbol } from "./blob.ts";
import { Headers } from "./headers.ts";
import { EOF } from "./io.ts";
import { read, close } from "./files.ts";
import { URLSearchParams } from "./url_search_params.ts";
import * as dispatch from "./dispatch.ts";
import { sendAsync } from "./dispatch_json.ts";
import { ReadableStream } from "./streams/mod.ts";

interface ReadableStreamController {
  enqueue(chunk: string | ArrayBuffer): void;
  close(): void;
}

class UnderlyingRIDSource implements domTypes.UnderlyingSource {
  constructor(private rid: number) {
    this.rid = rid;
  }

  start(controller: ReadableStreamController): Promise<void> {
    const buff: Uint8Array = new Uint8Array(32 * 1024);
    const pump = (): Promise<void> => {
      return read(this.rid, buff).then(value => {
        if (value == EOF) {
          close(this.rid);
          return controller.close();
        }
        controller.enqueue(buff.slice(0, value));
        return pump();
      });
    };
    return pump();
  }

  cancel(controller: ReadableStreamController): void {
    close(this.rid);
    return controller.close();
  }
}

class Body extends body.Body implements domTypes.ReadableStream {
  async cancel(): Promise<void> {
    if (this._stream) {
      return this._stream.cancel();
    }
    throw new Error("no stream present");
  }

  getReader(): domTypes.ReadableStreamReader {
    if (this._stream) {
      return this._stream.getReader();
    }
    throw new Error("no stream present");
  }

  get locked(): boolean {
    if (this._stream) {
      return this._stream.locked;
    }
    throw new Error("no stream present");
  }

  tee(): [domTypes.ReadableStream, domTypes.ReadableStream] {
    if (this._stream) {
      const streams = this._stream.tee();
      return [streams[0], streams[1]];
    }
    throw new Error("no stream present");
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
    //@ts-ignore
    const reader = this.body.getReader();

    return {
      [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
        return this;
      },

      async next() {
        return reader.read();
      },

      return() {
        return reader.releaseLock();
      }
    } as AsyncIterableIterator<Uint8Array>;
  }
}

export class Response implements domTypes.Response {
  readonly type = "basic"; // TODO
  readonly redirected: boolean;
  headers: domTypes.Headers;
  readonly trailer: Promise<domTypes.Headers>;
  protected _body: Body;

  constructor(
    readonly url: string,
    readonly status: number,
    readonly statusText: string,
    headersList: Array<[string, string]>,
    rid: number,
    redirected_: boolean,
    readableStream_: domTypes.ReadableStream | null = null
  ) {
    this.trailer = createResolvable();
    this.headers = new Headers(headersList);
    const contentType = this.headers.get("content-type") || "";

    if (readableStream_ == null) {
      const underlyingSource = new UnderlyingRIDSource(rid);
      const rs = new ReadableStream(underlyingSource);
      this._body = new Body(rs, contentType);
    } else {
      this._body = new Body(readableStream_, contentType);
    }

    this.redirected = redirected_;
  }

  get body(): domTypes.ReadableStream | null {
    return this._body;
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    return this._body.arrayBuffer();
  }

  async blob(): Promise<domTypes.Blob> {
    return this._body.blob().then(blob => {
      return blob;
    });
  }

  async formData(): Promise<domTypes.FormData> {
    return this._body.formData();
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async json(): Promise<any> {
    return this._body.json();
  }

  async text(): Promise<string> {
    return this._body.text();
  }

  get ok(): boolean {
    return 200 <= this.status && this.status < 300;
  }

  get bodyUsed(): boolean {
    return this._body.bodyUsed;
  }

  clone(): domTypes.Response {
    if (this.bodyUsed) {
      throw new TypeError(
        "Failed to execute 'clone' on 'Response': Response body is already used"
      );
    }

    const iterators = this.headers.entries();
    const headersList: Array<[string, string]> = [];
    for (const header of iterators) {
      headersList.push(header);
    }

    let clonedStream: domTypes.ReadableStream | null = null;
    if (this._body.body) {
      const streams = this._body.body.tee();
      clonedStream = streams[1];
      this._body = new Body(streams[0], this._body.contentType);
    }

    return new Response(
      this.url,
      this.status,
      this.statusText,
      headersList,
      -1,
      this.redirected,
      clonedStream
    );
  }
}

interface FetchResponse {
  bodyRid: number;
  status: number;
  statusText: string;
  headers: Array<[string, string]>;
}

async function sendFetchReq(
  url: string,
  method: string | null,
  headers: domTypes.Headers | null,
  body: ArrayBufferView | undefined
): Promise<FetchResponse> {
  let headerArray: Array<[string, string]> = [];
  if (headers) {
    headerArray = Array.from(headers.entries());
  }

  let zeroCopy = undefined;
  if (body) {
    zeroCopy = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  }

  const args = {
    method,
    url,
    headers: headerArray
  };

  return (await sendAsync(dispatch.OP_FETCH, args, zeroCopy)) as FetchResponse;
}

/** Fetch a resource from the network. */
export async function fetch(
  input: domTypes.Request | string,
  init?: domTypes.RequestInit
): Promise<Response> {
  let url: string;
  let method: string | null = null;
  let headers: domTypes.Headers | null = null;
  let body: ArrayBufferView | undefined;
  let redirected = false;
  let remRedirectCount = 20; // TODO: use a better way to handle

  if (typeof input === "string") {
    url = input;
    if (init != null) {
      method = init.method || null;
      if (init.headers) {
        headers =
          init.headers instanceof Headers
            ? init.headers
            : new Headers(init.headers);
      } else {
        headers = null;
      }

      // ref: https://fetch.spec.whatwg.org/#body-mixin
      // Body should have been a mixin
      // but we are treating it as a separate class
      if (init.body) {
        if (!headers) {
          headers = new Headers();
        }
        let contentType = "";
        if (typeof init.body === "string") {
          body = new TextEncoder().encode(init.body);
          contentType = "text/plain;charset=UTF-8";
        } else if (isTypedArray(init.body)) {
          body = init.body;
        } else if (init.body instanceof URLSearchParams) {
          body = new TextEncoder().encode(init.body.toString());
          contentType = "application/x-www-form-urlencoded;charset=UTF-8";
        } else if (init.body instanceof DenoBlob) {
          body = init.body[blobBytesSymbol];
          contentType = init.body.type;
        } else {
          // TODO: FormData, ReadableStream
          notImplemented();
        }
        if (contentType && !headers.has("content-type")) {
          headers.set("content-type", contentType);
        }
      }
    }
  } else {
    url = input.url;
    method = input.method;
    headers = input.headers;

    //@ts-ignore
    if (input._bodySource) {
      body = new DataView(await input.arrayBuffer());
    }
  }

  while (remRedirectCount) {
    const fetchResponse = await sendFetchReq(url, method, headers, body);

    const response = new Response(
      url,
      fetchResponse.status,
      fetchResponse.statusText,
      fetchResponse.headers,
      fetchResponse.bodyRid,
      redirected
    );
    if ([301, 302, 303, 307, 308].includes(response.status)) {
      // We're in a redirect status
      switch ((init && init.redirect) || "follow") {
        case "error":
          throw notImplemented();
        case "manual":
          throw notImplemented();
        case "follow":
        default:
          let redirectUrl = response.headers.get("Location");
          if (redirectUrl == null) {
            return response; // Unspecified
          }
          if (
            !redirectUrl.startsWith("http://") &&
            !redirectUrl.startsWith("https://")
          ) {
            redirectUrl =
              url.split("//")[0] +
              "//" +
              url.split("//")[1].split("/")[0] +
              redirectUrl; // TODO: handle relative redirection more gracefully
          }
          url = redirectUrl;
          redirected = true;
          remRedirectCount--;
      }
    } else {
      return response;
    }
  }
  // Return a network error due to too many redirections
  throw notImplemented();
}
