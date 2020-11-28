// Copyright Node.js contributors. All rights reserved. MIT License.
import { once } from "../_utils.ts";
import { destroyer as implDestroyer } from "./destroy.ts";
import eos from "./end_of_stream.ts";
import createReadableStreamAsyncIterator from "./async_iterator.ts";
import * as events from "../events.ts";
import PassThrough from "./passthrough.ts";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_CALLBACK,
  ERR_INVALID_RETURN_VALUE,
  ERR_MISSING_ARGS,
  ERR_STREAM_DESTROYED,
  NodeErrorAbstraction,
} from "../_errors.ts";
import type Duplex from "./duplex.ts";
import type Readable from "./readable.ts";
import type Stream from "./stream.ts";
import type Transform from "./transform.ts";
import type Writable from "./writable.ts";

type Streams = Duplex | Readable | Writable;
// deno-lint-ignore no-explicit-any
type EndCallback = (err?: NodeErrorAbstraction | null, val?: any) => void;
type TransformCallback =
  // deno-lint-ignore no-explicit-any
  | ((value?: any) => AsyncGenerator<any>)
  // deno-lint-ignore no-explicit-any
  | ((value?: any) => Promise<any>);
/**
 * This type represents an array that contains a data source,
 * many Transform Streams, a writable stream destination
 * and end in an optional callback
 * */
type DataSource =
  // deno-lint-ignore no-explicit-any
  | (() => AsyncGenerator<any>)
  | // deno-lint-ignore no-explicit-any
  AsyncIterable<any>
  | Duplex
  | // deno-lint-ignore no-explicit-any
  Iterable<any>
  | // deno-lint-ignore no-explicit-any
  (() => Generator<any>)
  | Readable;
type Transformers = Duplex | Transform | TransformCallback | Writable;
export type PipelineArguments = [
  DataSource,
  ...Array<Transformers | EndCallback>,
];

function destroyer(
  stream: Streams,
  reading: boolean,
  writing: boolean,
  callback: EndCallback,
) {
  callback = once(callback);

  let finished = false;
  stream.on("close", () => {
    finished = true;
  });

  eos(stream, { readable: reading, writable: writing }, (err) => {
    finished = !err;

    // deno-lint-ignore no-explicit-any
    const rState = (stream as any)?._readableState;
    if (
      err &&
      err.code === "ERR_STREAM_PREMATURE_CLOSE" &&
      reading &&
      (rState?.ended && !rState?.errored && !rState?.errorEmitted)
    ) {
      stream
        .once("end", callback)
        .once("error", callback);
    } else {
      callback(err);
    }
  });

  return (err: NodeErrorAbstraction) => {
    if (finished) return;
    finished = true;
    implDestroyer(stream, err);
    callback(err || new ERR_STREAM_DESTROYED("pipe"));
  };
}

function popCallback(streams: PipelineArguments): EndCallback {
  if (typeof streams[streams.length - 1] !== "function") {
    throw new ERR_INVALID_CALLBACK(streams[streams.length - 1]);
  }
  return streams.pop() as EndCallback;
}

// function isPromise(obj) {
//   return !!(obj && typeof obj.then === "function");
// }

// deno-lint-ignore no-explicit-any
function isReadable(obj: any): obj is Stream {
  return !!(obj && typeof obj.pipe === "function");
}

// deno-lint-ignore no-explicit-any
function isWritable(obj: any) {
  return !!(obj && typeof obj.write === "function");
}

// deno-lint-ignore no-explicit-any
function isStream(obj: any) {
  return isReadable(obj) || isWritable(obj);
}

// deno-lint-ignore no-explicit-any
function isIterable(obj: any, isAsync?: boolean) {
  if (!obj) return false;
  if (isAsync === true) return typeof obj[Symbol.asyncIterator] === "function";
  if (isAsync === false) return typeof obj[Symbol.iterator] === "function";
  return typeof obj[Symbol.asyncIterator] === "function" ||
    typeof obj[Symbol.iterator] === "function";
}

// deno-lint-ignore no-explicit-any
function makeAsyncIterable(val: Readable | Iterable<any> | AsyncIterable<any>) {
  if (isIterable(val)) {
    return val;
  } else if (isReadable(val)) {
    return fromReadable(val as Readable);
  }
  throw new ERR_INVALID_ARG_TYPE(
    "val",
    ["Readable", "Iterable", "AsyncIterable"],
    val,
  );
}

