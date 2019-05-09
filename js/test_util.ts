// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//
// We want to test many ops in deno which have different behavior depending on
// the permissions set. These tests can specify which permissions they expect,
// which appends a special string like "permW1N0" to the end of the test name.
// Here we run several copies of deno with different permissions, filtering the
// tests by the special string. permW1N0 means allow-write but not allow-net.
// See tools/unit_tests.py for more details.

import * as testing from "./deps/https/deno.land/std/testing/mod.ts";
import {
  assert,
  assertEquals
} from "./deps/https/deno.land/std/testing/asserts.ts";
export {
  assert,
  assertEquals
} from "./deps/https/deno.land/std/testing/asserts.ts";

interface TestPermissions {
  read?: boolean;
  write?: boolean;
  net?: boolean;
  env?: boolean;
  run?: boolean;
  highPrecision?: boolean;
}

const processPerms = Deno.permissions();

function permissionsMatch(
  processPerms: Deno.Permissions,
  requiredPerms: Deno.Permissions
): boolean {
  for (const permName in processPerms) {
    if (processPerms[permName] !== requiredPerms[permName]) {
      return false;
    }
  }

  return true;
}

export const permissionCombinations: Map<string, Deno.Permissions> = new Map();

function permToString(perms: Deno.Permissions): string {
  const r = perms.read ? 1 : 0;
  const w = perms.write ? 1 : 0;
  const n = perms.net ? 1 : 0;
  const e = perms.env ? 1 : 0;
  const u = perms.run ? 1 : 0;
  const h = perms.highPrecision ? 1 : 0;
  return `permR${r}W${w}N${n}E${e}U${u}H${h}`;
}

function registerPermCombination(perms: Deno.Permissions): void {
  const key = permToString(perms);
  if (!permissionCombinations.has(key)) {
    permissionCombinations.set(key, perms);
  }
}

function normalizeTestPermissions(perms: TestPermissions): Deno.Permissions {
  return {
    read: !!perms.read,
    write: !!perms.write,
    net: !!perms.net,
    run: !!perms.run,
    env: !!perms.env,
    highPrecision: !!perms.highPrecision
  };
}

export function testPerm(
  perms: TestPermissions,
  fn: testing.TestFunction
): void {
  const normalizedPerms = normalizeTestPermissions(perms);

  registerPermCombination(normalizedPerms);

  if (!permissionsMatch(processPerms, normalizedPerms)) {
    return;
  }

  testing.test(fn);
}

export function test(fn: testing.TestFunction): void {
  testPerm(
    {
      read: false,
      write: false,
      net: false,
      env: false,
      run: false,
      highPrecision: false
    },
    fn
  );
}

function extractNumber(re: RegExp, str: string): number | undefined {
  const match = str.match(re);

  if (match) {
    return Number.parseInt(match[1]);
  }
}


// MIN_READ is the minimum ArrayBuffer size passed to a read call by
// buffer.ReadFrom. As long as the Buffer has at least MIN_READ bytes beyond
// what is required to hold the contents of r, readFrom() will not grow the
// underlying buffer.
const MIN_READ = 512;
const MAX_SIZE = 2 ** 32 - 2;

// `off` is the offset into `dst` where it will at which to begin writing values
// from `src`.
// Returns the number of bytes copied.
function copyBytes(dst: Uint8Array, src: Uint8Array, off = 0): number {
  const r = dst.byteLength - off;
  if (src.byteLength > r) {
    src = src.subarray(0, r);
  }
  dst.set(src, off);
  return src.byteLength;
}

/** A Buffer is a variable-sized buffer of bytes with read() and write()
 * methods. Based on https://golang.org/pkg/bytes/#Buffer
 */
export class Buffer {
  private buf: Uint8Array; // contents are the bytes buf[off : len(buf)]
  private off = 0; // read at buf[off], write at buf[buf.byteLength]

  constructor(ab?: ArrayBuffer) {
    if (ab == null) {
      this.buf = new Uint8Array(0);
      return;
    }

    this.buf = new Uint8Array(ab);
  }

  /** bytes() returns a slice holding the unread portion of the buffer.
   * The slice is valid for use only until the next buffer modification (that
   * is, only until the next call to a method like read(), write(), reset(), or
   * truncate()). The slice aliases the buffer content at least until the next
   * buffer modification, so immediate changes to the slice will affect the
   * result of future reads.
   */
  bytes(): Uint8Array {
    return this.buf.subarray(this.off);
  }

