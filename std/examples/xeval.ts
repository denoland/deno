import { parse } from "../flags/mod.ts";
import { readStringDelim } from "../io/bufio.ts";

/* eslint-disable-next-line max-len */
// See https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/AsyncFunction.
const AsyncFunction = Object.getPrototypeOf(async function (): Promise<void> {})
  .constructor;

/* eslint-disable max-len */
const HELP_MSG = `xeval

Run a script for each new-line or otherwise delimited chunk of standard input.

Print all the usernames in /etc/passwd:
  cat /etc/passwd | deno run -A https://deno.land/std/examples/xeval.ts "a = $.split(':'); if (a) console.log(a[0])"

A complicated way to print the current git branch:
  git branch | deno run -A https://deno.land/std/examples/xeval.ts -I 'line' "if (line.startsWith('*')) console.log(line.slice(2))"

Demonstrates breaking the input up by space delimiter instead of by lines:
  cat LICENSE | deno run -A https://deno.land/std/examples/xeval.ts -d " " "if ($ === 'MIT') console.log('MIT licensed')",

USAGE:
  deno run -A https://deno.land/std/examples/xeval.ts [OPTIONS] <code>
OPTIONS:
  -d, --delim <delim>       Set delimiter, defaults to newline
  -I, --replvar <replvar>   Set variable name to be used in eval, defaults to $
ARGS:
  <code>`;
/* eslint-enable max-len */

export type XevalFunc = (v: string) => void;

export interface XevalOptions {
  delimiter?: string;
}

const DEFAULT_DELIMITER = "\n";

export async function xeval(
  reader: Deno.Reader,
  xevalFunc: XevalFunc,
  { delimiter = DEFAULT_DELIMITER }: XevalOptions = {}
): Promise<void> {
  for await (const chunk of readStringDelim(reader, delimiter)) {
    // Ignore empty chunks.
    if (chunk.length > 0) {
      await xevalFunc(chunk);
    }
  }
}

async function main(): Promise<void> {
  const parsedArgs = parse(Deno.args, {
    boolean: ["help"],
    string: ["delim", "replvar"],
    alias: {
      delim: ["d"],
      replvar: ["I"],
      help: ["h"],
    },
    default: {
      delim: DEFAULT_DELIMITER,
      replvar: "$",
    },
  });
  if (parsedArgs._.length != 1) {
    console.error(HELP_MSG);
    console.log(parsedArgs._);
    Deno.exit(1);
  }
  if (parsedArgs.help) {
    return console.log(HELP_MSG);
  }

  const delimiter = parsedArgs.delim;
  const replVar = parsedArgs.replvar;
  const code = parsedArgs._[0];

  // new AsyncFunction()'s error message for this particular case isn't great.
  if (!replVar.match(/^[_$A-z][_$A-z0-9]*$/)) {
    console.error(`Bad replvar identifier: "${replVar}"`);
    Deno.exit(1);
  }

  const xEvalFunc = new AsyncFunction(replVar, code);

  await xeval(Deno.stdin, xEvalFunc, { delimiter });
}

if (import.meta.main) {
  main();
}
