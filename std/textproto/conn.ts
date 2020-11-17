// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/textproto/textproto.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { BufReader, BufWriter } from "../io/bufio.ts";
import { TextProtoReader } from "./reader.ts";
import type { Response } from "./reader.ts";
import { TextProtoWriter } from "./writer.ts";
import type { AsyncCloser } from "./writer.ts";

export class TextProtoConn {
  readonly #conn: Deno.Conn;
  readonly #r: TextProtoReader;
  readonly #w: TextProtoWriter;

  constructor(conn: Deno.Conn) {
    this.#conn = conn;
    this.#r = new TextProtoReader(BufReader.create(conn));
    this.#w = new TextProtoWriter(BufWriter.create(conn));
  }

  close(): void {
    return this.#conn.close();
  }

  dotWriter(): Deno.Writer & AsyncCloser {
    return this.#w.dotWriter();
  }

  /**
   * @description `cmd` is a convenience method that sends a command after
   *   waiting its turn in the pipeline. The command text is the
   *   result of formatting format with args and appending \r\n.
   *   Cmd returns the id of the command, for use with StartResponse and EndResponse.
   * 
   *   For example, a client might run a HELP command that returns a dot-body
   *   by using:
   * 
   * 	 id, err := c.Cmd("HELP")
   * 	 if err != nil {
   * 	 	return nil, err
   * 	 }
   * 
   * 	 c.StartResponse(id)
   * 	 defer c.EndResponse(id)
   * 
   * 	 if _, _, err = c.ReadCodeLine(110); err != nil {
   * 	 	return nil, err
   * 	 }
   * 	 text, err := c.ReadDotBytes()
   * 	 if err != nil {
   * 	 	return nil, err
   * 	 }
   * 	 return c.ReadCodeLine(250)
   *
   */
  cmd(cmd: string): Promise<void> {
    return this.#w.printLine(cmd);
  }

  readLine(): Promise<string | null> {
    return this.#r.readLine();
  }

  readResponse(expectCode: number): Promise<Response> {
    return this.#r.readResponse(expectCode);
  }

  printLine(cmd: string): Promise<void> {
    return this.#w.printLine(cmd);
  }
}
