// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare class DOMException extends Error {
  constructor(message?: string, name?: string);
  readonly name: string;
  readonly message: string;
}

interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

/** An event which takes place in the DOM. */
declare class Event {
  constructor(type: string, eventInitDict?: EventInit);
  /** Returns true or false depending on how event was initialized. True if
     * event goes through its target's ancestors in reverse tree order, and
     * false otherwise. */
  readonly bubbles: boolean;
  cancelBubble: boolean;
  /** Returns true or false depending on how event was initialized. Its return
     * value does not always carry meaning, but true can indicate that part of the
     * operation during which event was dispatched, can be canceled by invoking
     * the preventDefault() method. */
  readonly cancelable: boolean;
  /** Returns true or false depending on how event was initialized. True if
     * event invokes listeners past a ShadowRoot node that is the root of its
     * target, and false otherwise. */
  readonly composed: boolean;
  /** Returns the object whose event listener's callback is currently being
     * invoked. */
  readonly currentTarget: EventTarget | null;
  /** Returns true if preventDefault() was invoked successfully to indicate
     * cancellation, and false otherwise. */
  readonly defaultPrevented: boolean;
  /** Returns the event's phase, which is one of NONE, CAPTURING_PHASE,
     * AT_TARGET, and BUBBLING_PHASE. */
  readonly eventPhase: number;
  /** Returns true if event was dispatched by the user agent, and false
     * otherwise. */
  readonly isTrusted: boolean;
  /** Returns the object to which event is dispatched (its target). */
  readonly target: EventTarget | null;
  /** Returns the event's timestamp as the number of milliseconds measured
     * relative to the time origin. */
  readonly timeStamp: number;
  /** Returns the type of event, e.g. "click", "hashchange", or "submit". */
  readonly type: string;
  /** Returns the invocation target objects of event's path (objects on which
     * listeners will be invoked), except for any nodes in shadow trees of which
     * the shadow root's mode is "closed" that are not reachable from event's
     * currentTarget. */
  composedPath(): EventTarget[];
  /** If invoked when the cancelable attribute value is true, and while
     * executing a listener for the event with passive set to false, signals to
     * the operation that caused event to be dispatched that it needs to be
     * canceled. */
  preventDefault(): void;
  /** Invoking this method prevents event from reaching any registered event
     * listeners after the current one finishes running and, when dispatched in a
     * tree, also prevents event from reaching any other objects. */
  stopImmediatePropagation(): void;
  /** When dispatched in a tree, invoking this method prevents event from
     * reaching any objects other than the current object. */
  stopPropagation(): void;
  readonly AT_TARGET: number;
  readonly BUBBLING_PHASE: number;
  readonly CAPTURING_PHASE: number;
  readonly NONE: number;
  static readonly AT_TARGET: number;
  static readonly BUBBLING_PHASE: number;
  static readonly CAPTURING_PHASE: number;
  static readonly NONE: number;
}

/**
   * EventTarget is a DOM interface implemented by objects that can receive events
   * and may have listeners for them.
   */
declare class EventTarget {
  /** Appends an event listener for events whose type attribute value is type.
     * The callback argument sets the callback that will be invoked when the event
     * is dispatched.
     *
     * The options argument sets listener-specific options. For compatibility this
     * can be a boolean, in which case the method behaves exactly as if the value
     * was specified as options's capture.
     *
     * When set to true, options's capture prevents callback from being invoked
     * when the event's eventPhase attribute value is BUBBLING_PHASE. When false
     * (or not present), callback will not be invoked when event's eventPhase
     * attribute value is CAPTURING_PHASE. Either way, callback will be invoked if
     * event's eventPhase attribute value is AT_TARGET.
     *
     * When set to true, options's passive indicates that the callback will not
     * cancel the event by invoking preventDefault(). This is used to enable
     * performance optimizations described in § 2.8 Observing event listeners.
     *
     * When set to true, options's once indicates that the callback will only be
     * invoked once after which the event listener will be removed.
     *
     * The event listener is appended to target's event listener list and is not
     * appended if it has the same type, callback, and capture. */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ): void;
  /** Dispatches a synthetic event event to target and returns true if either
     * event's cancelable attribute value is false or its preventDefault() method
     * was not invoked, and false otherwise. */
  dispatchEvent(event: Event): boolean;
  /** Removes the event listener in target's event listener list with the same
     * type, callback, and options. */
  removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean,
  ): void;
  [Symbol.toStringTag]: string;
}

interface EventListener {
  (evt: Event): void | Promise<void>;
}

interface EventListenerObject {
  handleEvent(evt: Event): void | Promise<void>;
}

declare type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

interface AddEventListenerOptions extends EventListenerOptions {
  once?: boolean;
  passive?: boolean;
}

interface EventListenerOptions {
  capture?: boolean;
}

/** Decodes a string of data which has been encoded using base-64 encoding.
 *
 *     console.log(atob("aGVsbG8gd29ybGQ=")); // outputs 'hello world'
 */
declare function atob(s: string): string;

/** Creates a base-64 ASCII encoded string from the input string.
 *
 *     console.log(btoa("hello world"));  // outputs "aGVsbG8gd29ybGQ="
 */
declare function btoa(s: string): string;

declare class TextDecoder {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM = false;
  constructor(
    label?: string,
    options?: { fatal?: boolean; ignoreBOM?: boolean },
  );
  /** Returns the result of running encoding's decoder. */
  decode(input?: BufferSource, options?: { stream?: false }): string;
  readonly [Symbol.toStringTag]: string;
}

declare class TextEncoder {
  /** Returns "utf-8". */
  readonly encoding = "utf-8";
  /** Returns the result of running UTF-8's encoder. */
  encode(input?: string): Uint8Array;
  encodeInto(
    input: string,
    dest: Uint8Array,
  ): { read: number; written: number };
  readonly [Symbol.toStringTag]: string;
}
