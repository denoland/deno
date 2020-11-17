// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/smtp/smtp.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import type { SMTPClient } from "./_client.ts";
import { SMTPClientImpl } from "./_client.ts";
import type {
  Auth,
  AuthMessage,
  PlainAuthOptions,
  ServerInfo,
} from "./auth.ts";
import { splitHostPort, validateLine } from "./_helpers.ts";
import { TextProtoConn } from "../textproto/conn.ts";
import { encode } from "../encoding/utf8.ts";

export { cramMD5Auth, plainAuth } from "./auth.ts";
export type { Auth, AuthMessage, PlainAuthOptions, ServerInfo };

export type { SMTPClient };

const DEFAULT_HOST = "localhost";

/** `connect` returns a new SMTPClient connected to an SMTP server at `options.host` and `options.port`.
 */
export async function connect(
  options: Deno.ConnectOptions,
): Promise<SMTPClient> {
  const conn = await Deno.connect(options);
  return createSMTPClient(conn, options.hostname ?? DEFAULT_HOST);
}

/** `createSMTPClient` returns a new `SMTPClient` using an existing connection and host as a server name to be used when authenticating.
 */
export function createSMTPClient(
  conn: Deno.Conn,
  host: string,
): Promise<SMTPClient> {
  return SMTPClientImpl.create(conn, host, DEFAULT_HOST, false);
}

/** `sendMail` connects to the server at addr, switches to TLS if
 * possible, authenticates with the optional mechanism a if possible,
 * and then sends an email from address from, to addresses to, with
 * message msg.
 * The addr must include a port, as in "mail.example.com:smtp".
 * 
 * The addresses in the to parameter are the SMTP RCPT addresses.
 * 
 * The msg parameter should be an RFC 822-style email with headers
 * first, a blank line, and then the message body. The lines of msg
 * should be CRLF terminated. The msg headers should usually include
 * fields such as "From", "To", "Subject", and "Cc".  Sending "Bcc"
 * messages is accomplished by including an email address in the to
 * parameter but not including it in the msg headers.
 * 
 * The `sendMail` function and the net/smtp package are low-level
 * mechanisms and provide no support for DKIM signing, MIME
 * attachments (see the mime/multipart package), or other mail
 * functionality. Higher-level packages exist outside of the standard
 * library.
 */
export async function sendMail(options: SendMailOptions): Promise<void> {
  const { addr, auth, from, to, msg } = options;
  validateLine(from);
  for (const recp of to) {
    validateLine(recp);
  }
  const [hostname, port] = typeof addr === "string"
    ? splitHostPort(addr)
    : [addr.hostname, addr.port];
  const c = await connect({ hostname, port });
  try {
    if (await c.extension("STARTTLS") !== null) {
      await c.startTLS({});
    }

    if (auth != null) {
      if (await c.extension("AUTH") === null) {
        throw new Error("smtp: server doesn't support AUTH");
      }
      await c.auth(auth);
    }
    await c.mail(from);
    for (const recp of to) {
      await c.rcpt(recp);
    }
    const writer = await c.data();
    await writer.write(encode(msg));
    await writer.close();
    await c.quit();
  } finally {
    c.close();
  }
}

export interface SendMailOptions {
  addr: string | Deno.NetAddr;
  auth?: Auth | null;
  from: string;
  to: string[];
  msg: string;
}
