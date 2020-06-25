// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface NetAddr {
  transport: "tcp" | "udp";
  hostname: string;
  port: number;
}

export interface UnixAddr {
  transport: "unix" | "unixpacket";
  path: string;
}

export type Addr = NetAddr | UnixAddr;

export enum ShutdownMode {
  // See http://man7.org/linux/man-pages/man2/shutdown.2.html
  // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
  Read = 0,
  Write,
  ReadWrite, // unused
}

export function shutdown(rid: number, how: ShutdownMode): Promise<void> {
  sendSync("op_shutdown", { rid, how });
  return Promise.resolve();
}

interface AcceptResponse {
  rid: number;
  localAddr: Addr;
  remoteAddr: Addr;
}

export function accept(
  rid: number,
  transport: string
): Promise<AcceptResponse> {
  return sendAsync("op_accept", { rid, transport });
}

export type ListenRequest = Addr;

interface ListenResponse {
  rid: number;
  localAddr: Addr;
}

export function listen(args: ListenRequest): ListenResponse {
  return sendSync("op_listen", args);
}

interface ConnectResponse {
  rid: number;
  localAddr: Addr;
  remoteAddr: Addr;
}

export type ConnectRequest = Addr;

export function connect(args: ConnectRequest): Promise<ConnectResponse> {
  return sendAsync("op_connect", args);
}

interface ReceiveResponse {
  size: number;
  remoteAddr: Addr;
}

export function receive(
  rid: number,
  transport: string,
  zeroCopy: Uint8Array
): Promise<ReceiveResponse> {
  return sendAsync("op_datagram_receive", { rid, transport }, zeroCopy);
}

export type SendRequest = {
  rid: number;
} & Addr;

export async function send(
  args: SendRequest,
  zeroCopy: Uint8Array
): Promise<number> {
  const byteLength = await sendAsync("op_datagram_send", args, zeroCopy);
  return byteLength;
}
