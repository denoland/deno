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

export async function xevalMain(xevalFunc: XevalFunc): Promise<void> {
  const inputBuffer = new Buffer();
  const inputArray = new Uint8Array(1024);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  while (true) {
    const rr = await stdin.read(inputArray);
    if (rr.nread < 0) {
      exit(1);
    }
    const sliceRead = inputArray.subarray(0, rr.nread);
    let stringRead = decoder.decode(sliceRead);
    stringRead = stringRead.replace(/\s+/g, " "); // don't trim

    while (stringRead.length > 0) {
      const index = stringRead.indexOf(" ");
      if (index < 0) {
        await writeAll(inputBuffer, encoder.encode(stringRead));
        break; // start next stdin read
      }

      const value = inputBuffer.toString() + stringRead.slice(0, index);
      stringRead = stringRead.slice(index + 1);
      inputBuffer.reset();
      if (value.length > 0) {
        // we might read a single whitespace
        xevalFunc(value);
      }
    }

    if (rr.eof) {
      const lastString = inputBuffer.toString();
      if (lastString.length > 0) {
        xevalFunc(lastString);
      }
      break;
    }
  }
}
