// Regression test for https://github.com/denoland/deno/issues/19620
// `Readable.fromWeb`/`Writable.fromWeb`/`Duplex.fromWeb` must accept the
// global `ReadableStream`/`WritableStream` values, since `node:stream/web`
// re-exposes the same classes.
import { Duplex, Readable, Writable } from "node:stream";
import {
  ReadableStream as NodeReadableStream,
  WritableStream as NodeWritableStream,
} from "node:stream/web";

const _r1: Readable = Readable.fromWeb(new ReadableStream());
const _r2: Readable = Readable.fromWeb(new NodeReadableStream());

const _w1: Writable = Writable.fromWeb(new WritableStream());
const _w2: Writable = Writable.fromWeb(new NodeWritableStream());

const _d: Duplex = Duplex.fromWeb({
  readable: new ReadableStream(),
  writable: new WritableStream(),
});

const _toWebR: ReadableStream = Readable.toWeb(new Readable());
const _toWebW: WritableStream = Writable.toWeb(new Writable());
