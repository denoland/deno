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
  HeadersInit,
  FormData
} from "./dom_types";
import { TextDecoder } from "./text_encoding";
import { DenoBlob } from "./blob";

interface Header {
  name: string;
  value: string;
}

export class DenoHeaders implements Headers {
  private readonly headerList: Header[] = [];

  constructor(init?: HeadersInit) {
    if (init) {
      this._fill(init);
    }
  }

  private _append(header: Header): void {
    // TODO(qti3e) Check header based on the fetch spec.
    this._appendToHeaderList(header);
  }

  private _appendToHeaderList(header: Header): void {
    const lowerCaseName = header.name.toLowerCase();
    for (let i = 0; i < this.headerList.length; ++i) {
      if (this.headerList[i].name.toLowerCase() === lowerCaseName) {
        header.name = this.headerList[i].name;
      }
    }
    this.headerList.push(header);
  }

  private _fill(init: HeadersInit): void {
    if (Array.isArray(init)) {
      for (let i = 0; i < init.length; ++i) {
        const header = init[i];
        if (header.length !== 2) {
          throw new TypeError("Failed to construct 'Headers': Invalid value");
        }
        this._append({
          name: header[0],
          value: header[1]
        });
      }
    } else {
      for (const key in init) {
        this._append({
          name: key,
          value: init[key]
        });
      }
    }
  }

  append(name: string, value: string): void {
    this._appendToHeaderList({ name, value });
  }

  delete(name: string): void {
    assert(false, "Implement me");
  }
  get(name: string): string | null {
    for (const header of this.headerList) {
      if (header.name.toLowerCase() === name.toLowerCase()) {
        return header.value;
      }
    }
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
  headers: DenoHeaders;
  readonly trailer: Promise<Headers>;
  //private bodyChunks: Uint8Array[] = [];
  private first = true;
  private bodyWaiter: Resolvable<ArrayBuffer>;

  constructor(
    readonly status: number,
    readonly body_: ArrayBuffer,
    headersList: Array<[string, string]>
  ) {
    this.bodyWaiter = createResolvable();
    this.trailer = createResolvable();
    this.headers = new DenoHeaders(headersList);
    setTimeout(() => {
      this.bodyWaiter.resolve(body_);
    }, 0);
  }

  arrayBuffer(): Promise<ArrayBuffer> {
    return this.bodyWaiter;
  }

  async blob(): Promise<Blob> {
    const arrayBuffer = await this.arrayBuffer();
    return new DenoBlob([arrayBuffer], {
      type: this.headers.get("content-type") || ""
    });
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

  const headersList: Array<[string, string]> = [];
  const len = msg.headerKeyLength();
  for (let i = 0; i < len; ++i) {
    const key = msg.headerKey(i);
    const value = msg.headerValue(i);
    headersList.push([key, value]);
  }

  const response = new FetchResponse(status, body, headersList);
  return response;
}
