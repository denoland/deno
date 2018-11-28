// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as deno from "./deno";
import { close } from "./files";
import * as dispatch from "./dispatch";
import { exit } from "./os";
import { globalEval } from "./global_eval";

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
  window.deno = deno; // FIXME use a new scope (rather than window).

  const historyFile = "deno_history.txt";
  const rid = startRepl(historyFile);

  let code = "";
  while (true) {
    try {
      code = await readBlock(rid, "> ", "  ");
    } catch (err) {
      if (err.message === "EOF") {
        break;
      }
      console.error(err);
      exit(1);
    }
    if (!code) {
      continue;
    } else if (code.trim() === ".exit") {
      break;
    }

    evaluate(code);
  }

  close(rid);
}

function evaluate(code: string): void {
  try {
    const result = eval.call(window, code); // FIXME use a new scope.
    console.log(result);
  } catch (err) {
    if (err instanceof Error) {
      console.error(`${err.constructor.name}: ${err.message}`);
    } else {
      console.error("Thrown:", err);
    }
  }
}

async function readBlock(
  rid: number,
  prompt: string,
  continuedPrompt: string
): Promise<string> {
  let code = "";
  do {
    code += await readline(rid, prompt);
    prompt = continuedPrompt;
  } while (parenthesesAreOpen(code));
  return code;
}

// modified from
// https://codereview.stackexchange.com/a/46039/148556
function parenthesesAreOpen(code: string): boolean {
  const parentheses = "[]{}()";
  const stack = [];

  for (const ch of code) {
    const bracePosition = parentheses.indexOf(ch);

    if (bracePosition === -1) {
      // not a paren
      continue;
    }

    if (bracePosition % 2 === 0) {
      stack.push(bracePosition + 1); // push next expected brace position
    } else {
      if (stack.length === 0 || stack.pop() !== bracePosition) {
        return false;
      }
    }
  }
  return stack.length > 0;
}
