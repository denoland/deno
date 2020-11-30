// Copyright Node.js contributors. All rights reserved. MIT License.
import eos from "./end_of_stream.ts";
import type {
  FinishedOptions,
  StreamImplementations as FinishedStreams,
} from "./end_of_stream.ts";
import pl from "./pipeline.ts";
import type { PipelineArguments } from "./pipeline.ts";

export function pipeline(...streams: PipelineArguments) {
  return new Promise((resolve, reject) => {
    pl(
      ...streams,
      (err, value) => {
        if (err) {
          reject(err);
        } else {
          resolve(value);
        }
      },
    );
  });
}

export function finished(
  stream: FinishedStreams,
  opts?: FinishedOptions,
) {
  return new Promise<void>((resolve, reject) => {
    eos(
      stream,
      opts || null,
      (err) => {
        if (err) {
          reject(err);
        } else {
          resolve();
        }
      },
    );
  });
}
