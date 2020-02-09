// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
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

import { validateIntegerRange } from "./util.ts";

export interface WrappedFunction extends Function {
  listener: Function;
}

export default class EventEmitter {
  public static defaultMaxListeners = 10;
  private maxListeners: number | undefined;
  private _events: Map<string | symbol, Array<Function | WrappedFunction>>;

  public constructor() {
    this._events = new Map();
  }

  private _addListener(
    eventName: string | symbol,
    listener: Function | WrappedFunction,
    prepend: boolean
  ): this {
    this.emit("newListener", eventName, listener);
    if (this._events.has(eventName)) {
      const listeners = this._events.get(eventName) as Array<
        Function | WrappedFunction
      >;
      if (prepend) {
        listeners.unshift(listener);
      } else {
        listeners.push(listener);
      }
    } else {
      this._events.set(eventName, [listener]);
    }
    const max = this.getMaxListeners();
    if (max > 0 && this.listenerCount(eventName) > max) {
      const warning = new Error(
        `Possible EventEmitter memory leak detected.
         ${this.listenerCount(eventName)} ${eventName.toString()} listeners.
         Use emitter.setMaxListeners() to increase limit`
      );
      warning.name = "MaxListenersExceededWarning";
      console.warn(warning);
    }

    return this;
  }

  public addListener(
    eventName: string | symbol,
    listener: Function | WrappedFunction
  ): this {
    return this._addListener(eventName, listener, false);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  public emit(eventName: string | symbol, ...args: any[]): boolean {
    if (this._events.has(eventName)) {
      const listeners = (this._events.get(eventName) as Function[]).slice(); // We copy with slice() so array is not mutated during emit
      for (const listener of listeners) {
        try {
          listener.apply(this, args);
        } catch (err) {
          this.emit("error", err);
        }
      }
      return true;
    } else if (eventName === "error") {
      const errMsg = args.length > 0 ? args[0] : Error("Unhandled error.");
      throw errMsg;
    }
    return false;
  }

  public eventNames(): [string | symbol] {
    return Array.from(this._events.keys()) as [string | symbol];
  }

  public getMaxListeners(): number {
    return this.maxListeners || EventEmitter.defaultMaxListeners;
  }

  public listenerCount(eventName: string | symbol): number {
    if (this._events.has(eventName)) {
      return (this._events.get(eventName) as Function[]).length;
    } else {
      return 0;
    }
  }

  private _listeners(
    target: EventEmitter,
    eventName: string | symbol,
    unwrap: boolean
  ): Function[] {
    if (!target._events.has(eventName)) {
      return [];
    }
    const eventListeners: Function[] = target._events.get(
      eventName
    ) as Function[];

    return unwrap
      ? this.unwrapListeners(eventListeners)
      : eventListeners.slice(0);
  }

  private unwrapListeners(arr: Function[]): Function[] {
    const unwrappedListeners: Function[] = new Array(arr.length) as Function[];
    for (let i = 0; i < arr.length; i++) {
      unwrappedListeners[i] = arr[i]["listener"] || arr[i];
    }
    return unwrappedListeners;
  }

  public listeners(eventName: string | symbol): Function[] {
    return this._listeners(this, eventName, true);
  }

  public rawListeners(
    eventName: string | symbol
  ): Array<Function | WrappedFunction> {
    return this._listeners(this, eventName, false);
  }

  public off(eventName: string | symbol, listener: Function): this {
    return this.removeListener(eventName, listener);
  }

  public on(
    eventName: string | symbol,
    listener: Function | WrappedFunction
  ): this {
    return this.addListener(eventName, listener);
  }

  public once(eventName: string | symbol, listener: Function): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.on(eventName, wrapped);
    return this;
  }

  // Wrapped function that calls EventEmitter.removeListener(eventName, self) on execution.
  private onceWrap(
    eventName: string | symbol,
    listener: Function
  ): WrappedFunction {
    const wrapper = function(
      this: {
        eventName: string | symbol;
        listener: Function;
        rawListener: Function;
        context: EventEmitter;
      },
      ...args: any[] // eslint-disable-line @typescript-eslint/no-explicit-any
    ): void {
      this.context.removeListener(this.eventName, this.rawListener);
      this.listener.apply(this.context, args);
    };
    const wrapperContext = {
      eventName: eventName,
      listener: listener,
      rawListener: wrapper,
      context: this
    };
    const wrapped = wrapper.bind(wrapperContext);
    wrapperContext.rawListener = wrapped;
    wrapped.listener = listener;
    return wrapped as WrappedFunction;
  }

  public prependListener(
    eventName: string | symbol,
    listener: Function | WrappedFunction
  ): this {
    return this._addListener(eventName, listener, true);
  }

  public prependOnceListener(
    eventName: string | symbol,
    listener: Function
  ): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.prependListener(eventName, wrapped);
    return this;
  }

  public removeAllListeners(eventName?: string | symbol): this {
    if (this._events === undefined) {
      return this;
    }

    if (this._events.has(eventName)) {
      const listeners = (this._events.get(eventName) as Array<
        Function | WrappedFunction
      >).slice(); // Create a copy; We use it AFTER it's deleted.
      this._events.delete(eventName);
      for (const listener of listeners) {
        this.emit("removeListener", eventName, listener);
      }
    } else {
      const eventList: [string | symbol] = this.eventNames();
      eventList.map((value: string | symbol) => {
        this.removeAllListeners(value);
      });
    }

    return this;
  }

  public removeListener(eventName: string | symbol, listener: Function): this {
    if (this._events.has(eventName)) {
      const arr: Array<Function | WrappedFunction> = this._events.get(
        eventName
      );

      let listenerIndex = -1;
      for (let i = arr.length - 1; i >= 0; i--) {
        // arr[i]["listener"] is the reference to the listener inside a bound 'once' wrapper
        if (arr[i] == listener || arr[i]["listener"] == listener) {
          listenerIndex = i;
          break;
        }
      }

      if (listenerIndex >= 0) {
        arr.splice(listenerIndex, 1);
        this.emit("removeListener", eventName, listener);
        if (arr.length === 0) {
          this._events.delete(eventName);
        }
      }
    }
    return this;
  }

  public setMaxListeners(n: number): this {
    validateIntegerRange(n, "maxListeners", 0);
    this.maxListeners = n;
    return this;
  }
}

export function once(
  emitter: EventEmitter | EventTarget,
  name: string
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
        { once: true, passive: false, capture: false }
      );
      return;
    } else if (emitter instanceof EventEmitter) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const eventListener = (...args: any[]): void => {
        if (errorListener !== undefined) {
          emitter.removeListener("error", errorListener);
        }
        resolve(args);
      };
      let errorListener: Function;

      // Adding an error listener is not optional because
      // if an error is thrown on an event emitter we cannot
      // guarantee that the actual event we are waiting will
      // be fired. The result could be a silent way to create
      // memory or file descriptor leaks, which is something
      // we should avoid.
      if (name !== "error") {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
