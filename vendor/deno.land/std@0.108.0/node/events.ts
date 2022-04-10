// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2019 Denolibs authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { assert } from "../_util/assert.ts";
import { ERR_INVALID_ARG_TYPE, ERR_OUT_OF_RANGE } from "./_errors.ts";
import { inspect } from "./util.ts";

// deno-lint-ignore no-explicit-any
export type GenericFunction = (...args: any[]) => any;

export interface WrappedFunction extends Function {
  listener: GenericFunction;
}

function ensureArray<T>(maybeArray: T[] | T): T[] {
  return Array.isArray(maybeArray) ? maybeArray : [maybeArray];
}

// deno-lint-ignore no-explicit-any
function createIterResult(value: any, done: boolean): IteratorResult<any> {
  return { value, done };
}

interface AsyncIterable {
  // deno-lint-ignore no-explicit-any
  next(): Promise<IteratorResult<any, any>>;
  // deno-lint-ignore no-explicit-any
  return(): Promise<IteratorResult<any, any>>;
  throw(err: Error): void;
  // deno-lint-ignore no-explicit-any
  [Symbol.asyncIterator](): any;
}

type EventMap = Record<
  string | symbol,
  (
    | (Array<GenericFunction | WrappedFunction>)
    | GenericFunction
    | WrappedFunction
  ) & { warned?: boolean }
>;

export let defaultMaxListeners = 10;
function validateMaxListeners(n: number, name: string): void {
  if (!Number.isInteger(n) || n < 0) {
    throw new ERR_OUT_OF_RANGE(name, "a non-negative number", inspect(n));
  }
}

/**
 * See also https://nodejs.org/api/events.html
 */
export class EventEmitter {
  public static captureRejectionSymbol = Symbol.for("nodejs.rejection");
  public static errorMonitor = Symbol("events.errorMonitor");
  public static get defaultMaxListeners() {
    return defaultMaxListeners;
  }
  public static set defaultMaxListeners(value: number) {
    validateMaxListeners(value, "defaultMaxListeners");
    defaultMaxListeners = value;
  }

  private maxListeners: number | undefined;
  private _events: EventMap;

  public constructor() {
    this._events = Object.create(null);
  }

