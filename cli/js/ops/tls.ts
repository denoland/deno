// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendAsync, sendSync } from "./dispatch_json.ts";

export interface ConnectTLSRequest {
  transport: "tcp";
  hostname: string;
  port: number;
  certFile?: string;
}

interface ConnectTLSResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: "tcp";
  };
  remoteAddr: {
    hostname: string;
    port: number;
    transport: "tcp";
  };
}

export function connectTLS(
  args: ConnectTLSRequest
): Promise<ConnectTLSResponse> {
  return sendAsync("op_connect_tls", args);
}

interface AcceptTLSResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: "tcp";
  };
  remoteAddr: {
    hostname: string;
    port: number;
    transport: "tcp";
  };
}

export function acceptTLS(rid: number): Promise<AcceptTLSResponse> {
  return sendAsync("op_accept_tls", { rid });
}

export interface ListenTLSRequest {
  port: number;
  hostname: string;
  transport: "tcp";
  certFile: string;
  keyFile: string;
}

interface ListenTLSResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: "tcp";
  };
}

export function listenTLS(args: ListenTLSRequest): ListenTLSResponse {
  return sendSync("op_listen_tls", args);
}
