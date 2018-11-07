// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as deno from "./deno";
import { close } from "./files";
import * as dispatch from "./dispatch";
import { exit } from "./os";
import { window } from "./globals";
import { DenoCompiler } from "./compiler";

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


interface ReplContext {
  lines: string[];
  previousOutput: string;
}

/**
 * Eval helpers.
 */
// const EVAL_FILENAME = `[eval].ts`
const REPL_CONTEXT: ReplContext = {
  lines: [],
  previousOutput: '',
};

// @internal
export function replLoop(): void {
  window.deno = deno; // FIXME use a new scope (rather than window).

  const historyFile = "deno_history.txt";
  const rid = startRepl(historyFile);

  let code = "";
  while (true) {
    try {
      code = readBlock(rid, "> ", "  ");
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
    // TODO:
    // 1. preserve context between calls
    // 2. run only incremental lines, not full output
    // 3. output diagnostics
    REPL_CONTEXT.lines.push(code);
    const compiledCode = compileReplCode(REPL_CONTEXT);
    console.log('compiledCode', compiledCode);

    const result = eval.call(window, compiledCode.outputCode); // FIXME use a new scope.
    console.log(result);
    REPL_CONTEXT.previousOutput = compiledCode.outputCode;
  } catch (err) {
    if (err instanceof Error) {
      console.error(`${err.constructor.name}: ${err.message}`);
    } else {
      console.error("Thrown:", err);
    }
  }
}

const compiler = DenoCompiler.instance();

function compileReplCode(context: ReplContext) {
  const sourceCode = context.lines.join('\n');
  return compiler.incrementalCompile(sourceCode, context.previousOutput);
}

function readBlock(
  rid: number,
  prompt: string,
  continuedPrompt: string
): string {
  let code = "";
  do {
    code += readline(rid, prompt);
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