  /** toString() returns the contents of the unread portion of the buffer
   * as a string. Warning - if multibyte characters are present when data is
   * flowing through the buffer, this method may result in incorrect strings
   * due to a character being split.
   */
  toString(): string {
    const decoder = new TextDecoder();
    return decoder.decode(this.buf.subarray(this.off));
  }

  /** empty() returns whether the unread portion of the buffer is empty. */
  empty(): boolean {
    return this.buf.byteLength <= this.off;
  }

  /** length is a getter that returns the number of bytes of the unread
   * portion of the buffer
   */
  get length(): number {
    return this.buf.byteLength - this.off;
  }

  /** Returns the capacity of the buffer's underlying byte slice, that is,
   * the total space allocated for the buffer's data.
   */
  get capacity(): number {
    return this.buf.buffer.byteLength;
  }

  /** reset() resets the buffer to be empty, but it retains the underlying
   * storage for use by future writes. reset() is the same as truncate(0)
   */
  reset(): void {
    this._reslice(0);
    this.off = 0;
  }

  /** _tryGrowByReslice() is a version of grow for the fast-case
   * where the internal buffer only needs to be resliced. It returns the index
   * where bytes should be written and whether it succeeded.
   * It returns -1 if a reslice was not needed.
   */
  private _tryGrowByReslice(n: number): number {
    const l = this.buf.byteLength;
    if (n <= this.capacity - l) {
      this._reslice(l + n);
      return l;
    }
    return -1;
  }

  private _reslice(len: number): void {
    assert(len <= this.buf.buffer.byteLength);
    this.buf = new Uint8Array(this.buf.buffer, 0, len);
  }

  writeSync(p: Uint8Array): number {
    const m = this._grow(p.byteLength);
    return copyBytes(this.buf, p, m);
  }

  async write(p: Uint8Array): Promise<number> {
    const n = this.writeSync(p);
    return Promise.resolve(n);
  }

  /** _grow() grows the buffer to guarantee space for n more bytes.
   * It returns the index where bytes should be written.
   * If the buffer can't grow it will throw with ErrTooLarge.
   */
  private _grow(n: number): number {
    const m = this.length;
    // If buffer is empty, reset to recover space.
    if (m === 0 && this.off !== 0) {
      this.reset();
    }
    // Fast: Try to grow by means of a reslice.
    const i = this._tryGrowByReslice(n);
    if (i >= 0) {
      return i;
    }
    const c = this.capacity;
    if (n <= Math.floor(c / 2) - m) {
      // We can slide things down instead of allocating a new
      // ArrayBuffer. We only need m+n <= c to slide, but
      // we instead let capacity get twice as large so we
      // don't spend all our time copying.
      copyBytes(this.buf, this.buf.subarray(this.off));
    } else if (c > MAX_SIZE - c - n) {
      // throw new DenoError(
      //   ErrorKind.TooLarge,
      //   "The buffer cannot be grown beyond the maximum size."
      // );
      throw new Error();
    } else {
      // Not enough space anywhere, we need to allocate.
      const buf = new Uint8Array(2 * c + n);
      copyBytes(buf, this.buf.subarray(this.off));
      this.buf = buf;
    }
    // Restore this.off and len(this.buf).
    this.off = 0;
    this._reslice(m + n);
    return m;
  }
}

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

