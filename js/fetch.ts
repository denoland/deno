// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { assert, log, createResolvable, notImplemented } from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendAsync } from "./dispatch";
import * as msg from "gen/msg_generated";
import * as domTypes from "./dom_types";
import { TextDecoder } from "./text_encoding";
import { DenoBlob } from "./blob";
import { Headers } from "./headers";
import * as io from "./io";
import { read, close } from "./files";
import { Buffer } from "./buffer";

class Body implements domTypes.Body, domTypes.ReadableStream, io.ReadCloser {
  bodyUsed = false;
  private _bodyPromise: null | Promise<ArrayBuffer> = null;
  private _data: ArrayBuffer | null = null;
  readonly locked: boolean = false; // TODO
  readonly body: null | Body = this;

  constructor(private rid: number, readonly contentType: string) {}

  private async _bodyBuffer(): Promise<ArrayBuffer> {
    assert(this._bodyPromise == null);
    const buf = new Buffer();
    try {
      const nread = await buf.readFrom(this);
      const ui8 = buf.bytes();
      assert(ui8.byteLength === nread);
      this._data = ui8.buffer.slice(
        ui8.byteOffset,
        ui8.byteOffset + nread
      ) as ArrayBuffer;
      assert(this._data.byteLength === nread);
    } finally {
      this.close();
    }

    return this._data;
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    // If we've already bufferred the response, just return it.
    if (this._data != null) {
      return this._data;
    }

    // If there is no _bodyPromise yet, start it.
    if (this._bodyPromise == null) {
      this._bodyPromise = this._bodyBuffer();
    }

    return this._bodyPromise;
  }

  async blob(): Promise<domTypes.Blob> {
    const arrayBuffer = await this.arrayBuffer();
    return new DenoBlob([arrayBuffer], {
      type: this.contentType
    });
  }

  async formData(): Promise<domTypes.FormData> {
    return notImplemented();
  }

  // tslint:disable-next-line:no-any
  async json(): Promise<any> {
    const text = await this.text();
    return JSON.parse(text);
  }

  async text(): Promise<string> {
    const ab = await this.arrayBuffer();
    const decoder = new TextDecoder("utf-8");
    return decoder.decode(ab);
  }

  read(p: Uint8Array): Promise<io.ReadResult> {
    return read(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }

  async cancel(): Promise<void> {
    return notImplemented();
  }

  getReader(): domTypes.ReadableStreamReader {
    return notImplemented();
  }
}

class Response implements domTypes.Response {
  readonly url: string = "";
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers: domTypes.Headers;
  readonly trailer: Promise<domTypes.Headers>;
  bodyUsed = false;
  readonly body: Body;

  constructor(
    readonly status: number,
    headersList: Array<[string, string]>,
    rid: number,
    body_: null | Body = null
  ) {
    this.trailer = createResolvable();
    this.headers = new Headers(headersList);
    const contentType = this.headers.get("content-type") || "";

    if (body_ == null) {
      this.body = new Body(rid, contentType);
    } else {
      this.body = body_;
    }
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    return this.body.arrayBuffer();
  }

  async blob(): Promise<domTypes.Blob> {
    return this.body.blob();
  }

  async formData(): Promise<domTypes.FormData> {
    return this.body.formData();
  }

  // tslint:disable-next-line:no-any
  async json(): Promise<any> {
    return this.body.json();
  }

  async text(): Promise<string> {
    return this.body.text();
  }

  get ok(): boolean {
    return 200 <= this.status && this.status < 300;
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

    return new Response(this.status, headersList, -1, this.body);
  }
}

/** Fetch a resource from the network. */
export async function fetch(
  input?: domTypes.Request | string,
  init?: domTypes.RequestInit
): Promise<Response> {
  const url = input as string;
  log("dispatch FETCH_REQ", url);

  // Send Fetch message
  const builder = flatbuffers.createBuilder();
  const url_ = builder.createString(url);
  msg.Fetch.startFetch(builder);
  msg.Fetch.addUrl(builder, url_);
  const resBase = await sendAsync(
    builder,
    msg.Any.Fetch,
    msg.Fetch.endFetch(builder)
  );

  // Decode FetchRes
  assert(msg.Any.FetchRes === resBase.innerType());
  const inner = new msg.FetchRes();
  assert(resBase.inner(inner) != null);

  const status = inner.status();
  const bodyRid = inner.bodyRid();

  const headersList: Array<[string, string]> = [];
  const len = inner.headerKeyLength();
  for (let i = 0; i < len; ++i) {
    const key = inner.headerKey(i);
    const value = inner.headerValue(i);
    headersList.push([key, value]);
  }

  const response = new Response(status, headersList, bodyRid);
  return response;
}
