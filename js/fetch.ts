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
import { send } from "./fbs_util";
import { deno as fbs } from "gen/msg_generated";
import {
  Headers,
  Request,
  Response,
  Blob,
  RequestInit,
  FormData
} from "./fetch_types";
import { TextDecoder } from "./text_encoding";

/** @internal */
export function onFetchRes(base: fbs.Base, msg: fbs.FetchRes) {
  const id = msg.id();
  const req = fetchRequests.get(id);
  assert(req != null, `Couldn't find FetchRequest id ${id}`);
  req!.onMsg(base, msg);
}

const fetchRequests = new Map<number, FetchRequest>();

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
  readonly url: string;
  body: null;
  bodyUsed = false; // TODO
  status = 0;
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers = new DenoHeaders();
  readonly trailer: Promise<Headers>;
  //private bodyChunks: Uint8Array[] = [];
  private first = true;
  private bodyWaiter: Resolvable<ArrayBuffer>;

  constructor(readonly req: FetchRequest) {
    this.url = req.url;
    this.bodyWaiter = createResolvable();
    this.trailer = createResolvable();
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

  onMsg(base: fbs.Base, msg: fbs.FetchRes) {
    const error = base.error();
    if (error != null) {
      assert(this.onError != null);
      this.onError!(new Error(error));
      return;
    }

    if (this.first) {
      this.first = false;
      this.status = msg.status();
      assert(this.onHeader != null);
      this.onHeader!(this);
    } else {
      // Body message. Assuming it all comes in one message now.
      const bodyArray = msg.bodyArray();
      assert(bodyArray != null);
      const ab = typedArrayToArrayBuffer(bodyArray!);
      this.bodyWaiter.resolve(ab);
    }
  }
}

let nextFetchId = 0;
//TODO implements Request
class FetchRequest {
  private readonly id: number;
  response: FetchResponse;
  constructor(readonly url: string) {
    this.id = nextFetchId++;
    fetchRequests.set(this.id, this);
    this.response = new FetchResponse(this);
  }

  onMsg(base: fbs.Base, msg: fbs.FetchRes) {
    this.response.onMsg(base, msg);
  }

  destroy() {
    fetchRequests.delete(this.id);
  }

  start() {
    log("dispatch FETCH_REQ", this.id, this.url);

    // Send FetchReq message
    const builder = new flatbuffers.Builder();
    const url = builder.createString(this.url);
    fbs.FetchReq.startFetchReq(builder);
    fbs.FetchReq.addId(builder, this.id);
    fbs.FetchReq.addUrl(builder, url);
    const msg = fbs.FetchReq.endFetchReq(builder);
    const res = send(builder, fbs.Any.FetchReq, msg);
    assert(res == null);
  }
}

export function fetch(
  input?: Request | string,
  init?: RequestInit
): Promise<Response> {
  const fetchReq = new FetchRequest(input as string);
  const response = fetchReq.response;
  return new Promise((resolve, reject) => {
    response.onHeader = (response: FetchResponse) => {
      log("onHeader");
      resolve(response);
    };
    response.onError = (error: Error) => {
      log("onError", error);
      reject(error);
    };
    fetchReq.start();
  });
}
