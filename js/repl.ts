// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";
import { window } from "./globals";
import * as deno from "./deno";

// FIXME assignis like this is bad
window.deno = deno;
/** Read the next line for the repl.
 *
 *       import { readFile } from "deno";
 *       const decoder = new TextDecoder("utf-8");
 *       const data = await readFile("hello.txt");
 *       console.log(decoder.decode(data));
 */
async function readline(prompt: string): Promise<string> {
  return res(await dispatch.sendAsync(...req(prompt)));
}

function req(
  prompt: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const prompt_ = builder.createString(prompt);
  msg.Repl.startRepl(builder);
  msg.Repl.addPrompt(builder, prompt_);
  const inner = msg.Repl.endRepl(builder);
  return [builder, msg.Any.Repl, inner];
}

function res(baseRes: null | msg.Base): string {
  assert(baseRes != null);
  assert(msg.Any.ReplRes === baseRes!.innerType());
  const inner = new msg.ReplRes();
  assert(baseRes!.inner(inner) != null);
  const line = inner.line();
  assert(line !== null);
  return line ? line : "FIXME"; // FIXME null handling
}

export async function repl_loop() {
  while (true) {
    const line = await readline(">> ");
    try {
      const result = eval.call(window, line);
      if (result) {
        console.log(result);
      }
    } catch (err) {
      console.log(err);
    }
  }
}
