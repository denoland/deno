import { libdeno } from "./libdeno";

const promiseRejectEvents = libdeno.constants.promiseRejectEvents;

/* tslint:disable-next-line:no-any */
const promiseRejectMap = new Map<Promise<any>, string>();
/* tslint:disable-next-line:no-any */
const otherPromiseErrorMap = new Map<Promise<any>, string>();

export function promiseRejectHandler(
  /* tslint:disable-next-line:no-any */
  error: any,
  event: number,
  /* tslint:disable-next-line:no-any */
  promise: Promise<any>
) {
  switch (event) {
    case promiseRejectEvents.kPromiseRejectWithNoHandler:
      promiseRejectMap.set(
        promise,
        error.stack || "PromiseRejectWithNoHandler"
      );
      break;
    case promiseRejectEvents.kPromiseHandlerAddedAfterReject:
      promiseRejectMap.delete(promise);
      break;
    default:
      // error is string here
      otherPromiseErrorMap.set(promise, `Promise error: ${error}`);
  }
}

// Return 0 when continue, 1 to die
export function promiseErrorExaminer(): number {
  if (otherPromiseErrorMap.size > 0) {
    for (const msg of otherPromiseErrorMap.values()) {
      console.log(msg);
    }
    otherPromiseErrorMap.clear();
  }
  if (promiseRejectMap.size > 0) {
    for (const msg of promiseRejectMap.values()) {
      console.log(msg);
    }
    promiseRejectMap.clear();
    return 1;
  }
  return 0;
}
