// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/smtp/smtp.go
  * Copyright 2010 The Go Authors. All rights reserved.
  * Use of this source code is governed by a BSD-style
  * license that can be found in the LICENSE file.
  */
export function validateLine(line: string): void {
  if (line.includes("\r") || line.includes("\n")) {
    throw new Error("smtp: A line must not contain CR or LF");
  }
}

// TODO(uki00a) I'm not sure if std/smtp/mod.ts is the right place for this function.
/** Adopted from https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/ipsock.go
 * Copyright 2009 The Go Authors. All rights reserved.
 * Use of this source code is governed by a BSD-style
 * license that can be found in the LICENSE file.
 *
 * SplitHostPort splits a network address of the form "host:port",
 * "host%zone:port", "[host]:port" or "[host%zone]:port" into host or
 * host%zone and port.
 * 
 * A literal IPv6 address in hostport must be enclosed in square
 * brackets, as in "[::1]:80", "[::1%lo0]:80".
 * 
 * See func Dial for a description of the hostport parameter, and host
 * and port results.
 */
export function splitHostPort(hostport: string): [string, number] {
  const missingPort = "missing port in address";
  const tooManyColons = "too many colons in address";
  function addrErr(addr: string, why: string): AddrError {
    return new AddrError(why, addr);
  }
  let j = 0, k = 0;

  // The port starts after the last colon.
  const i = hostport.lastIndexOf(":");
  if (i < 0) {
    throw addrErr(hostport, missingPort);
  }

  let host: string;
  if (hostport[0] === "[") {
    // Expect the first ']' just before the last ':'.
    const end = hostport.indexOf("]");
    if (end < 0) {
      throw addrErr(hostport, "missing ']' in address");
    }
    switch (end + 1) {
      case hostport.length:
        // There can't be a ':' behind the ']' now.
        throw addrErr(hostport, missingPort);
      case i:
        // The expected result.
        break;
      default:
        // Either ']' isn't followed by a colon, or it is
        // followed by a colon that is not the last one.
        if (hostport[end + 1] === ":") {
          throw addrErr(hostport, tooManyColons);
        }
        throw addrErr(hostport, missingPort);
    }
    host = hostport.slice(1, end);
    j = 1;
    k = end + 1; // there can't be a '[' resp. ']' before these positions
  } else {
    host = hostport.slice(0, i);
    if (host.indexOf(":") >= 0) {
      throw addrErr(hostport, tooManyColons);
    }
  }
  if (hostport.indexOf("[", j) >= 0) {
    throw addrErr(hostport, "unexpected '[' in address");
  }
  if (hostport.indexOf("]", k) >= 0) {
    throw addrErr(hostport, "unexpected ']' in address");
  }

  const port = hostport.slice(i + 1);
  return [host, parseInt(port)];
}

class AddrError extends Error {
  constructor(why: string, readonly addr: string) {
    super(why);
  }
}
