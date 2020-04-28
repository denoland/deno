import * as netOps from "./ops/net.ts";
import {
  Listener,
  DatagramConn,
  ListenerImpl,
  DatagramImpl,
  ConnectOptions,
  Conn,
  ConnImpl,
  listen as stableListen,
  connect as stableConnect,
} from "./net.ts";

export interface ListenOptions {
  port: number;
  hostname?: string;
  transport?: "tcp" | "udp";
}

export interface UnixListenOptions {
  transport: "unix" | "unixpacket";
  path: string;
}

export function listen(
  options: ListenOptions & { transport?: "tcp" }
): Listener;
export function listen(
  options: UnixListenOptions & { transport: "unix" }
): Listener;
export function listen(options: ListenOptions | UnixListenOptions): Listener {
  if (options.transport === "unix") {
    const res = netOps.listen(options);
    return new ListenerImpl(res.rid, res.localAddr);
  } else {
    return stableListen(options as ListenOptions & { transport?: "tcp" });
  }
}

export function listenDatagram(
  options: ListenOptions & { transport: "udp" }
): DatagramConn;
export function listenDatagram(
  options: UnixListenOptions & { transport: "unixpacket" }
): DatagramConn;
export function listenDatagram(
  options: ListenOptions | UnixListenOptions
): DatagramConn {
  let res;
  if (options.transport === "unixpacket") {
    res = netOps.listen(options);
  } else {
    res = netOps.listen({
      transport: "udp",
      hostname: "127.0.0.1",
      ...(options as ListenOptions),
    });
  }

  return new DatagramImpl(res.rid, res.localAddr);
}

export interface UnixConnectOptions {
  transport: "unix";
  path: string;
}

export async function connect(options: UnixConnectOptions): Promise<Conn>;
export async function connect(options: ConnectOptions): Promise<Conn>;
export async function connect(
  options: ConnectOptions | UnixConnectOptions
): Promise<Conn> {
  if (options.transport === "unix") {
    const res = await netOps.connect(options);
    return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
  } else {
    return stableConnect(options as ConnectOptions);
  }
}
