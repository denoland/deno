// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { Closer } from "./io";
import * as msg from "gen/msg_generated";
import { assert, log } from "./util";
import * as dispatch from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { close } from "./files";

// TODO Cannot use Headers due to bug in ts_declaration_builder.
// import { Headers } from "./headers";
// import * as headers from "./headers";
// import * as domTypes from "./dom_types";

type HttpHandler = (req: ServerRequest, res: ServerResponse) => void;

export class HttpServer implements Closer {
  private _closing = false;

  constructor(readonly rid: number) {
    assert(rid >= 2); // rid should be after stdout/stderr
  }

  async serve(handler: HttpHandler): Promise<void> {
    while (this._closing === false) {
      const [req, res] = await httpAccept(this.rid);
      handler(req, res);
    }
  }

  close(): void {
    this._closing = true;
    close(this.rid);
  }
}

function deserializeHeaderFields(m: msg.HttpHeader): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  for (let i = 0; i < m.fieldsLength(); i++) {
    const item = m.fields(i)!;
    out.push([item.key()!, item.value()!]);
  }
  return out;
}

export class ServerRequest {
  // TODO Cannot do this due to ts_declaration_builder bug.
  // headers: domTypes.Headers;
  readonly headers: Array<[string, string]>;

  constructor(
    readonly rid: number,
    readonly method: string,
    readonly url: string,
    headersInit: Array<[string, string]>
  ) {
    // TODO cannot use Headers due to ts_declaration_builder bug.
    // this.headers = new Headers(headersInit);
    this.headers = headersInit;
  }
}

export class ServerResponse {
  headers = new Array<[string, string]>(); // TODO Use Headers
  status = 200;

  constructor(readonly rid: number, readonly url: string) {}

  writeResponse(body?: ArrayBufferView): void {
    httpWriteResponse(this, body);
    close(this.rid); // TODO Streaming response body.
  }
}

async function httpAccept(
  rid: number
): Promise<[ServerRequest, ServerResponse]> {
  const builder = flatbuffers.createBuilder();

  msg.HttpAccept.startHttpAccept(builder);
  msg.HttpAccept.addListenerRid(builder, rid);
  const inner = msg.HttpAccept.endHttpAccept(builder);

  const baseRes = await dispatch.sendAsync(builder, msg.Any.HttpAccept, inner);
  assert(baseRes != null);
  assert(msg.Any.HttpAcceptRes === baseRes!.innerType());
  const acceptResMsg = new msg.HttpAcceptRes();
  assert(baseRes!.inner(acceptResMsg) != null);

  const transactionRid = acceptResMsg.transactionRid();
  const header = acceptResMsg.header()!;
  const fields = deserializeHeaderFields(header);
  const url = header.url()!;
  const method = header.method()!;
  log("http accept:", method, url, fields);

  const req = new ServerRequest(transactionRid, method, url, fields);
  const res = new ServerResponse(transactionRid, url);
  return [req, res];
}

export function httpListen(address: string): HttpServer {
  const builder = flatbuffers.createBuilder();
  const address_ = builder.createString(address);

  msg.HttpListen.startHttpListen(builder);
  msg.HttpListen.addAddress(builder, address_);
  const inner = msg.HttpListen.endHttpListen(builder);

  const baseRes = dispatch.sendSync(builder, msg.Any.HttpListen, inner);
  assert(baseRes != null);
  assert(msg.Any.HttpListenRes === baseRes!.innerType());
  const res = new msg.HttpListenRes();
  assert(baseRes!.inner(res) != null);
  return new HttpServer(res.rid());
}

export function httpWriteResponse(
  res: ServerResponse,
  body?: ArrayBufferView
): void {
  const builder = flatbuffers.createBuilder();
  const fields = msg.HttpHeader.createFieldsVector(
    builder,
    res.headers.map(([key, val]) => {
      const key_ = builder.createString(key);
      const val_ = builder.createString(val);
      msg.KeyValue.startKeyValue(builder);
      msg.KeyValue.addKey(builder, key_);
      msg.KeyValue.addValue(builder, val_);
      return msg.KeyValue.endKeyValue(builder);
    })
  );
  msg.HttpHeader.startHttpHeader(builder);
  msg.HttpHeader.addFields(builder, fields);
  msg.HttpHeader.addStatus(builder, res.status);
  msg.HttpHeader.addIsRequest(builder, false);

  const header = msg.HttpHeader.endHttpHeader(builder);
  msg.HttpWriteResponse.startHttpWriteResponse(builder);
  msg.HttpWriteResponse.addTransactionRid(builder, res.rid);
  msg.HttpWriteResponse.addHeader(builder, header);
  const inner = msg.HttpWriteResponse.endHttpWriteResponse(builder);
  const r = dispatch.sendSync(builder, msg.Any.HttpWriteResponse, inner, body);
  assert(r == null);
}
