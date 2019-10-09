import { parse } from "../flags/mod.ts";
const { Buffer, EOF, args, exit, stdin, writeAll } = Deno;
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

// Generate longest proper prefix which is also suffix array.
function createLPS(pat: Uint8Array): Uint8Array {
  const lps = new Uint8Array(pat.length);
  lps[0] = 0;
  let prefixEnd = 0;
  let i = 1;
  while (i < lps.length) {
    if (pat[i] == pat[prefixEnd]) {
      prefixEnd++;
      lps[i] = prefixEnd;
      i++;
    } else if (prefixEnd === 0) {
      lps[i] = 0;
      i++;
    } else {
      prefixEnd = pat[prefixEnd - 1];
    }
  }
  return lps;
}

// TODO(kevinkassimo): Move this utility somewhere public in deno_std.
// Import from there once doable.
// Read from reader until EOF and emit string chunks separated
// by the given delimiter.
async function* chunks(
  reader: Reader,
  delim: string
): AsyncIterableIterator<string> {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  // Avoid unicode problems
  const delimArr = encoder.encode(delim);
  const delimLen = delimArr.length;
  const delimLPS = createLPS(delimArr);

  let inputBuffer = new Buffer();
  const inspectArr = new Uint8Array(Math.max(1024, delimLen + 1));

  // Modified KMP
  let inspectIndex = 0;
  let matchIndex = 0;
  while (true) {
    const result = await reader.read(inspectArr);
    if (result === EOF) {
      // Yield last chunk.
      const lastChunk = inputBuffer.toString();
      yield lastChunk;
      return;
    }
    if ((result as number) < 0) {
      // Discard all remaining and silently fail.
      return;
    }
    const sliceRead = inspectArr.subarray(0, result as number);
    await writeAll(inputBuffer, sliceRead);

    let sliceToProcess = inputBuffer.bytes();
    while (inspectIndex < sliceToProcess.length) {
      if (sliceToProcess[inspectIndex] === delimArr[matchIndex]) {
        inspectIndex++;
        matchIndex++;
        if (matchIndex === delimLen) {
          // Full match
          const matchEnd = inspectIndex - delimLen;
          const readyBytes = sliceToProcess.subarray(0, matchEnd);
          // Copy
          const pendingBytes = sliceToProcess.slice(inspectIndex);
          const readyChunk = decoder.decode(readyBytes);
          yield readyChunk;
          // Reset match, different from KMP.
          sliceToProcess = pendingBytes;
          inspectIndex = 0;
          matchIndex = 0;
        }
      } else {
        if (matchIndex === 0) {
          inspectIndex++;
        } else {
          matchIndex = delimLPS[matchIndex - 1];
        }
      }
    }
    // Keep inspectIndex and matchIndex.
    inputBuffer = new Buffer(sliceToProcess);
  }
}

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
