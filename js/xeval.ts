import { Buffer, writeAll } from "./buffer.ts";
import { stdin } from "./files.ts";
import { TextEncoder, TextDecoder } from "./text_encoding.ts";
import { Reader, EOF } from "./io.ts";

export type XevalFunc = (v: string) => void;

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

// TODO(kevinkassimo): Move this utility to deno_std.
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
    let result = await reader.read(inspectArr);
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
    let sliceRead = inspectArr.subarray(0, result as number);
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

export async function xevalMain(
  xevalFunc: XevalFunc,
  delim_: string | null
): Promise<void> {
  if (!delim_) {
    delim_ = "\n";
  }
  for await (const chunk of chunks(stdin, delim_)) {
    // Ignore empty chunks.
    if (chunk.length > 0) {
      xevalFunc(chunk);
    }
  }
}
