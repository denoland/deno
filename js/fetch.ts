// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  log,
  createResolvable,
  Resolvable,
  typedArrayToArrayBuffer,
  notImplemented
} from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendAsync } from "./dispatch";
import * as msg from "gen/msg_generated";
import * as domTypes from "./dom_types";
import { TextDecoder } from "./text_encoding";
import { DenoBlob } from "./blob";
import { Headers } from "./headers";

class FetchResponse implements domTypes.Response {
  readonly url: string = "";
  body: null;
  bodyUsed = false; // TODO
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers: domTypes.Headers;
  readonly trailer: Promise<domTypes.Headers>;
  //private bodyChunks: Uint8Array[] = [];
  private first = true;
  private bodyData: ArrayBuffer;
  private bodyWaiter: Resolvable<ArrayBuffer>;

  constructor(
    readonly status: number,
    readonly body_: ArrayBuffer,
    headersList: Array<[string, string]>
  ) {
    this.bodyWaiter = createResolvable();
    this.trailer = createResolvable();
    this.headers = new Headers(headersList);
    this.bodyData = body_;
    setTimeout(() => {
      this.bodyWaiter.resolve(body_);
    }, 0);
  }

  arrayBuffer(): Promise<ArrayBuffer> {
    return this.bodyWaiter;
  }

  async blob(): Promise<domTypes.Blob> {
    const arrayBuffer = await this.arrayBuffer();
    return new DenoBlob([arrayBuffer], {
      type: this.headers.get("content-type") || ""
    });
  }

  async formData(): Promise<domTypes.FormData> {
    notImplemented();
    return {} as domTypes.FormData;
  }

  async json(): Promise<object> {
    const text = await this.text();
    return JSON.parse(text);
  }

  async text(): Promise<string> {
    const ab = await this.arrayBuffer();
    const decoder = new TextDecoder("utf-8");
    return decoder.decode(ab);
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

    return new FetchResponse(this.status, this.bodyData.slice(0), headersList);
  }

  onHeader?: (res: FetchResponse) => void;
  onError?: (error: Error) => void;

  onMsg(base: msg.Base) {
    /*
    const error = base.error();
    if (error != null) {
      assert(this.onError != null);
      this.onError!(new Error(error));
      return;
    }
    */

    if (this.first) {
      this.first = false;
    }
  }
}

/** Fetch a resource from the network. */
export async function fetch(
  input?: domTypes.Request | string,
  init?: domTypes.RequestInit
): Promise<domTypes.Response> {
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
  const bodyArray = inner.bodyArray();
  assert(bodyArray != null);
  const body = typedArrayToArrayBuffer(bodyArray!);

  const headersList: Array<[string, string]> = [];
  const len = inner.headerKeyLength();
  for (let i = 0; i < len; ++i) {
    const key = inner.headerKey(i);
    const value = inner.headerValue(i);
    headersList.push([key, value]);
  }

  const response = new FetchResponse(status, body, headersList);
  return response;
}
