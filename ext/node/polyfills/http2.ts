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

export class ServerHttp2Session {
  constructor() {
    notImplemented("ServerHttp2Session");
  }
}
export class ClientHttp2Session {
  constructor() {
    notImplemented("ClientHttp2Session");
  }
}
export class Http2Stream {
  constructor() {
    notImplemented("Http2Stream");
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
