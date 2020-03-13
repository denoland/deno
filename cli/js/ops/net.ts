// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export type Transport = "tcp" | "udp";
// TODO support other types:
// export type Transport = "tcp" | "tcp4" | "tcp6" | "unix" | "unixpacket";

export enum ShutdownMode {
  // See http://man7.org/linux/man-pages/man2/shutdown.2.html
  // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
  Read = 0,
  Write,
  ReadWrite // unused
}

export function shutdown(rid: number, how: ShutdownMode): void {
  sendSync("op_shutdown", { rid, how });
}

interface AcceptResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
  remoteAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
}

export async function accept(rid: number): Promise<AcceptResponse> {
  return await sendAsync("op_accept", { rid });
}

export interface ListenRequest {
  transport: Transport;
  hostname: string;
  port: number;
}

interface ListenResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
}

export function listen(args: ListenRequest): ListenResponse {
  return sendSync("op_listen", args);
}

interface ConnectResponse {
  rid: number;
  localAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
  remoteAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
}

export interface ConnectRequest {
  transport: Transport;
  hostname: string;
  port: number;
}

export async function connect(args: ConnectRequest): Promise<ConnectResponse> {
  return await sendAsync("op_connect", args);
}

interface ReceiveResponse {
  size: number;
  remoteAddr: {
    hostname: string;
    port: number;
    transport: Transport;
  };
}

export async function receive(
  rid: number,
  zeroCopy: Uint8Array
): Promise<ReceiveResponse> {
  return await sendAsync("op_receive", { rid }, zeroCopy);
}

export interface SendRequest {
  rid: number;
  hostname: string;
  port: number;
  transport: Transport;
}

export async function send(
  args: SendRequest,
  zeroCopy: Uint8Array
): Promise<void> {
  await sendAsync("op_send", args, zeroCopy);
}
