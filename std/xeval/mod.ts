import { parse } from "../flags/mod.ts";
import { chunks } from "../io/bufio.ts";
const { args, exit, stdin } = Deno;
type Reader = Deno.Reader;

/* eslint-disable-next-line max-len */
// See https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/AsyncFunction.
const AsyncFunction = Object.getPrototypeOf(async function(): Promise<void> {})
  .constructor;

const HELP_MSG = `Deno xeval

USAGE:
  deno -A https://deno.land/std/xeval/mod.ts [OPTIONS] <code>

OPTIONS:
  -d, --delim <delim>       Set delimiter, defaults to newline
  -I, --replvar <replvar>   Set variable name to be used in eval, defaults to $

ARGS:
  <code>`;

export type XevalFunc = (v: string) => void;

export interface XevalOptions {
  delimiter?: string;
}

const DEFAULT_DELIMITER = "\n";

export async function xeval(
  reader: Reader,
  xevalFunc: XevalFunc,
  { delimiter = DEFAULT_DELIMITER }: XevalOptions = {}
): Promise<void> {
  for await (const chunk of chunks(reader, delimiter)) {
    // Ignore empty chunks.
    if (chunk.length > 0) {
      await xevalFunc(chunk);
    }
  }
}

async function main(): Promise<void> {
  const parsedArgs = parse(args.slice(1), {
    boolean: ["help"],
    string: ["delim", "replvar"],
    alias: {
      delim: ["d"],
      replvar: ["I"],
      help: ["h"]
    },
    default: {
      delim: DEFAULT_DELIMITER,
      replvar: "$"
    }
  });
  if (parsedArgs._.length != 1) {
    console.error(HELP_MSG);
    exit(1);
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
    exit(1);
  }

  const xEvalFunc = new AsyncFunction(replVar, code);

  await xeval(stdin, xEvalFunc, { delimiter });
}

if (import.meta.main) {
  main();
}