// TODO(kevinkassimo): Move this utility to deno_std.
// Import from there once doable.
// Read from reader until EOF and emit string chunks separated
// by the given delimiter.
async function* chunks(
  reader: Deno.Reader,
  delim: string
): AsyncIterableIterator<string> {
  const inputBuffer = new Buffer();
  const inspectArr = new Uint8Array(1024);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  // Avoid unicode problems
  const delimArr = encoder.encode(delim);

  // Record how far we have gone with delimiter matching.
  let nextMatchIndex = 0;
  while (true) {
    const rr = await reader.read(inspectArr);
    if (rr.nread < 0) {
      // Silently fail.
      break;
    }
    const sliceRead = inspectArr.subarray(0, rr.nread);
    // Remember how far we have scanned through inspectArr.
    let nextSliceStartIndex = 0;
    for (let i = 0; i < sliceRead.length; i++) {
      if (sliceRead[i] == delimArr[nextMatchIndex]) {
        // One byte matches with delimiter, move 1 step forward.
        nextMatchIndex++;
      } else {
        // Match delimiter failed. Start from beginning.
        nextMatchIndex = 0;
      }
      // A complete match is found.
      if (nextMatchIndex === delimArr.length) {
        nextMatchIndex = 0; // Reset delim match index.
        const sliceToJoin = sliceRead.subarray(nextSliceStartIndex, i + 1);
        // Record where to start next chunk when a subsequent match is found.
        nextSliceStartIndex = i + 1;
        // Write slice to buffer before processing, since potentially
        // part of the delimiter is stored in the buffer.
        await writeAll(inputBuffer, sliceToJoin);

        let readyBytes = inputBuffer.bytes();
        inputBuffer.reset();
        // Remove delimiter from buffer bytes.
        readyBytes = readyBytes.subarray(
          0,
          readyBytes.length - delimArr.length
        );
        let readyChunk = decoder.decode(readyBytes);
        yield readyChunk;
      }
    }
    // Write all unprocessed chunk to buffer for future inspection.
    await writeAll(inputBuffer, sliceRead.subarray(nextSliceStartIndex));
    if (rr.eof) {
      // Flush the remainder unprocessed chunk.
      const lastChunk = inputBuffer.toString();
      yield lastChunk;
      break;
    }
  }
}

export async function parseUnitTestOutput(
  output: Deno.Reader,
  print: boolean
): Promise<{ actual?: number; expected?: number; resultOutput?: string }> {
  // const decoder = new TextDecoder();
  // const output = decoder.decode(rawOutput);

  let expected, actual, result;

  for await (const line of chunks(output, "\n")) {
    if (!expected) {
      // expect "running 30 tests"
      expected = extractNumber(/running (\d+) tests/, line);
    } else if (line.indexOf("test result:") !== -1) {
      result = line;
    }

    if (print) {
      console.log(line);
    }
  }

  // Check that the number of expected tests equals what was reported at the
  // bottom.
  if (result) {
    // result should be a string like this:
    // "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ..."
    actual = extractNumber(/(\d+) passed/, result);
  }

  return { actual, expected, resultOutput: result };
}

test(function permissionsMatches(): void {
  assert(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: false,
        env: false,
        run: false,
        highPrecision: false
      },
      normalizeTestPermissions({ read: true })
    )
  );

  assert(
    permissionsMatch(
      {
        read: false,
        write: false,
        net: false,
        env: false,
        run: false,
        highPrecision: false
      },
      normalizeTestPermissions({})
    )
  );

  assertEquals(
    permissionsMatch(
      {
        read: false,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      },
      normalizeTestPermissions({ read: true })
    ),
    false
  );

  assertEquals(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: true,
        env: false,
        run: false,
        highPrecision: false
      },
      normalizeTestPermissions({ read: true })
    ),
    false
  );

  assert(
    permissionsMatch(
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      },
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      }
    )
  );
});

// testPerm({ read: true }, async function parsingUnitTestOutput(): Promise<void> {
//   const cwd = Deno.cwd();
//   const testDataPath = `${cwd}/tools/testdata/`;
//
//   let result;
//
//   // This is an example of a successful unit test output.
//   result = parseUnitTestOutput(
//     await Deno.readFile(`${testDataPath}/unit_test_output1.txt`),
//     false
//   );
//   assertEquals(result.actual, 96);
//   assertEquals(result.expected, 96);
//
//   // This is an example of a silently dying unit test.
//   result = parseUnitTestOutput(
//     await Deno.readFile(`${testDataPath}/unit_test_output2.txt`),
//     false
//   );
//   assertEquals(result.actual, undefined);
//   assertEquals(result.expected, 96);
//
//   // This is an example of compiling before successful unit tests.
//   result = parseUnitTestOutput(
//     await Deno.readFile(`${testDataPath}/unit_test_output3.txt`),
//     false
//   );
//   assertEquals(result.actual, 96);
//   assertEquals(result.expected, 96);
//
//   // Check what happens on empty output.
//   result = parseUnitTestOutput(new TextEncoder().encode("\n\n\n"), false);
//   assertEquals(result.actual, undefined);
//   assertEquals(result.expected, undefined);
// });