async function* fromReadable(val: Readable) {
  yield* createReadableStreamAsyncIterator(val);
}

async function pump(
  // deno-lint-ignore no-explicit-any
  iterable: Iterable<any>,
  writable: Duplex | Writable,
  finish: (err?: NodeErrorAbstraction | null) => void,
) {
  let error;
  try {
    for await (const chunk of iterable) {
      if (!writable.write(chunk)) {
        if (writable.destroyed) return;
        await events.once(writable, "drain");
      }
    }
    writable.end();
  } catch (err) {
    error = err;
  } finally {
    finish(error);
  }
}

export default function pipeline(...args: PipelineArguments) {
  const callback: EndCallback = once(popCallback(args));

  let streams: [DataSource, ...Transformers[]];
  if (args.length > 1) {
    streams = args as [DataSource, ...Transformers[]];
  } else {
    throw new ERR_MISSING_ARGS("streams");
  }

  let error: NodeErrorAbstraction;
  // deno-lint-ignore no-explicit-any
  let value: any;
  const destroys: Array<(err: NodeErrorAbstraction) => void> = [];

  let finishCount = 0;

  function finish(err?: NodeErrorAbstraction | null) {
    const final = --finishCount === 0;

    if (err && (!error || error.code === "ERR_STREAM_PREMATURE_CLOSE")) {
      error = err;
    }

    if (!error && !final) {
      return;
    }

    while (destroys.length) {
      (destroys.shift() as (err: NodeErrorAbstraction) => void)(error);
    }

    if (final) {
      callback(error, value);
    }
  }

  // TODO(Soremwar)
  // Simplify the hell out of this
  // deno-lint-ignore no-explicit-any
  let ret: any;
  for (let i = 0; i < streams.length; i++) {
    const stream = streams[i];
    const reading = i < streams.length - 1;
    const writing = i > 0;

    if (isStream(stream)) {
      finishCount++;
      destroys.push(destroyer(stream as Streams, reading, writing, finish));
    }

    if (i === 0) {
      if (typeof stream === "function") {
        ret = stream();
        if (!isIterable(ret)) {
          throw new ERR_INVALID_RETURN_VALUE(
            "Iterable, AsyncIterable or Stream",
            "source",
            ret,
          );
        }
      } else if (isIterable(stream) || isReadable(stream)) {
        ret = stream;
      } else {
        throw new ERR_INVALID_ARG_TYPE(
          "source",
          ["Stream", "Iterable", "AsyncIterable", "Function"],
          stream,
        );
      }
    } else if (typeof stream === "function") {
      ret = makeAsyncIterable(ret);
      ret = stream(ret);

      if (reading) {
        if (!isIterable(ret, true)) {
          throw new ERR_INVALID_RETURN_VALUE(
            "AsyncIterable",
            `transform[${i - 1}]`,
            ret,
          );
        }
      } else {
        // If the last argument to pipeline is not a stream
        // we must create a proxy stream so that pipeline(...)
        // always returns a stream which can be further
        // composed through `.pipe(stream)`.
        const pt = new PassThrough({
          objectMode: true,
        });
        if (ret instanceof Promise) {
          ret
            .then((val) => {
              value = val;
              pt.end(val);
            }, (err) => {
              pt.destroy(err);
            });
        } else if (isIterable(ret, true)) {
          finishCount++;
          pump(ret, pt, finish);
        } else {
          throw new ERR_INVALID_RETURN_VALUE(
            "AsyncIterable or Promise",
            "destination",
            ret,
          );
        }

        ret = pt;

        finishCount++;
        destroys.push(destroyer(ret, false, true, finish));
      }
    } else if (isStream(stream)) {
      if (isReadable(ret)) {
        ret.pipe(stream as Readable);

        // TODO(Soremwar)
        // Reimplement after stdout and stderr are implemented
        // if (stream === process.stdout || stream === process.stderr) {
        //   ret.on("end", () => stream.end());
        // }
      } else {
        ret = makeAsyncIterable(ret);

        finishCount++;
        pump(ret, stream as Writable, finish);
      }
      ret = stream;
    } else {
      const name = reading ? `transform[${i - 1}]` : "destination";
      throw new ERR_INVALID_ARG_TYPE(
        name,
        ["Stream", "Function"],
        ret,
      );
    }
  }

  return ret as unknown as Readable;
}
