// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as deno from "./deno";
import { close } from "./files";
import * as dispatch from "./dispatch";
import { exit } from "./os";
import { globalEval } from "./global_eval";
import { libdeno } from "./libdeno";

const window = globalEval("this");

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
export async function readline(rid: number, prompt: string): Promise<string> {
  const builder = flatbuffers.createBuilder();
  const prompt_ = builder.createString(prompt);
  msg.ReplReadline.startReplReadline(builder);
  msg.ReplReadline.addRid(builder, rid);
  msg.ReplReadline.addPrompt(builder, prompt_);
  const inner = msg.ReplReadline.endReplReadline(builder);

  const baseRes = await dispatch.sendAsync(
    builder,
    msg.Any.ReplReadline,
    inner
  );

  assert(baseRes != null);
  assert(msg.Any.ReplReadlineRes === baseRes!.innerType());
  const innerRes = new msg.ReplReadlineRes();
  assert(baseRes!.inner(innerRes) != null);
  const line = innerRes.line();
  assert(line !== null);
  return line || "";
}

// @internal
export async function replLoop(): Promise<void> {
  window.deno = deno;

  const historyFile = "deno_history.txt";
  const rid = startRepl(historyFile);

  while (true) {
    let code = "";
    try {
      code = await readline(rid, "> ");
      if (!code) {
        continue;
      } else if (code.trim() === ".exit") {
        break;
      }
      while (!evaluate(code)) {
        code += "\n";
        code += await readline(rid, "  ");
      }
    } catch (err) {
      if (err.message === "EOF") {
        break;
      }
      console.error(err);
      exit(1);
    }
  }

  close(rid);
}

// @internal
function evaluate(code: string): boolean {
  // returns true if code is consumed
  try {
    // TODO: use sandbox in the future
    const [result, errInfo] = libdeno.eval(code);
    if (!errInfo) {
      console.log(result);
    } else {
      if (
        errInfo.isCompileError &&
        errInfo.thrown.message === "SyntaxError: Unexpected end of input"
      ) {
        return false; // don't consume code.
      } else {
        // non-compiler error
        if (errInfo.isNativeError) {
          console.error((errInfo.thrown as Error).message);
        } else {
          console.error("Thrown:", errInfo.thrown);
        }
      }
    }
  } catch (err) {
    if (err instanceof Error) {
      console.error(`${err.constructor.name}: ${err.message}`);
    } else {
      console.error("Thrown:", err);
    }
  }
  return true; // code consumed
}
