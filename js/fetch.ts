// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  log,
  createResolvable,
  Resolvable,
  typedArrayToArrayBuffer,
  notImplemented,
  CreateIterableIterator
} from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendAsync } from "./dispatch";
import * as msg from "gen/msg_generated";
import * as domTypes from "./dom_types";
import { TextDecoder } from "./text_encoding";
import { DenoBlob } from "./blob";

// ref: https://fetch.spec.whatwg.org/#dom-headers
export class DenoHeaders implements domTypes.Headers {
  private headerMap: Map<string, string> = new Map();

  constructor(init?: domTypes.HeadersInit) {
    if (arguments.length === 0 || init === undefined) {
      return;
    }

    if (init instanceof DenoHeaders) {
      // init is the instance of Header
      init.forEach((value: string, name: string) => {
        this.headerMap.set(name, value);
      });
    } else if (Array.isArray(init)) {
      // init is a sequence
      init.forEach(item => {
        if (item.length !== 2) {
          throw new TypeError("Failed to construct 'Headers': Invalid value");
        }
        const [name, value] = this.normalizeParams(item[0], item[1]);
        const v = this.headerMap.get(name);
        const str = v ? `${v}, ${value}` : value;
        this.headerMap.set(name, str);
      });
    } else if (Object.prototype.toString.call(init) === "[object Object]") {
      // init is a object
      const names = Object.keys(init);
      names.forEach(name => {
        const value = (init as Record<string, string>)[name];
        const [newname, newvalue] = this.normalizeParams(name, value);
        this.headerMap.set(newname, newvalue);
      });
    } else {
      throw new TypeError("Failed to construct 'Headers': Invalid value");
    }
  }

  private normalizeParams(name: string, value?: string): string[] {
    name = String(name).toLowerCase();
    value = String(value).trim();
    return [name, value];
  }

  append(name: string, value: string): void {
    const [newname, newvalue] = this.normalizeParams(name, value);
    const v = this.headerMap.get(newname);
    const str = v ? `${v}, ${newvalue}` : newvalue;
    this.headerMap.set(newname, str);
  }

  delete(name: string): void {
    const [newname] = this.normalizeParams(name);
    this.headerMap.delete(newname);
  }

  entries(): IterableIterator<[string, string]> {
    const iterators = this.headerMap.entries();
    return new CreateIterableIterator(iterators);
  }

  get(name: string): string | null {
    const [newname] = this.normalizeParams(name);
    const value = this.headerMap.get(newname);
    return value || null;
  }

  has(name: string): boolean {
    const [newname] = this.normalizeParams(name);
    return this.headerMap.has(newname);
  }

  keys(): IterableIterator<string> {
    const iterators = this.headerMap.keys();
    return new CreateIterableIterator(iterators); 
  }

  set(name: string, value: string): void {
    const [newname, newvalue] = this.normalizeParams(name, value);
    this.headerMap.set(newname, newvalue);
  }

  values(): IterableIterator<string> {
    const iterators = this.headerMap.values();
    return new CreateIterableIterator(iterators); 
  }

  forEach(
    callbackfn: (value: string, key: string, parent: domTypes.Headers) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ): void {
    this.headerMap.forEach((value, name) => {
      callbackfn(value, name, this);
    });
  }

  [Symbol.iterator](): IterableIterator<[string, string]> {
    return this.entries();
  }
}

class FetchResponse implements domTypes.Response {
  readonly url: string = "";
  body: null;
  bodyUsed = false; // TODO
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers: DenoHeaders;
  readonly trailer: Promise<domTypes.Headers>;
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
    notImplemented();
    return {} as domTypes.Response;
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

  // Send FetchReq message
  const builder = flatbuffers.createBuilder();
  const url_ = builder.createString(url);
  msg.FetchReq.startFetchReq(builder);
  msg.FetchReq.addUrl(builder, url_);
  const resBase = await sendAsync(
    builder,
    msg.Any.FetchReq,
    msg.FetchReq.endFetchReq(builder)
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
