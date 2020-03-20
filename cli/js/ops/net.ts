// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface NetAddr {
  transport: "tcp" | "udp";
  hostname: string;
  port: number;
}

export interface UnixAddr {
  transport: "unix" | "unixpacket";
  address: string;
}

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
  localAddr: NetAddr | UnixAddr;
  remoteAddr: NetAddr | UnixAddr;
}

export async function accept(
  rid: number,
  transport: string
): Promise<AcceptResponse> {
  return sendAsync("op_accept", { rid, transport });
}

export type ListenRequest = NetAddr | UnixAddr;

interface ListenResponse {
  rid: number;
  localAddr: NetAddr | UnixAddr;
}

export function listen(args: ListenRequest): ListenResponse {
  return sendSync("op_listen", args);
}

interface ConnectResponse {
  rid: number;
  localAddr: NetAddr | UnixAddr;
  remoteAddr: NetAddr | UnixAddr;
}

export type ConnectRequest = NetAddr | UnixAddr;

export async function connect(args: ConnectRequest): Promise<ConnectResponse> {
  return sendAsync("op_connect", args);
}

interface ReceiveResponse {
  size: number;
  remoteAddr: NetAddr | UnixAddr;
}

export async function receive(
  rid: number,
  transport: string,
  zeroCopy: Uint8Array
): Promise<ReceiveResponse> {
  return sendAsync("op_receive", { rid, transport }, zeroCopy);
}

export interface SendRequest {
  rid: number;
}

export async function send(
  args: SendRequest & (UnixAddr | NetAddr),
  zeroCopy: Uint8Array
): Promise<void> {
  await sendAsync("op_send", args, zeroCopy);
}
