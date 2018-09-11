// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  log,
  createResolvable,
  Resolvable,
  typedArrayToArrayBuffer,
  notImplemented
} from "./util";
import { flatbuffers } from "flatbuffers";
import { sendAsync } from "./dispatch";
import * as fbs from "gen/msg_generated";
import {
  Headers,
  Request,
  Response,
  Blob,
  RequestInit,
  FormData
} from "./fetch_types";
import { TextDecoder } from "./text_encoding";

class DenoHeaders implements Headers {
  append(name: string, value: string): void {
    assert(false, "Implement me");
  }
  delete(name: string): void {
    assert(false, "Implement me");
  }
  get(name: string): string | null {
    assert(false, "Implement me");
    return null;
  }
  has(name: string): boolean {
    assert(false, "Implement me");
    return false;
  }
  set(name: string, value: string): void {
    assert(false, "Implement me");
  }
  forEach(
    callbackfn: (value: string, key: string, parent: Headers) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ): void {
    assert(false, "Implement me");
  }
}

class FetchResponse implements Response {
  readonly url: string = "";
  body: null;
  bodyUsed = false; // TODO
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers = new DenoHeaders();
  readonly trailer: Promise<Headers>;
  //private bodyChunks: Uint8Array[] = [];
  private first = true;
  private bodyWaiter: Resolvable<ArrayBuffer>;

  constructor(readonly status: number, readonly body_: ArrayBuffer) {
    this.bodyWaiter = createResolvable();
    this.trailer = createResolvable();
    setTimeout(() => {
      this.bodyWaiter.resolve(body_);
    }, 0);
  }

  arrayBuffer(): Promise<ArrayBuffer> {
    return this.bodyWaiter;
  }

  async blob(): Promise<Blob> {
    notImplemented();
    return {} as Blob;
  }

  async formData(): Promise<FormData> {
    notImplemented();
    return {} as FormData;
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

  clone(): Response {
    notImplemented();
    return {} as Response;
  }

  onHeader?: (res: FetchResponse) => void;
  onError?: (error: Error) => void;

  onMsg(base: fbs.Base) {
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

export async function fetch(
  input?: Request | string,
  init?: RequestInit
): Promise<Response> {
  const url = input as string;
  log("dispatch FETCH_REQ", url);

  // Send FetchReq message
  const builder = new flatbuffers.Builder();
  const url_ = builder.createString(url);
  fbs.FetchReq.startFetchReq(builder);
  fbs.FetchReq.addUrl(builder, url_);
  const resBase = await sendAsync(
    builder,
    fbs.Any.FetchReq,
    fbs.FetchReq.endFetchReq(builder)
  );

  // Decode FetchRes
  assert(fbs.Any.FetchRes === resBase.msgType());
  const msg = new fbs.FetchRes();
  assert(resBase.msg(msg) != null);

  const status = msg.status();
  const bodyArray = msg.bodyArray();
  assert(bodyArray != null);
  const body = typedArrayToArrayBuffer(bodyArray!);

  const response = new FetchResponse(status, body);
  return response;
}
