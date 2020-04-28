import * as netOps from "./ops/net.ts";
import {
  ListenOptions,
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
export function listen(
  options: ListenOptions & { transport: "udp" }
): DatagramConn;
export function listen(
  options: UnixListenOptions & { transport: "unixpacket" }
): DatagramConn;
export function listen(
  options: ListenOptions | UnixListenOptions
): Listener | DatagramConn {
  if (options.transport === "unix" || options.transport === "unixpacket") {
    const res = netOps.listen(options);
    if (options.transport === "unix") {
      return new ListenerImpl(res.rid, res.localAddr);
    } else {
      return new DatagramImpl(res.rid, res.localAddr);
    }
  } else {
    // Contrary to what the typing says it can also be "udp"
    return stableListen(options as ListenOptions & { transport?: "tcp" });
  }
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
