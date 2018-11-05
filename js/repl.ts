// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as deno from "./deno";
import { close } from "./files";
import * as dispatch from "./dispatch";
import { exit } from "./os";
import { window } from "./globals";

function startRepl(historyFile: string): number {
  const builder = flatbuffers.createBuilder();
  const historyFile_ = builder.createString(historyFile);

  msg.ReplStart.startReplStart(builder);
  msg.ReplStart.addHistoryFile(builder, historyFile_);
  const inner = msg.ReplStart.endReplStart(builder);

  const baseRes = dispatch.sendSync(builder, msg.Any.ReplStart, inner);
  assert(baseRes != null);
  assert(msg.Any.ReplStartRes === baseRes!.innerType());
  const innerRes = new msg.ReplStartRes();
  assert(baseRes!.inner(innerRes) != null);
  const rid = innerRes.rid();
  return rid;
}

// @internal
export function readline(rid: number, prompt: string): string {
  const builder = flatbuffers.createBuilder();
  const prompt_ = builder.createString(prompt);
  msg.ReplReadline.startReplReadline(builder);
  msg.ReplReadline.addRid(builder, rid);
  msg.ReplReadline.addPrompt(builder, prompt_);
  const inner = msg.ReplReadline.endReplReadline(builder);

  // TODO use async?
  const baseRes = dispatch.sendSync(builder, msg.Any.ReplReadline, inner);

  assert(baseRes != null);
  assert(msg.Any.ReplReadlineRes === baseRes!.innerType());
  const innerRes = new msg.ReplReadlineRes();
  assert(baseRes!.inner(innerRes) != null);
  const line = innerRes.line();
  assert(line !== null);
  return line || "";
}

// @internal
export function replLoop(): void {
  window.deno = deno; // FIXME use a new scope (rather than window).

  const historyFile = "deno_history.txt";
  const prompt = "> ";

  const rid = startRepl(historyFile);

  let line = "";
  while (true) {
    try {
      line = readline(rid, prompt);
      line = line.trim();
    } catch (err) {
      if (err.message === "EOF") {
        break;
      }
      console.error(err);
      exit(1);
    }
    if (!line) {
      continue;
    }
    if (line === ".exit") {
      break;
    }
    try {
      const result = eval.call(window, line); // FIXME use a new scope.
      console.log(result);
    } catch (err) {
      if (err instanceof Error) {
        console.error(`${err.constructor.name}: ${err.message}`);
      } else {
        console.error("Thrown:", err);
      }
    }
  }

  close(rid);
}
