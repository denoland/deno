import { libdeno } from "./libdeno";

/* tslint:disable-next-line:no-any */
const rejectMap = new Map<Promise<any>, string>();
// For uncaught promise rejection errors

/* tslint:disable-next-line:no-any */
const otherErrorMap = new Map<Promise<any>, string>();
// For reject after resolve / resolve after resolve errors

export function promiseRejectHandler(
  error: Error | string,
  event: number,
  /* tslint:disable-next-line:no-any */
  promise: Promise<any>
) {
  switch (event) {
    case libdeno.kPromiseRejectWithNoHandler:
      rejectMap.set(
        promise,
        (error as Error).stack || "PromiseRejectWithNoHandler"
      );
      break;
    case libdeno.kPromiseHandlerAddedAfterReject:
      rejectMap.delete(promise);
      break;
    default:
      // error is string here
      otherErrorMap.set(promise, `Promise warning: ${error as string}`);
  }
}

// Return true when continue, false to die on uncaught promise reject
export function promiseErrorExaminer(): boolean {
  if (otherErrorMap.size > 0) {
    for (const msg of otherErrorMap.values()) {
      console.log(msg);
    }
    otherErrorMap.clear();
  }
  if (rejectMap.size > 0) {
    for (const msg of rejectMap.values()) {
      console.log(msg);
    }
    rejectMap.clear();
    return false;
  }
  return true;
}
