import { Buffer } from "./buffer";
import { stdin } from "./files";
import { TextEncoder, TextDecoder } from "./text_encoding";
import { exit } from "./os";

export type XevalFunc = (v: string) => void;

async function writeAll(buffer: Buffer, arr: Uint8Array): Promise<void> {
  let bytesWritten = 0;
  while (bytesWritten < arr.length) {
    try {
      const nwritten = await buffer.write(arr.subarray(bytesWritten));
      bytesWritten += nwritten;
    } catch {
      return;
    }
  }
}

export async function xevalMain(
  xevalFunc: XevalFunc,
  delim_: string
): Promise<void> {
  const inputBuffer = new Buffer();
  const inspectArr = new Uint8Array(1024);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const delimArr = encoder.encode(delim_); // due to unicode

  let nextMatchIndex = 0;
  while (true) {
    const rr = await stdin.read(inspectArr);
    if (rr.nread < 0) {
      exit(1);
    }
    const sliceRead = inspectArr.subarray(0, rr.nread);
    // Avoid unicode problems
    let nextSliceStartIndex = 0;
    for (let i = 0; i < sliceRead.length; i++) {
      if (sliceRead[i] == delimArr[nextMatchIndex]) {
        nextMatchIndex++;
      } else {
        nextMatchIndex = 0;
      }
      if (nextMatchIndex === delimArr.length) {
        nextMatchIndex = 0; // reset match
        const sliceToJoin = sliceRead.subarray(nextSliceStartIndex, i + 1);
        nextSliceStartIndex = i + 1;
        await writeAll(inputBuffer, sliceToJoin);

        let readyBytes = inputBuffer.bytes();
        inputBuffer.reset();
        readyBytes = readyBytes.subarray(
          0,
          readyBytes.length - delimArr.length
        );
        let readyChunk = decoder.decode(readyBytes);
        if (readyChunk.length > 0) {
          // ignore blank chunk
          xevalFunc(readyChunk);
        }
      }
    }
    await writeAll(inputBuffer, sliceRead.subarray(nextSliceStartIndex));
    if (rr.eof) {
      const lastString = inputBuffer.toString();
      if (lastString.length > 0) {
        xevalFunc(lastString);
      }
      break;
    }
  }
}
