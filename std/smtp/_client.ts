// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/smtp/smtp.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
import {
  decode as fromBase64,
  encode as toBase64,
} from "../encoding/base64.ts";
import { decode } from "../encoding/utf8.ts";
import { TextProtoConn, TextProtoError } from "../textproto/mod.ts";
import type { Response } from "../textproto/mod.ts";
import type { AsyncCloser } from "../textproto/writer.ts";
import { Auth } from "./auth.ts";
import { assert } from "../_util/assert.ts";
import { validateLine } from "./_helpers.ts";

type DataWriter = Deno.Writer & AsyncCloser;

interface StartTLSOptions {
  hostname?: string;
  certFile?: string;
}

export interface SMTPClient {
  /**
   * @description `close` closes the connection.
   */
  close(): void;

  /**
   * @description `hello` sends a HELO or EHLO to the server as the given host name.
   *   Calling this method is only necessary if the client needs control
   *   over the host name used. The client will introduce itself as "localhost"
   *   automatically otherwise. If `hello` is called, it must be called before
   *   any of the other methods.
  */
  hello(localName: string): Promise<void>;

  /**
   * @description `startTLS` sends the STARTTLS command and encrypts all further communication.
   *   Only servers that advertise the STARTTLS extension support this function.
   */
  startTLS(config: StartTLSOptions): Promise<void>;

  /**
   * @description `verify` checks the validity of an email address on the server.
   *   If `verify` returns the resolved promise, the address is valid. A promise rejection
   *   does not necessarily indicate an invalid address. Many servers
   *   will not verify addresses for security reasons.
   */
  verify(addr: string): Promise<void>;

  /**
   * @description `auth` authenticates a client using the provided authentication mechanism.
   *   A failed authentication closes the connection.
   *   Only servers that advertise the AUTH extension support this function.
   */
  auth(auth: Auth): Promise<void>;

  /**
   * @description `mail` issues a MAIL command to the server using the provided email address.
   *   If the server supports the 8BITMIME extension, `mail` adds the BODY=8BITMIME
   *   parameter. If the server supports the SMTPUTF8 extension, `mail` adds the
   *   SMTPUTF8 parameter.
   *   This initiates a mail transaction and is followed by one or more Rcpt calls.
   */
  mail(from: string): Promise<void>;

  /**
   * @description `rcpt` issues a RCPT command to the server using the provided email address.
   *   A call to `rcpt` must be preceded by a call to `mail` and may be followed by
   *   a `data` call or another `rcpt` call.
   */
  rcpt(to: string): Promise<void>;

  /**
   * @description `data` issues a DATA command to the server and returns a writer that
   *   can be used to write the mail headers and body. The caller should
   *   close the writer before calling any more methods on `SMTPClient`. A call to
   *   `data` must be preceded by one or more calls to `rcpt`.
   */
  data(): Promise<DataWriter>;

  /**
   * @description `extension` reports whether an extension is support by the server.
   *   The extension name is case-insensitive. If the extension is supported,
   *   `extension` also returns a string that contains any parameters the
   *   server specifies for the extension.
   */
  extension(ext: string): Promise<string | null>;

  /**
   * @description `reset` sends the RSET command to the server, aborting the current mail
   *   transaction.
   */
  reset(): Promise<void>;

  /**
   * @description `noop` sends the NOOP command to the server. It does nothing but check
   *   that the connection to the server is okay.
   */
  noop(): Promise<void>;

  /**
   * @description `quit` sends the QUIT command and closes the connection to the server.
   */
  quit(): Promise<void>;
}

const EXT_AUTH = "AUTH";

type Ext = {
  [ext: string]: string;
};

/**
 * NOTE: This class is exported for testing purposes only.
 * @private
 */
export class SMTPClientImpl implements SMTPClient {
  #conn: Deno.Conn;
  #tpConn: TextProtoConn;
  #ext: Ext | null = null;
  #helloError: Error | null = null;
  #auth: string[] | null = null;
  #isClosed = false;

  /**
   * NOTE: This property is public for testing purposes only.
   * @private
   */
  _didHello = false;

  /**
   * NOTE: This property is public for testing purposes only.
   * @private
   */
  _isTLS: boolean;

  /**
   * NOTE: This property is public for testing purposes only.
   * @private
   */
  _serverName: string;

  /**
   * NOTE: This property is public for testing purposes only.
   * @private
   */
  _localName: string;

  static async create(
    conn: Deno.Conn,
    serverName: string,
    localName: string,
    isTLS = false,
  ): Promise<SMTPClientImpl> {
    const tpConn = new TextProtoConn(conn);
    try {
      await tpConn.readResponse(220);
      return new SMTPClientImpl(conn, tpConn, serverName, localName);
    } catch (err) {
      tpConn.close();
      throw err;
    }
  }

  constructor(
    conn: Deno.Conn,
    tpConn: TextProtoConn,
    serverName: string,
    localName: string,
    isTLS = false,
  ) {
    this.#conn = conn;
    this.#tpConn = tpConn;
    this._serverName = serverName;
    this._localName = localName;
    this._isTLS = isTLS;
  }

  close(): void {
    if (!this.#isClosed) {
      this.#isClosed = true;
      try {
        this.#tpConn.close();
      } catch (err) {
        if (err instanceof Deno.errors.BadResource) { // Alread closed
          return;
        }
        throw err;
      }
    }
  }

  hello(localName: string): Promise<void> {
    validateLine(localName);
    if (this._didHello) {
      throw new Error("smtp: Hello called after other methods");
    }
    this._localName = localName;
    return this._hello();
  }

