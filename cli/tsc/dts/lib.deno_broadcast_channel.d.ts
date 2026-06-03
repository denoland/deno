// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * @category Messaging
 */
interface BroadcastChannelEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** Represents a named channel that any
 * {@linkcode BroadcastChannel} with the same name (across workers or isolates
 * in the same Deno process) can use to send and receive messages, allowing
 * one-to-many communication between execution contexts.
 *
 * @category Messaging
 */
interface BroadcastChannel extends EventTarget {
  /**
   * Returns the channel name (as passed to the constructor).
   */
  readonly name: string;
  onmessage: ((this: BroadcastChannel, ev: MessageEvent) => any) | null;
  onmessageerror: ((this: BroadcastChannel, ev: MessageEvent) => any) | null;
  /**
   * Closes the BroadcastChannel object, opening it up to garbage collection.
   */
  close(): void;
  /**
   * Sends the given message to other BroadcastChannel objects set up for
   * this channel. Messages can be structured objects, e.g. nested objects
   * and arrays.
   */
  postMessage(message: any): void;
  addEventListener<K extends keyof BroadcastChannelEventMap>(
    type: K,
    listener: (this: BroadcastChannel, ev: BroadcastChannelEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof BroadcastChannelEventMap>(
    type: K,
    listener: (this: BroadcastChannel, ev: BroadcastChannelEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** The constructor object for {@linkcode BroadcastChannel}.
 *
 * Construct a channel with `new BroadcastChannel(name)` to join the channel
 * identified by `name`; messages posted on it are delivered to every other
 * `BroadcastChannel` connected to the same name.
 *
 * @category Messaging
 */
declare var BroadcastChannel: {
  readonly prototype: BroadcastChannel;
  new (name: string): BroadcastChannel;
};
