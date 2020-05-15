// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export declare type ErrorFactory = (kind: number, msg: string) => Error;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export declare type Ok = any;

interface Deferred<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

declare enum InternalErrorKinds {
  JsonIoError = 1,
  JsonSyntaxError = 2,
  JsonDataError = 3,
  JsonEofError = 4,
}

interface JsonError {
  kind: number;
  message: string;
}

interface JsonResponse {
  ok?: Ok;
  err?: JsonError;
  promiseId?: number;
}

/** Json based dispatch wrapper for core ops.
 *
 * Error kind mapping is controlled by errorFactory. Async handler is automatically
 * set during construction.
 *
 *       const opId = Deno.ops()["json_op"];
 *       const jsonOp = new DispatchJsonOp(opId, (kind, msg) => return new CustomError(kind, msg));
 *       const response = jsonOp.dispatchSync({ data });
 *       console.log(response.items[3].name);
 */
export declare class DispatchJsonOp {
  protected readonly promiseTable: Map<number, Deferred<JsonResponse>>;
  protected _nextPromiseId: number;
  constructor(opId: number, errorFactory: ErrorFactory);
  protected nextPromiseId(): number;
  protected unwrapResponse(res: JsonResponse): Ok;
  protected handleAsync(resUi8: Uint8Array): void;
  dispatchSync(args?: object, zeroCopy?: Uint8Array): Ok;
  dispatchAsync(args?: object, zeroCopy?: Uint8Array): Promise<Ok>;
}