  async startTLS(config: StartTLSOptions): Promise<void> {
    await this._hello();
    await this.cmd(220, "STARTTLS");
    const {
      hostname = this._serverName,
      certFile,
    } = config;
    this.#conn = await Deno.startTls(this.#conn, { hostname, certFile });
    this.#tpConn = new TextProtoConn(this.#conn);
    this._isTLS = true;
    return this._ehlo();
  }

  async verify(addr: string): Promise<void> {
    validateLine(addr);
    await this._hello();
    await this.cmd(
      250,
      `VRFY ${addr}`,
    );
  }

  async auth(auth: Auth): Promise<void> {
    await this._hello();
    let mech: string;
    let resp: string | null;
    try {
      const proto = auth.start({
        name: this._serverName,
        tls: this._isTLS,
        auth: this.#auth,
      });
      mech = proto.protocol;
      resp = proto.toServer;
    } catch (err) {
      await this.quit();
      throw err;
    }
    const resp64 = resp ? toBase64(resp) : "";
    let [code, msg64] = await this.cmd(0, `AUTH ${mech} ${resp64}`.trimEnd());
    while (true) {
      let msg: string | null = null;
      let err: Error | null = null;
      switch (code) {
        case 334:
          msg = decode(fromBase64(msg64));
          break;
        case 235:
          msg = msg64;
          break;
        default:
          err = new TextProtoError(code, msg64);
          break;
      }
      if (err === null) {
        try {
          assert(msg);
          resp = auth.next(msg, code === 334);
        } catch (e) {
          err = e;
        }
      }
      if (err !== null) {
        try {
          // deno-lint-ignore no-unreachable
          await this.cmd(501, "*");
        } finally {
          try {
            // deno-lint-ignore no-unreachable
            await this.quit();
            // deno-lint-ignore no-unsafe-finally
          } finally {
            throw err;
          }
        }
      }
      if (resp === null) {
        break;
      }
      const resp64 = toBase64(resp);
      [code, msg64] = await this.cmd(0, resp64);
    }
  }

  async mail(from: string): Promise<void> {
    validateLine(from);
    await this._hello();
    let cmdStr = `MAIL FROM:<${from}>`;
    if (this.#ext !== null) {
      if (this.hasExtension("8BITMIME")) {
        cmdStr += " BODY=8BITMIME";
      }
      if (this.hasExtension("SMTPUTF8")) {
        cmdStr += " SMTPUTF8";
      }
    }
    await this.cmd(250, cmdStr);
  }

  async rcpt(to: string): Promise<void> {
    validateLine(to);
    await this.cmd(25, `RCPT TO:<${to}>`);
  }

  async extension(ext: string): Promise<string | null> {
    await this._hello();
    if (this.#ext === null) {
      return null;
    }
    return this.#ext[ext.toUpperCase()] ?? null;
  }

  async data(): Promise<DataWriter> {
    await this.cmd(354, "DATA");
    const dotWriter = this.#tpConn.dotWriter();
    return {
      write: (p) => dotWriter.write(p),
      close: async () => {
        await dotWriter.close();
        await this.#tpConn.readResponse(
          250,
        );
      },
    };
  }

  async reset(): Promise<void> {
    await this._hello();
    await this.cmd(250, "RSET");
  }

  async noop(): Promise<void> {
    await this._hello();
    await this.cmd(250, "NOOP");
  }

  async quit(): Promise<void> {
    await this._hello();
    await this.cmd(221, "QUIT");
    return this.close();
  }

  /**
   * NOTE: This method is public for testing purposes only.
   * @private
   */
  async _hello(): Promise<void> {
    if (!this._didHello) {
      this._didHello = true;
      try {
        await this._ehlo();
      } catch {
        try {
          await this._helo();
        } catch (helloError) {
          this.#helloError = helloError;
        }
      }
    }
    if (this.#helloError) {
      throw this.#helloError;
    }
  }

  /**
   * @description `helo` sends the HELO greeting to the server. It should be used only when the
   *   server does not support ehlo.
   *   NOTE: This method is public for testing purposes only.
   * @private
   */
  async _helo(): Promise<void> {
    this.#ext = null;
    await this.cmd(
      250,
      `HELO ${this._localName}`,
    );
  }

  /**
   * @description `ehlo` sends the EHLO (extended hello) greeting to the server. It
   *   should be the preferred greeting for servers that support it.
   *   NOTE: This method is public for testing purposes only.
   * @private 
   */
  async _ehlo(): Promise<void> {
    const [_, msg] = await this.cmd(
      250,
      `EHLO ${this._localName}`,
    );
    const ext: Ext = {};
    let extList = msg.split("\n");
    if (extList.length > 1) {
      extList = extList.slice(1);
      for (const line of extList) {
        const i = line.indexOf(" ");
        const extName = i > -1 ? line.slice(0, i) : line;
        const extValue = i > -1 ? line.slice(i + 1) : "";
        ext[extName] = extValue;
      }
    }
    const mechs = ext[EXT_AUTH];
    if (mechs != null) {
      this.#auth = mechs.split(" ");
    }
    this.#ext = ext;
  }

  /**
   * @description `cmd` is a convenience function that sends a command and returns the response
   */
  private async cmd(
    expectCode: number,
    cmd: string,
  ): Promise<Response> {
    await this.#tpConn.cmd(cmd);
    return this.#tpConn.readResponse(expectCode);
  }

  private hasExtension(ext: string): boolean {
    if (this.#ext == null) {
      return false;
    }
    return this.#ext[ext] != null;
  }
}
