// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "ext:deno_node/events.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { Socket } from "ext:deno_node/net.ts";
import { TypedArray } from "ext:deno_node/internal/util/types.ts";

export class Http2Session extends EventEmitter {
  constructor() {
    super();
  }

  get alpnProtocol(): string | undefined {
    notImplemented("Http2Session.alpnProtocol");
    return undefined;
  }

  close(_callback?: () => void) {
    notImplemented("Http2Session.close");
  }

  get closed(): boolean {
    notImplemented("Http2Session.closed");
    return false;
  }

  get connecting(): boolean {
    notImplemented("Http2Session.connecting");
    return false;
  }

  destroy(_error?: Error, _code?: number) {
    notImplemented("Http2Session.destroy");
  }

  get destroyed(): boolean {
    notImplemented("Http2Session.destroyed");
    return false;
  }

  get encrypted(): boolean {
    notImplemented("Http2Session.encrypted");
    return false;
  }

  goaway(
    _code: number,
    _lastStreamID: number,
    _opaqueData: Buffer | TypedArray | DataView,
  ) {
    notImplemented("Http2Session.goaway");
  }

  get localSettings(): Record<string, unknown> {
    notImplemented("Http2Session.localSettings");
    return {};
  }

  get originSet(): string[] | undefined {
    notImplemented("Http2Session.originSet");
    return undefined;
  }

  get pendingSettingsAck(): boolean {
    notImplemented("Http2Session.pendingSettingsAck");
    return false;
  }

  ping(
    _payload: Buffer | TypedArray | DataView,
    _callback: () => void,
  ): boolean {
    notImplemented("Http2Session.ping");
    return false;
  }

  ref() {
    notImplemented("Http2Session.ref");
  }

  get remoteSettings(): Record<string, unknown> {
    notImplemented("Http2Session.remoteSettings");
    return {};
  }

  setLocalWindowSize(_windowSize: number) {
    notImplemented("Http2Session.setLocalWindowSize");
  }

  setTimeout(_msecs: number, _callback: () => void) {
    notImplemented("Http2Session.setTimeout");
  }

  get socket(): Socket /*| TlsSocket*/ {
    notImplemented("Http2Session.socket");
    return null;
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Session.state");
    return {};
  }

  settings(_settings: Record<string, unknown>, _callback: () => void) {
    notImplemented("Http2Session.settings");
  }

  get type(): number {
    notImplemented("Http2Session.type");
    return 0;
  }

  unref() {
    notImplemented("Http2Session.unref");
  }
}

export class ServerHttp2Session extends Http2Session {
  constructor() {
    super();
  }

  altsvc(
    _alt: string,
    _originOrStream: number | string | URL | { origin: string },
  ) {
    notImplemented("ServerHttp2Session.altsvc");
  }

  origin(..._origins: (string | URL | { origin: string })[]) {
    notImplemented("ServerHttp2Session.origins");
  }
}

export class ClientHttp2Session extends Http2Session {
  constructor() {
    super();
  }

  request(
    _headers: Record<string, unknown>,
    _options?: Record<string, unknown>,
  ): ClientHttp2Stream {
    notImplemented("ClientHttp2Session.request");
    return new ClientHttp2Stream();
  }
}

export class Http2Stream {
  constructor() {
  }

  get aborted(): boolean {
    notImplemented("Http2Stream.aborted");
    return false;
  }

  get bufferSize(): number {
    notImplemented("Http2Stream.bufferSize");
    return 0;
  }

  close(_code: number, _callback: () => void) {
    notImplemented("Http2Stream.close");
  }

  get closed(): boolean {
    notImplemented("Http2Stream.closed");
    return false;
  }

  get destroyed(): boolean {
    notImplemented("Http2Stream.destroyed");
    return false;
  }

  get endAfterHeaders(): boolean {
    notImplemented("Http2Stream.endAfterHeaders");
    return false;
  }

  get id(): number | undefined {
    notImplemented("Http2Stream.id");
    return undefined;
  }

  get pending(): boolean {
    notImplemented("Http2Stream.pending");
    return false;
  }

  priority(_options: Record<string, unknown>) {
    notImplemented("Http2Stream.priority");
  }

  get rstCode(): number {
    notImplemented("Http2Stream.rstCode");
    return 0;
  }

  get sentHeaders(): boolean {
    notImplemented("Http2Stream.sentHeaders");
    return false;
  }

  get sentInfoHeaders(): Record<string, unknown> {
    notImplemented("Http2Stream.sentInfoHeaders");
    return {};
  }

  get sentTrailers(): Record<string, unknown> {
    notImplemented("Http2Stream.sentTrailers");
    return {};
  }

  get session(): Http2Session {
    notImplemented("Http2Stream.session");
    return new Http2Session();
  }

  setTimeout(_msecs: number, _callback: () => void) {
    notImplemented("Http2Stream.setTimeout");
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Stream.state");
    return {};
  }

  sendTrailers(_headers: Record<string, unknown>) {
    notImplemented("Http2Stream.sendTrailers");
  }
}

export class ClientHttp2Stream {
  constructor() {
    notImplemented("ClientHttp2Stream");
  }
}
export class ServerHttp2Stream {
  constructor() {
    notImplemented("ServerHttp2Stream");
  }
}
export class Http2Server {
  constructor() {
    notImplemented("Http2Server");
  }
}
export class Http2SecureServer {
  constructor() {
    notImplemented("Http2SecureServer");
  }
}
export function createServer() {}
export function createSecureServer() {}
export function connect() {}
export const constants = {};
export function getDefaultSettings() {}
export function getPackedSettings() {}
export function getUnpackedSettings() {}
export const sensitiveHeaders = Symbol("nodejs.http2.sensitiveHeaders");
export class Http2ServerRequest {
  constructor() {
    notImplemented("Http2ServerRequest");
  }
}
export class Http2ServerResponse {
  constructor() {
    notImplemented("Http2ServerResponse");
  }
}
export default {
  Http2Session,
  ServerHttp2Session,
  ClientHttp2Session,
  Http2Stream,
  ClientHttp2Stream,
  ServerHttp2Stream,
  Http2Server,
  Http2SecureServer,
  createServer,
  createSecureServer,
  connect,
  constants,
  getDefaultSettings,
  getPackedSettings,
  getUnpackedSettings,
  sensitiveHeaders,
  Http2ServerRequest,
  Http2ServerResponse,
};