  private _addListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
    prepend: boolean,
  ): this {
    this.checkListenerArgument(listener);
    this.emit("newListener", eventName, this.unwrapListener(listener));
    if (this.hasListeners(eventName)) {
      let listeners = this._events[eventName];
      if (!Array.isArray(listeners)) {
        listeners = [listeners];
        this._events[eventName] = listeners;
      }

      if (prepend) {
        listeners.unshift(listener);
      } else {
        listeners.push(listener);
      }
    } else {
      this._events[eventName] = listener;
    }
    const max = this.getMaxListeners();
    if (max > 0 && this.listenerCount(eventName) > max) {
      const warning = new MaxListenersExceededWarning(this, eventName);
      this.warnIfNeeded(eventName, warning);
    }

    return this;
  }

  /** Alias for emitter.on(eventName, listener). */
  addListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
  ): this {
    return this._addListener(eventName, listener, false);
  }

  /**
   * Synchronously calls each of the listeners registered for the event named
   * eventName, in the order they were registered, passing the supplied
   * arguments to each.
   * @return true if the event had listeners, false otherwise
   */
  // deno-lint-ignore no-explicit-any
  public emit(eventName: string | symbol, ...args: any[]): boolean {
    if (this.hasListeners(eventName)) {
      if (
        eventName === "error" &&
        this.hasListeners(EventEmitter.errorMonitor)
      ) {
        this.emit(EventEmitter.errorMonitor, ...args);
      }

      const listeners = ensureArray(this._events[eventName]!)
        .slice() as Array<GenericFunction>; // We copy with slice() so array is not mutated during emit
      for (const listener of listeners) {
        try {
          listener.apply(this, args);
        } catch (err) {
          this.emit("error", err);
        }
      }
      return true;
    } else if (eventName === "error") {
      if (this.hasListeners(EventEmitter.errorMonitor)) {
        this.emit(EventEmitter.errorMonitor, ...args);
      }
      const errMsg = args.length > 0 ? args[0] : Error("Unhandled error.");
      throw errMsg;
    }
    return false;
  }

  /**
   * Returns an array listing the events for which the emitter has
   * registered listeners.
   */
  public eventNames(): [string | symbol] {
    return Reflect.ownKeys(this._events) as [string | symbol];
  }

  /**
   * Returns the current max listener value for the EventEmitter which is
   * either set by emitter.setMaxListeners(n) or defaults to
   * EventEmitter.defaultMaxListeners.
   */
  public getMaxListeners(): number {
    return this.maxListeners == null
      ? EventEmitter.defaultMaxListeners
      : this.maxListeners;
  }

  /**
   * Returns the number of listeners listening to the event named
   * eventName.
   */
  public listenerCount(eventName: string | symbol): number {
    if (this.hasListeners(eventName)) {
      const maybeListeners = this._events[eventName];
      return Array.isArray(maybeListeners) ? maybeListeners.length : 1;
    } else {
      return 0;
    }
  }

  static listenerCount(
    emitter: EventEmitter,
    eventName: string | symbol,
  ): number {
    return emitter.listenerCount(eventName);
  }

  private _listeners(
    target: EventEmitter,
    eventName: string | symbol,
    unwrap: boolean,
  ): GenericFunction[] {
    if (!target.hasListeners(eventName)) {
      return [];
    }

    const eventListeners = target._events[eventName];
    if (Array.isArray(eventListeners)) {
      return unwrap
        ? this.unwrapListeners(eventListeners)
        : eventListeners.slice(0) as GenericFunction[];
    } else {
      return [
        unwrap ? this.unwrapListener(eventListeners) : eventListeners,
      ] as GenericFunction[];
    }
  }

  private unwrapListeners(
    arr: (GenericFunction | WrappedFunction)[],
  ): GenericFunction[] {
    const unwrappedListeners = new Array(arr.length) as GenericFunction[];
    for (let i = 0; i < arr.length; i++) {
      unwrappedListeners[i] = this.unwrapListener(arr[i]);
    }
    return unwrappedListeners;
  }

  private unwrapListener(
    listener: GenericFunction | WrappedFunction,
  ): GenericFunction {
    return (listener as WrappedFunction)["listener"] ?? listener;
  }

  /** Returns a copy of the array of listeners for the event named eventName.*/
  public listeners(eventName: string | symbol): GenericFunction[] {
    return this._listeners(this, eventName, true);
  }

  /**
   * Returns a copy of the array of listeners for the event named eventName,
   * including any wrappers (such as those created by .once()).
   */
  public rawListeners(
    eventName: string | symbol,
  ): Array<GenericFunction | WrappedFunction> {
    return this._listeners(this, eventName, false);
  }

  /** Alias for emitter.removeListener(). */
  public off(
    // deno-lint-ignore no-unused-vars
    eventName: string | symbol,
    // deno-lint-ignore no-unused-vars
    listener: GenericFunction,
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
  ): this {
    // The body of this method is empty because it will be overwritten by later code. (`EventEmitter.prototype.off = EventEmitter.prototype.removeListener;`)
    // The purpose of this dirty hack is to get around the current limitation of TypeScript type checking.
  }

  /**
   * Adds the listener function to the end of the listeners array for the event
   *  named eventName. No checks are made to see if the listener has already
   * been added. Multiple calls passing the same combination of eventName and
   * listener will result in the listener being added, and called, multiple
   * times.
   */
  public on(
    // deno-lint-ignore no-unused-vars
    eventName: string | symbol,
    // deno-lint-ignore no-unused-vars
    listener: GenericFunction | WrappedFunction,
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
  ): this {
    // The body of this method is empty because it will be overwritten by later code. (`EventEmitter.prototype.addListener = EventEmitter.prototype.on;`)
    // The purpose of this dirty hack is to get around the current limitation of TypeScript type checking.
  }

  /**
   * Adds a one-time listener function for the event named eventName. The next
   * time eventName is triggered, this listener is removed and then invoked.
   */
  public once(eventName: string | symbol, listener: GenericFunction): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.on(eventName, wrapped);
    return this;
  }

  // Wrapped function that calls EventEmitter.removeListener(eventName, self) on execution.
  private onceWrap(
    eventName: string | symbol,
    listener: GenericFunction,
  ): WrappedFunction {
    this.checkListenerArgument(listener);
    const wrapper = function (
      this: {
        eventName: string | symbol;
        listener: GenericFunction;
        rawListener: GenericFunction | WrappedFunction;
        context: EventEmitter;
        isCalled?: boolean;
      },
      // deno-lint-ignore no-explicit-any
      ...args: any[]
    ): void {
      // If `emit` is called in listeners, the same listener can be called multiple times.
      // To prevent that, check the flag here.
      if (this.isCalled) {
        return;
      }
      this.context.removeListener(
        this.eventName,
        this.listener as GenericFunction,
      );
      this.isCalled = true;
      return this.listener.apply(this.context, args);
    };
    const wrapperContext = {
      eventName: eventName,
      listener: listener,
      rawListener: (wrapper as unknown) as WrappedFunction,
      context: this,
    };
    const wrapped = (wrapper.bind(
      wrapperContext,
    ) as unknown) as WrappedFunction;
    wrapperContext.rawListener = wrapped;
    wrapped.listener = listener;
    return wrapped as WrappedFunction;
  }

  /**
   * Adds the listener function to the beginning of the listeners array for the
   *  event named eventName. No checks are made to see if the listener has
   * already been added. Multiple calls passing the same combination of
   * eventName and listener will result in the listener being added, and
   * called, multiple times.
   */
  public prependListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
  ): this {
    return this._addListener(eventName, listener, true);
  }

  /**
   * Adds a one-time listener function for the event named eventName to the
   * beginning of the listeners array. The next time eventName is triggered,
   * this listener is removed, and then invoked.
   */
  public prependOnceListener(
    eventName: string | symbol,
    listener: GenericFunction,
  ): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.prependListener(eventName, wrapped);
    return this;
  }

  /** Removes all listeners, or those of the specified eventName. */
  public removeAllListeners(eventName?: string | symbol): this {
    if (this._events === undefined) {
      return this;
    }

    if (eventName) {
      if (this.hasListeners(eventName)) {
        const listeners = ensureArray(this._events[eventName]).slice()
          .reverse();
        for (const listener of listeners) {
          this.removeListener(
            eventName,
            this.unwrapListener(listener),
          );
        }
      }
    } else {
      const eventList = this.eventNames();
      eventList.forEach((eventName: string | symbol) => {
        if (eventName === "removeListener") return;
        this.removeAllListeners(eventName);
      });
      this.removeAllListeners("removeListener");
    }

    return this;
  }

  /**
   * Removes the specified listener from the listener array for the event
   * named eventName.
   */
  public removeListener(
    eventName: string | symbol,
    listener: GenericFunction,
  ): this {
    this.checkListenerArgument(listener);
    if (this.hasListeners(eventName)) {
      const maybeArr = this._events[eventName];

      assert(maybeArr);
      const arr = ensureArray(maybeArr);

      let listenerIndex = -1;
      for (let i = arr.length - 1; i >= 0; i--) {
        // arr[i]["listener"] is the reference to the listener inside a bound 'once' wrapper
        if (
          arr[i] == listener ||
          (arr[i] && (arr[i] as WrappedFunction)["listener"] == listener)
        ) {
          listenerIndex = i;
          break;
        }
      }

      if (listenerIndex >= 0) {
        arr.splice(listenerIndex, 1);
        if (arr.length === 0) {
          delete this._events[eventName];
        } else if (arr.length === 1) {
          // If there is only one listener, an array is not necessary.
          this._events[eventName] = arr[0];
        }

        if (this._events.removeListener) {
          this.emit("removeListener", eventName, listener);
        }
      }
    }
    return this;
  }

  /**
   * By default EventEmitters will print a warning if more than 10 listeners
   * are added for a particular event. This is a useful default that helps
   * finding memory leaks. Obviously, not all events should be limited to just
   * 10 listeners. The emitter.setMaxListeners() method allows the limit to be
   * modified for this specific EventEmitter instance. The value can be set to
   * Infinity (or 0) to indicate an unlimited number of listeners.
   */
  public setMaxListeners(n: number): this {
    if (n !== Infinity) {
      validateMaxListeners(n, "n");
    }

    this.maxListeners = n;
    return this;
  }

  /**
   * Creates a Promise that is fulfilled when the EventEmitter emits the given
   * event or that is rejected when the EventEmitter emits 'error'. The Promise
   * will resolve with an array of all the arguments emitted to the given event.
   */
  public static once(
    emitter: EventEmitter | EventTarget,
    name: string,
    // deno-lint-ignore no-explicit-any
  ): Promise<any[]> {
    return new Promise((resolve, reject) => {
      if (emitter instanceof EventTarget) {
        // EventTarget does not have `error` event semantics like Node
        // EventEmitters, we do not listen to `error` events here.
        emitter.addEventListener(
          name,
          (...args) => {
            resolve(args);
          },
          { once: true, passive: false, capture: false },
        );
        return;
      } else if (emitter instanceof EventEmitter) {
        // deno-lint-ignore no-explicit-any
        const eventListener = (...args: any[]): void => {
          if (errorListener !== undefined) {
            emitter.removeListener("error", errorListener);
          }
          resolve(args);
        };
        let errorListener: GenericFunction;

        // Adding an error listener is not optional because
        // if an error is thrown on an event emitter we cannot
        // guarantee that the actual event we are waiting will
        // be fired. The result could be a silent way to create
        // memory or file descriptor leaks, which is something
        // we should avoid.
        if (name !== "error") {
          // deno-lint-ignore no-explicit-any
          errorListener = (err: any): void => {
            emitter.removeListener(name, eventListener);
            reject(err);
          };

          emitter.once("error", errorListener);
        }

        emitter.once(name, eventListener);
        return;
      }
    });
  }

  /**
   * Returns an AsyncIterator that iterates eventName events. It will throw if
   * the EventEmitter emits 'error'. It removes all listeners when exiting the
   * loop. The value returned by each iteration is an array composed of the
   * emitted event arguments.
   */
  public static on(
    emitter: EventEmitter,
    event: string | symbol,
  ): AsyncIterable {
    // deno-lint-ignore no-explicit-any
    const unconsumedEventValues: any[] = [];
    // deno-lint-ignore no-explicit-any
    const unconsumedPromises: any[] = [];
    let error: Error | null = null;
    let finished = false;

    const iterator = {
      // deno-lint-ignore no-explicit-any
      next(): Promise<IteratorResult<any>> {
        // First, we consume all unread events
        // deno-lint-ignore no-explicit-any
        const value: any = unconsumedEventValues.shift();
        if (value) {
          return Promise.resolve(createIterResult(value, false));
        }

        // Then we error, if an error happened
        // This happens one time if at all, because after 'error'
        // we stop listening
        if (error) {
          const p: Promise<never> = Promise.reject(error);
          // Only the first element errors
          error = null;
          return p;
        }

        // If the iterator is finished, resolve to done
        if (finished) {
          return Promise.resolve(createIterResult(undefined, true));
        }

        // Wait until an event happens
        return new Promise(function (resolve, reject) {
          unconsumedPromises.push({ resolve, reject });
        });
      },

      // deno-lint-ignore no-explicit-any
      return(): Promise<IteratorResult<any>> {
        emitter.removeListener(event, eventHandler);
        emitter.removeListener("error", errorHandler);
        finished = true;

        for (const promise of unconsumedPromises) {
          promise.resolve(createIterResult(undefined, true));
        }

        return Promise.resolve(createIterResult(undefined, true));
      },

      throw(err: Error): void {
        error = err;
        emitter.removeListener(event, eventHandler);
        emitter.removeListener("error", errorHandler);
      },

      // deno-lint-ignore no-explicit-any
      [Symbol.asyncIterator](): any {
        return this;
      },
    };

    emitter.on(event, eventHandler);
    emitter.on("error", errorHandler);

    return iterator;

    // deno-lint-ignore no-explicit-any
    function eventHandler(...args: any[]): void {
      const promise = unconsumedPromises.shift();
      if (promise) {
        promise.resolve(createIterResult(args, false));
      } else {
        unconsumedEventValues.push(args);
      }
    }

    // deno-lint-ignore no-explicit-any
    function errorHandler(err: any): void {
      finished = true;

      const toError = unconsumedPromises.shift();
      if (toError) {
        toError.reject(err);
      } else {
        // The next time we call next()
        error = err;
      }

      iterator.return();
    }
  }

  private checkListenerArgument(listener: unknown): void {
    if (typeof listener !== "function") {
      throw new ERR_INVALID_ARG_TYPE("listener", "function", listener);
    }
  }

  private warnIfNeeded(eventName: string | symbol, warning: Error): void {
    const listeners = this._events[eventName];
    if (listeners.warned) {
      return;
    }
    listeners.warned = true;
    console.warn(warning);

    // TODO(uki00a): Here are two problems:
    // * If `global.ts` is not imported, then `globalThis.process` will be undefined.
    // * Importing `process.ts` from this file will result in circular reference.
    // As a workaround, explicitly check for the existence of `globalThis.process`.
    // deno-lint-ignore no-explicit-any
    const maybeProcess = (globalThis as any).process;
    if (maybeProcess instanceof EventEmitter) {
      maybeProcess.emit("warning", warning);
    }
  }

  private hasListeners(eventName: string | symbol): boolean {
    return this._events && Boolean(this._events[eventName]);
  }
}

// EventEmitter#on should point to the same function as EventEmitter#addListener.
EventEmitter.prototype.on = EventEmitter.prototype.addListener;
// EventEmitter#off should point to the same function as EventEmitter#removeListener.
EventEmitter.prototype.off = EventEmitter.prototype.removeListener;

class MaxListenersExceededWarning extends Error {
  readonly count: number;
  constructor(
    readonly emitter: EventEmitter,
    readonly type: string | symbol,
  ) {
    const listenerCount = emitter.listenerCount(type);
    const message = "Possible EventEmitter memory leak detected. " +
      `${listenerCount} ${
        type == null ? "null" : type.toString()
      } listeners added to [${emitter.constructor.name}]. ` +
      " Use emitter.setMaxListeners() to increase limit";
    super(message);
    this.count = listenerCount;
    this.name = "MaxListenersExceededWarning";
  }
}

export default Object.assign(EventEmitter, { EventEmitter });

export const captureRejectionSymbol = EventEmitter.captureRejectionSymbol;
export const errorMonitor = EventEmitter.errorMonitor;
export const listenerCount = EventEmitter.listenerCount;
export const on = EventEmitter.on;
export const once = EventEmitter.once;
