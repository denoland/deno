// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";

export interface TraceInfo {
  sync: boolean; // is synchronous call
  name: string; // name of operation
}

interface TraceListStackNode {
  list: TraceInfo[];
  prevStackNode: TraceListStackNode | null;
}

let currTraceListStackNode: TraceListStackNode | null = null;

// Push a new list to trace stack
export function pushTraceStack(): void {
  if (currTraceListStackNode === null) {
    currTraceListStackNode = { list: [], prevStackNode: null };
  } else {
    const newStack = { list: [], prevStackNode: currTraceListStackNode };
    currTraceListStackNode = newStack;
  }
}

// Pop from trace stack and (if possible) concat to parent trace stack node
export function popTraceStack(): TraceInfo[] {
  if (currTraceListStackNode === null) {
    throw new Error("trace list stack should not be empty");
  }
  const resultList = currTraceListStackNode!.list;
  if (!!currTraceListStackNode!.prevStackNode) {
    const prevStackNode = currTraceListStackNode!.prevStackNode!;
    // concat inner results to outer stack
    prevStackNode.list = prevStackNode.list.concat(resultList);
    currTraceListStackNode = prevStackNode;
  } else {
    currTraceListStackNode = null;
  }
  return resultList;
}

// Push to trace stack if we are tracing
export function maybePushTrace(op: fbs.Any, sync: boolean): void {
  if (currTraceListStackNode === null) {
    return; // no trace requested
  }
  // Freeze the object, avoid tampering
  currTraceListStackNode!.list.push(
    Object.freeze({
      sync,
      name: fbs.Any[op] // convert to enum names
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
  pushTraceStack();
  if (typeof fnOrPromise === "function") {
    await fnOrPromise();
  } else {
    await fnOrPromise;
  }
  return popTraceStack();
}
