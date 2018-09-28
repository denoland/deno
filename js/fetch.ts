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

// ref: https://fetch.spec.whatwg.org/#dom-headers
export class DenoHeaders implements Headers {
  private headerMap: Map<string, string[]> = new Map();

  constructor(init?: HeadersInit) {
    if (init === null) {
      throw new TypeError("Failed to construct 'Headers': Invalid value");
    }

    if (!init) {
      return;
    }

    if (init instanceof DenoHeaders) {
      // init is the instance of Header
      init.forEach((value: string, name: string) => {
        const values = value.split(", ");
        this.headerMap.set(name, values);
      });
    } else if (Array.isArray(init)) {
      // init is a  sequence
      init.forEach(item => {
        if (item.length !== 2) {
          throw new TypeError("Failed to construct 'Headers': Invalid value");
        }
         /* tslint:disable-next-line:max-line-length */
        const [name, value] = this.normalizeParams(item[0], item[1]);
        const values = this.headerMap.get(name);
        const newValues = values ? [...values, value] : [value];
        this.headerMap.set(name, newValues);
      });
    } else {
      // init is a object
      const names = Object.keys(init);
      names.forEach(name => {
        const value = (init as Record<string, string>)[name];
        const [newname, newvalue] = this.normalizeParams(name, value);
        this.headerMap.set(newname, [newvalue]);
      });
    }
  }

  private normalizeParams(name: string, value?: string): string[] {
    name = String(name).toLowerCase();
    value = String(value).trim();
    return [name, value];
  }

  append(name: string, value: string): void {
    const [newname, newvalue] = this.normalizeParams(name, value);
    const values = this.headerMap.get(newname);
    const newValues = values ? [...values, newvalue] : [newvalue];
    this.headerMap.set(newname, newValues);
  }

  delete(name: string): void {
    const [newname] = this.normalizeParams(name);
    this.headerMap.delete(newname);
  }

  get(name: string): string | null {
    const [newname] = this.normalizeParams(name);
    const values = this.headerMap.get(newname);
    return values ? values[0] : null;
  }

  has(name: string): boolean {
    const [newname] = this.normalizeParams(name);
    return this.headerMap.has(newname);
  }

  set(name: string, value: string): void {
    const [newname, newvalue] = this.normalizeParams(name, value);
    this.headerMap.set(newname, [newvalue]);
  }

  forEach(
    callbackfn: (value: string, key: string, parent: Headers) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ): void {
    this.headerMap.forEach((values, name) => {
      const str = values.reduce((pre, cur) => `${pre}, ${cur}`);
      callbackfn(str, name, this);
    });
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
