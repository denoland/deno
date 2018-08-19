// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  log,
  createResolvable,
  Resolvable,
  typedArrayToArrayBuffer,
  notImplemented,
} from "./util";
import { pubInternal, sub } from "./dispatch";
import { deno as pb } from "./msg.pb";

export function initFetch() {
  sub("fetch", (payload: Uint8Array) => {
    const msg = pb.Msg.decode(payload);
    assert(msg.command === pb.Msg.Command.FETCH_RES);
    const id = msg.fetchResId;
    const f = fetchRequests.get(id);
    assert(f != null, `Couldn't find FetchRequest id ${id}`);

    f.onMsg(msg);
  });
}

const fetchRequests = new Map<number, FetchRequest>();

class FetchResponse implements Response {
  readonly url: string;
  body: null;
  bodyUsed = false; // TODO
  status: number;
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers: null; // TODO
  //private bodyChunks: Uint8Array[] = [];
  private first = true;

  constructor(readonly req: FetchRequest) {
    this.url = req.url;
  }

  bodyWaiter: Resolvable<ArrayBuffer>;
  arrayBuffer(): Promise<ArrayBuffer> {
    this.bodyWaiter = createResolvable();
    return this.bodyWaiter;
  }

  blob(): Promise<Blob> {
    notImplemented();
  }

  formData(): Promise<FormData> {
    notImplemented();
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
  }

  onHeader: (res: Response) => void;
  onError: (error: Error) => void;

  onMsg(msg: pb.Msg) {
    if (msg.error !== null && msg.error !== "") {
      //throw new Error(msg.error)
      this.onError(new Error(msg.error));
      return;
    }

    if (this.first) {
      this.first = false;
      this.status = msg.fetchResStatus;
      this.onHeader(this);
    } else {
      // Body message. Assuming it all comes in one message now.
      const ab = typedArrayToArrayBuffer(msg.fetchResBody);
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

  onMsg(msg: pb.Msg) {
    this.response.onMsg(msg);
  }

  destroy() {
    fetchRequests.delete(this.id);
  }

  start() {
    log("dispatch FETCH_REQ", this.id, this.url);
    const res = pubInternal("fetch", {
      command: pb.Msg.Command.FETCH_REQ,
      fetchReqId: this.id,
      fetchReqUrl: this.url
    });
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
    // tslint:disable-next-line:no-any
    response.onHeader = (response: any) => {
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
