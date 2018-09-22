import { TraceInfo, pushTraceStack, popTraceStack } from "./dispatch";

/**
 * Trace operations executed inside a given function or promise.
 * Notice: To capture every operations in asynchronous deno.* calls,
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
