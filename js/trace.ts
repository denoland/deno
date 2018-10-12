// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";

export interface TraceInfo {
  sync: boolean; // is synchronous call
  name: string; // name of operation
}

interface TraceStackNode {
  list: TraceInfo[];
  prev: TraceStackNode | null;
}

let current: TraceStackNode | null = null;

// Push a new list to trace stack
function pushStack(): void {
  if (current === null) {
    current = { list: [], prev: null };
  } else {
    const newStack = { list: [], prev: current };
    current = newStack;
  }
}

// Pop from trace stack and (if possible) concat to parent trace stack node
function popStack(): TraceInfo[] {
  if (current === null) {
    throw new Error("trace list stack should not be empty");
  }
  const resultList = current!.list;
  if (!!current!.prev) {
    const prev = current!.prev!;
    // concat inner results to outer stack
    prev.list = prev.list.concat(resultList);
    current = prev;
  } else {
    current = null;
  }
  return resultList;
}

// Push to trace stack if we are tracing
// @internal
export function maybePushTrace(op: msg.Any, sync: boolean): void {
  if (current === null) {
    return; // no trace requested
  }
  // Freeze the object, avoid tampering
  current!.list.push(
    Object.freeze({
      sync,
      name: msg.Any[op] // convert to enum names
    })
  );
}

/**
 * Trace operations executed inside a given function or promise.
 * Notice: To capture every operation in asynchronous deno.* calls,
 * you might want to put them in functions instead of directly invoking.
 *
 *     import { trace, mkdir } from "deno";
 *
 *     const ops = await trace(async () => {
 *       await mkdir("my_dir");
 *     });
 *     // ops becomes [{ sync: false, name: "Mkdir" }]
 */
export async function trace(
  // tslint:disable-next-line:no-any
  fnOrPromise: Function | Promise<any>
): Promise<TraceInfo[]> {
  pushStack();
  if (typeof fnOrPromise === "function") {
    await fnOrPromise();
  } else {
    await fnOrPromise;
  }
  return popStack();
}
