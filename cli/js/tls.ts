// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { Conn, ConnImpl } from "./net.ts";

// TODO(ry) There are many configuration options to add...
// https://docs.rs/rustls/0.16.0/rustls/struct.ClientConfig.html
interface DialTLSOptions {
  port: number;
  hostname?: string;
}
const dialTLSDefaults = { hostname: "127.0.0.1", transport: "tcp" };

/**
 * dialTLS establishes a secure connection over TLS (transport layer security).
 */
export async function dialTLS(options: DialTLSOptions): Promise<Conn> {
  options = Object.assign(dialTLSDefaults, options);
  const res = await sendAsync(dispatch.OP_DIAL_TLS, options);
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
