// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/smtp/auth.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
import { HmacSha256 } from "../hash/sha256.ts";

export interface AuthMessage {
  protocol: string;
  toServer: string | null;
}

/** `Auth` is implemented by an SMTP authentication mechanism.
 */
export interface Auth {
  // `start` begins an authentication with a server.
  // It returns the name of the authentication protocol
  // and optionally data to include in the initial AUTH message
  // sent to the server. It can return proto == "" to indicate
  // that the authentication should be skipped.
  // If it throw an error, the SMTP client aborts
  // the authentication attempt and closes the connection.
  start(server: ServerInfo): AuthMessage;

  // `next` continues the authentication. The server has just sent
  // the `fromServer` data. If `more` is true, the server expects a
  // response, which `next` should return as toServer; otherwise
  // `next` should return toServer == nil.
  // If `next` returns a non-nil error, the SMTP client aborts
  // the authentication attempt and closes the connection.
  next(fromServer: string, more: boolean): string | null;
}

/** `ServerInfo` records information about an SMTP server.
 */
export interface ServerInfo {
  /** SMTP server name
   */
  name: string;
  /** using TLS, with valid certificate for Name
   */
  tls?: boolean | null;
  /** advertised authentication mechanisms
   */
  auth: string[] | null;
}

class PlainAuth implements Auth {
  readonly #identity: string;
  readonly #username: string;
  readonly #password: string;
  readonly #host: string;
  constructor(
    identity: string,
    username: string,
    password: string,
    host: string,
  ) {
    this.#identity = identity;
    this.#username = username;
    this.#password = password;
    this.#host = host;
  }

  start(server: ServerInfo): AuthMessage {
    // Must have TLS, or else localhost server.
    // Note: If TLS is not true, then we can't trust ANYTHING in ServerInfo.
    // In particular, it doesn't matter if the server advertises PLAIN auth.
    // That might just be the attacker saying
    // "it's ok, you can trust me with your password."
    if (!server.tls && !isLocalhost(server.name)) {
      throw new Error("unencrypted connection");
    }
    if (server.name !== this.#host) {
      throw new Error("wrong host name");
    }
    return {
      protocol: "PLAIN",
      toServer:
        (this.#identity + "\x00" + this.#username + "\x00" + this.#password),
    };
  }

  next(fromServer: string, more: boolean): string | null {
    if (more) {
      // We've already sent everything.
      throw new Error("unexpected server challenge");
    }
    return null;
  }
}

class CRAMMD5Auth implements Auth {
  readonly #username: string;
  readonly #secret: string;

  constructor(username: string, secret: string) {
    this.#username = username;
    this.#secret = secret;
  }

  start(server: ServerInfo): AuthMessage {
    return { protocol: "CRAM-MD5", toServer: null };
  }

  next(fromServer: string, more: boolean): string | null {
    if (more) {
      const algorithm = new HmacSha256(this.#secret);
      algorithm.update(fromServer);
      return `${this.#username} ${algorithm.hex()}`;
    }
    return null;
  }
}

function isLocalhost(name: string): boolean {
  return name == "localhost" || name == "127.0.0.1" || name == "::1";
}

export interface PlainAuthOptions {
  identity: string;
  username: string;
  password: string;
  host: string;
}

/** `plainAuth` returns an `Auth` that implements the PLAIN authentication
 * mechanism as defined in RFC 4616. The returned `Auth` uses the given
 * `username` and `password` to authenticate to host and act as identity.
 * Usually `identity` should be the empty string, to act as username.
 *
 * `PlainAuth` will only send the credentials if the connection is using TLS
 * or is connected to localhost. Otherwise authentication will fail with an
 * error, without sending the credentials.
 */
export function plainAuth(options: PlainAuthOptions): Auth {
  return new PlainAuth(
    options.identity,
    options.username,
    options.password,
    options.host,
  );
}

/** `CRAMMD5Auth` returns an `Auth` that implements the CRAM-MD5 authentication
 * mechanism as defined in RFC 2195.
 * The returned `Auth` uses the given `username` and `secret` to authenticate
 * to the server using the challenge-response mechanism.
 */
export function cramMD5Auth(username: string, secret: string): Auth {
  return new CRAMMD5Auth(username, secret);
}
