// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category DOM Events */
declare class DOMException extends Error {
  constructor(message?: string, name?: string);
  readonly name: string;
  readonly message: string;
  readonly code: number;
}

/** @category DOM Events */
interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

/** An event which takes place in the DOM.
 *
 * @category DOM Events
 */
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
 *
 * @category DOM Events
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
}

/** @category DOM Events */
interface EventListener {
  (evt: Event): void | Promise<void>;
}

/** @category DOM Events */
interface EventListenerObject {
  handleEvent(evt: Event): void | Promise<void>;
}

/** @category DOM Events */
declare type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

/** @category DOM Events */
interface AddEventListenerOptions extends EventListenerOptions {
  once?: boolean;
  passive?: boolean;
  signal?: AbortSignal;
}

/** @category DOM Events */
interface EventListenerOptions {
  capture?: boolean;
}

/** @category DOM Events */
interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>).
 *
 * @category DOM Events
 */
declare class ProgressEvent<T extends EventTarget = EventTarget> extends Event {
  constructor(type: string, eventInitDict?: ProgressEventInit);
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly target: T | null;
  readonly total: number;
}

/** Decodes a string of data which has been encoded using base-64 encoding.
 *
 * ```
 * console.log(atob("aGVsbG8gd29ybGQ=")); // outputs 'hello world'
 * ```
 *
 * @category Encoding API
 */
declare function atob(s: string): string;

/** Creates a base-64 ASCII encoded string from the input string.
 *
 * ```
 * console.log(btoa("hello world"));  // outputs "aGVsbG8gd29ybGQ="
 * ```
 *
 * @category Encoding API
 */
declare function btoa(s: string): string;

/** @category Encoding API */
declare interface TextDecoderOptions {
  fatal?: boolean;
  ignoreBOM?: boolean;
}

/** @category Encoding API */
declare interface TextDecodeOptions {
  stream?: boolean;
}

/** @category Encoding API */
interface TextDecoder {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM: boolean;

  /** Returns the result of running encoding's decoder. */
  decode(input?: BufferSource, options?: TextDecodeOptions): string;
}

/** @category Encoding API */
declare var TextDecoder: {
  prototype: TextDecoder;
  new (label?: string, options?: TextDecoderOptions): TextDecoder;
};

/** @category Encoding API */
declare interface TextEncoderEncodeIntoResult {
  read: number;
  written: number;
}

/** @category Encoding API */
interface TextEncoder {
  /** Returns "utf-8". */
  readonly encoding: "utf-8";
  /** Returns the result of running UTF-8's encoder. */
  encode(input?: string): Uint8Array;
  encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
}

/** @category Encoding API */
declare var TextEncoder: {
  prototype: TextEncoder;
  new (): TextEncoder;
};

/** @category Encoding API */
interface TextDecoderStream {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM: boolean;
  readonly readable: ReadableStream<string>;
  readonly writable: WritableStream<BufferSource>;
  readonly [Symbol.toStringTag]: string;
}

/** @category Encoding API */
declare var TextDecoderStream: {
  prototype: TextDecoderStream;
  new (label?: string, options?: TextDecoderOptions): TextDecoderStream;
};

/** @category Encoding API */
interface TextEncoderStream {
  /** Returns "utf-8". */
  readonly encoding: "utf-8";
  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<string>;
  readonly [Symbol.toStringTag]: string;
}

/** @category Encoding API */
declare var TextEncoderStream: {
  prototype: TextEncoderStream;
  new (): TextEncoderStream;
};

/** A controller object that allows you to abort one or more DOM requests as and
 * when desired.
 *
 * @category Web APIs
 */
declare class AbortController {
  /** Returns the AbortSignal object associated with this object. */
  readonly signal: AbortSignal;
  /** Invoking this method will set this object's AbortSignal's aborted flag and
   * signal to any observers that the associated activity is to be aborted. */
  abort(reason?: any): void;
}

/** @category Web APIs */
interface AbortSignalEventMap {
  abort: Event;
}

/** A signal object that allows you to communicate with a DOM request (such as a
 * Fetch) and abort it if required via an AbortController object.
 *
 * @category Web APIs
 */
interface AbortSignal extends EventTarget {
  /** Returns true if this AbortSignal's AbortController has signaled to abort,
   * and false otherwise. */
  readonly aborted: boolean;
  readonly reason: any;
  onabort: ((this: AbortSignal, ev: Event) => any) | null;
  addEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;

  /** Throws this AbortSignal's abort reason, if its AbortController has
   * signaled to abort; otherwise, does nothing. */
  throwIfAborted(): void;
}

/** @category Web APIs */
declare var AbortSignal: {
  prototype: AbortSignal;
  new (): AbortSignal;
  abort(reason?: any): AbortSignal;
  timeout(milliseconds: number): AbortSignal;
};

/** @category Web File API */
interface FileReaderEventMap {
  "abort": ProgressEvent<FileReader>;
  "error": ProgressEvent<FileReader>;
  "load": ProgressEvent<FileReader>;
  "loadend": ProgressEvent<FileReader>;
  "loadstart": ProgressEvent<FileReader>;
  "progress": ProgressEvent<FileReader>;
}

/** Lets web applications asynchronously read the contents of files (or raw data
 * buffers) stored on the user's computer, using File or Blob objects to specify
 * the file or data to read.
 *
 * @category Web File API
 */
interface FileReader extends EventTarget {
  readonly error: DOMException | null;
  onabort: ((this: FileReader, ev: ProgressEvent<FileReader>) => any) | null;
  onerror: ((this: FileReader, ev: ProgressEvent<FileReader>) => any) | null;
  onload: ((this: FileReader, ev: ProgressEvent<FileReader>) => any) | null;
  onloadend: ((this: FileReader, ev: ProgressEvent<FileReader>) => any) | null;
  onloadstart:
    | ((this: FileReader, ev: ProgressEvent<FileReader>) => any)
    | null;
  onprogress: ((this: FileReader, ev: ProgressEvent<FileReader>) => any) | null;
  readonly readyState: number;
  readonly result: string | ArrayBuffer | null;
  abort(): void;
  readAsArrayBuffer(blob: Blob): void;
  readAsBinaryString(blob: Blob): void;
  readAsDataURL(blob: Blob): void;
  readAsText(blob: Blob, encoding?: string): void;
  readonly DONE: number;
  readonly EMPTY: number;
  readonly LOADING: number;
  addEventListener<K extends keyof FileReaderEventMap>(
    type: K,
    listener: (this: FileReader, ev: FileReaderEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof FileReaderEventMap>(
    type: K,
    listener: (this: FileReader, ev: FileReaderEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** @category Web File API */
declare var FileReader: {
  prototype: FileReader;
  new (): FileReader;
  readonly DONE: number;
  readonly EMPTY: number;
  readonly LOADING: number;
};

/** @category Web File API */
type BlobPart = BufferSource | Blob | string;

/** @category Web File API */
interface BlobPropertyBag {
  type?: string;
  endings?: "transparent" | "native";
}

/** A file-like object of immutable, raw data. Blobs represent data that isn't
 * necessarily in a JavaScript-native format. The File interface is based on
 * Blob, inheriting blob functionality and expanding it to support files on the
 * user's system.
 *
 * @category Web File API
 */
declare class Blob {
  constructor(blobParts?: BlobPart[], options?: BlobPropertyBag);

  readonly size: number;
  readonly type: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start?: number, end?: number, contentType?: string): Blob;
  stream(): ReadableStream<Uint8Array>;
  text(): Promise<string>;
}

/** @category Web File API */
interface FilePropertyBag extends BlobPropertyBag {
  lastModified?: number;
}

/** Provides information about files and allows JavaScript in a web page to
 * access their content.
 *
 * @category Web File API
 */
declare class File extends Blob {
  constructor(
    fileBits: BlobPart[],
    fileName: string,
    options?: FilePropertyBag,
  );

  readonly lastModified: number;
  readonly name: string;
}

/** @category Streams API */
interface ReadableStreamDefaultReadDoneResult {
  done: true;
  value?: undefined;
}

/** @category Streams API */
interface ReadableStreamDefaultReadValueResult<T> {
  done: false;
  value: T;
}

/** @category Streams API */
type ReadableStreamDefaultReadResult<T> =
  | ReadableStreamDefaultReadValueResult<T>
  | ReadableStreamDefaultReadDoneResult;

/** @category Streams API */
interface ReadableStreamDefaultReader<R = any> {
  readonly closed: Promise<void>;
  cancel(reason?: any): Promise<void>;
  read(): Promise<ReadableStreamDefaultReadResult<R>>;
  releaseLock(): void;
}

/** @category Streams API */
declare var ReadableStreamDefaultReader: {
  prototype: ReadableStreamDefaultReader;
  new <R>(stream: ReadableStream<R>): ReadableStreamDefaultReader<R>;
};

/** @category Streams API */
interface ReadableStreamBYOBReadDoneResult<V extends ArrayBufferView> {
  done: true;
  value?: V;
}

/** @category Streams API */
interface ReadableStreamBYOBReadValueResult<V extends ArrayBufferView> {
  done: false;
  value: V;
}

/** @category Streams API */
type ReadableStreamBYOBReadResult<V extends ArrayBufferView> =
  | ReadableStreamBYOBReadDoneResult<V>
  | ReadableStreamBYOBReadValueResult<V>;

/** @category Streams API */
interface ReadableStreamBYOBReader {
  readonly closed: Promise<void>;
  cancel(reason?: any): Promise<void>;
  read<V extends ArrayBufferView>(
    view: V,
  ): Promise<ReadableStreamBYOBReadResult<V>>;
  releaseLock(): void;
}

/** @category Streams API */
declare var ReadableStreamBYOBReader: {
  prototype: ReadableStreamBYOBReader;
  new (stream: ReadableStream<Uint8Array>): ReadableStreamBYOBReader;
};

/** @category Streams API */
interface ReadableStreamBYOBRequest {
  readonly view: ArrayBufferView | null;
  respond(bytesWritten: number): void;
  respondWithNewView(view: ArrayBufferView): void;
}

/** @category Streams API */
interface ReadableByteStreamControllerCallback {
  (controller: ReadableByteStreamController): void | PromiseLike<void>;
}

/** @category Streams API */
interface UnderlyingByteSource {
  autoAllocateChunkSize?: number;
  cancel?: ReadableStreamErrorCallback;
  pull?: ReadableByteStreamControllerCallback;
  start?: ReadableByteStreamControllerCallback;
  type: "bytes";
}

/** @category Streams API */
interface UnderlyingSink<W = any> {
  abort?: WritableStreamErrorCallback;
  close?: WritableStreamDefaultControllerCloseCallback;
  start?: WritableStreamDefaultControllerStartCallback;
  type?: undefined;
  write?: WritableStreamDefaultControllerWriteCallback<W>;
}

/** @category Streams API */
interface UnderlyingSource<R = any> {
  cancel?: ReadableStreamErrorCallback;
  pull?: ReadableStreamDefaultControllerCallback<R>;
  start?: ReadableStreamDefaultControllerCallback<R>;
  type?: undefined;
}

/** @category Streams API */
interface ReadableStreamErrorCallback {
  (reason: any): void | PromiseLike<void>;
}

/** @category Streams API */
interface ReadableStreamDefaultControllerCallback<R> {
  (controller: ReadableStreamDefaultController<R>): void | PromiseLike<void>;
}

/** @category Streams API */
interface ReadableStreamDefaultController<R = any> {
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk: R): void;
  error(error?: any): void;
}

/** @category Streams API */
declare var ReadableStreamDefaultController: {
  prototype: ReadableStreamDefaultController;
  new (): ReadableStreamDefaultController;
};

/** @category Streams API */
interface ReadableByteStreamController {
  readonly byobRequest: ReadableStreamBYOBRequest | null;
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk: ArrayBufferView): void;
  error(error?: any): void;
}

/** @category Streams API */
declare var ReadableByteStreamController: {
  prototype: ReadableByteStreamController;
  new (): ReadableByteStreamController;
};

/** @category Streams API */
interface PipeOptions {
  preventAbort?: boolean;
  preventCancel?: boolean;
  preventClose?: boolean;
  signal?: AbortSignal;
}

/** @category Streams API */
interface QueuingStrategySizeCallback<T = any> {
  (chunk: T): number;
}

/** @category Streams API */
interface QueuingStrategy<T = any> {
  highWaterMark?: number;
  size?: QueuingStrategySizeCallback<T>;
}

/** This Streams API interface provides a built-in byte length queuing strategy
 * that can be used when constructing streams.
 *
 * @category Streams API
 */
interface CountQueuingStrategy extends QueuingStrategy {
  highWaterMark: number;
  size(chunk: any): 1;
}

/** @category Streams API */
declare var CountQueuingStrategy: {
  prototype: CountQueuingStrategy;
  new (options: { highWaterMark: number }): CountQueuingStrategy;
};

/** @category Streams API */
interface ByteLengthQueuingStrategy extends QueuingStrategy<ArrayBufferView> {
  highWaterMark: number;
  size(chunk: ArrayBufferView): number;
}

/** @category Streams API */
declare var ByteLengthQueuingStrategy: {
  prototype: ByteLengthQueuingStrategy;
  new (options: { highWaterMark: number }): ByteLengthQueuingStrategy;
};

/** This Streams API interface represents a readable stream of byte data. The
 * Fetch API offers a concrete instance of a ReadableStream through the body
 * property of a Response object.
 *
 * @category Streams API
 */
interface ReadableStream<R = any> {
  readonly locked: boolean;
  cancel(reason?: any): Promise<void>;
  getReader(options: { mode: "byob" }): ReadableStreamBYOBReader;
  getReader(options?: { mode?: undefined }): ReadableStreamDefaultReader<R>;
  pipeThrough<T>(transform: {
    writable: WritableStream<R>;
    readable: ReadableStream<T>;
  }, options?: PipeOptions): ReadableStream<T>;
  pipeTo(dest: WritableStream<R>, options?: PipeOptions): Promise<void>;
  tee(): [ReadableStream<R>, ReadableStream<R>];
  [Symbol.asyncIterator](options?: {
    preventCancel?: boolean;
  }): AsyncIterableIterator<R>;
}

/** @category Streams API */
declare var ReadableStream: {
  prototype: ReadableStream;
  new (
    underlyingSource: UnderlyingByteSource,
    strategy?: { highWaterMark?: number; size?: undefined },
  ): ReadableStream<Uint8Array>;
  new <R = any>(
    underlyingSource?: UnderlyingSource<R>,
    strategy?: QueuingStrategy<R>,
  ): ReadableStream<R>;
};

/** @category Streams API */
interface WritableStreamDefaultControllerCloseCallback {
  (): void | PromiseLike<void>;
}

/** @category Streams API */
interface WritableStreamDefaultControllerStartCallback {
  (controller: WritableStreamDefaultController): void | PromiseLike<void>;
}

/** @category Streams API */
interface WritableStreamDefaultControllerWriteCallback<W> {
  (chunk: W, controller: WritableStreamDefaultController):
    | void
    | PromiseLike<
      void
    >;
}

/** @category Streams API */
interface WritableStreamErrorCallback {
  (reason: any): void | PromiseLike<void>;
}

/** This Streams API interface provides a standard abstraction for writing
 * streaming data to a destination, known as a sink. This object comes with
 * built-in backpressure and queuing.
 *
 * @category Streams API
 */
interface WritableStream<W = any> {
  readonly locked: boolean;
  abort(reason?: any): Promise<void>;
  close(): Promise<void>;
  getWriter(): WritableStreamDefaultWriter<W>;
}

/** @category Streams API */
declare var WritableStream: {
  prototype: WritableStream;
  new <W = any>(
    underlyingSink?: UnderlyingSink<W>,
    strategy?: QueuingStrategy<W>,
  ): WritableStream<W>;
};

/** This Streams API interface represents a controller allowing control of a
 * WritableStream's state. When constructing a WritableStream, the underlying
 * sink is given a corresponding WritableStreamDefaultController instance to
 * manipulate.
 *
 * @category Streams API
 */
interface WritableStreamDefaultController {
  signal: AbortSignal;
  error(error?: any): void;
}

/** @category Streams API */
declare var WritableStreamDefaultController: WritableStreamDefaultController;

/** This Streams API interface is the object returned by
 * WritableStream.getWriter() and once created locks the < writer to the
 * WritableStream ensuring that no other streams can write to the underlying
 * sink.
 *
 * @category Streams API
 */
interface WritableStreamDefaultWriter<W = any> {
  readonly closed: Promise<void>;
  readonly desiredSize: number | null;
  readonly ready: Promise<void>;
  abort(reason?: any): Promise<void>;
  close(): Promise<void>;
  releaseLock(): void;
  write(chunk: W): Promise<void>;
}

/** @category Streams API */
declare var WritableStreamDefaultWriter: {
  prototype: WritableStreamDefaultWriter;
  new (): WritableStreamDefaultWriter;
};

/** @category Streams API */
interface TransformStream<I = any, O = any> {
  readonly readable: ReadableStream<O>;
  readonly writable: WritableStream<I>;
}

/** @category Streams API */
declare var TransformStream: {
  prototype: TransformStream;
  new <I = any, O = any>(
    transformer?: Transformer<I, O>,
    writableStrategy?: QueuingStrategy<I>,
    readableStrategy?: QueuingStrategy<O>,
  ): TransformStream<I, O>;
};

/** @category Streams API */
interface TransformStreamDefaultController<O = any> {
  readonly desiredSize: number | null;
  enqueue(chunk: O): void;
  error(reason?: any): void;
  terminate(): void;
}

/** @category Streams API */
declare var TransformStreamDefaultController: TransformStreamDefaultController;

/** @category Streams API */
interface Transformer<I = any, O = any> {
  flush?: TransformStreamDefaultControllerCallback<O>;
  readableType?: undefined;
  start?: TransformStreamDefaultControllerCallback<O>;
  transform?: TransformStreamDefaultControllerTransformCallback<I, O>;
  writableType?: undefined;
}

/** @category Streams API */
interface TransformStreamDefaultControllerCallback<O> {
  (controller: TransformStreamDefaultController<O>): void | PromiseLike<void>;
}

/** @category Streams API */
interface TransformStreamDefaultControllerTransformCallback<I, O> {
  (
    chunk: I,
    controller: TransformStreamDefaultController<O>,
  ): void | PromiseLike<void>;
}

/** @category Streams API */
interface MessageEventInit<T = any> extends EventInit {
  data?: T;
  origin?: string;
  lastEventId?: string;
}

/** @category Streams API */
declare class MessageEvent<T = any> extends Event {
  /**
   * Returns the data of the message.
   */
  readonly data: T;
  /**
   * Returns the last event ID string, for server-sent events.
   */
  readonly lastEventId: string;
  /**
   * Returns transferred ports.
   */
  readonly ports: ReadonlyArray<MessagePort>;
  constructor(type: string, eventInitDict?: MessageEventInit);
}

/** @category DOM APIs */
type Transferable = ArrayBuffer | MessagePort;

/**
 * This type has been renamed to StructuredSerializeOptions. Use that type for
 * new code.
 *
 * @deprecated use `StructuredSerializeOptions` instead.
 * @category DOM APIs
 */
type PostMessageOptions = StructuredSerializeOptions;

/** @category DOM APIs */
interface StructuredSerializeOptions {
  transfer?: Transferable[];
}

/** The MessageChannel interface of the Channel Messaging API allows us to
 * create a new message channel and send data through it via its two MessagePort
 * properties.
 *
 * @category DOM APIs
 */
declare class MessageChannel {
  constructor();
  readonly port1: MessagePort;
  readonly port2: MessagePort;
}

/** @category DOM APIs */
interface MessagePortEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** The MessagePort interface of the Channel Messaging API represents one of the
 * two ports of a MessageChannel, allowing messages to be sent from one port and
 * listening out for them arriving at the other.
 *
 * @category DOM APIs
 */
declare class MessagePort extends EventTarget {
  onmessage: ((this: MessagePort, ev: MessageEvent) => any) | null;
  onmessageerror: ((this: MessagePort, ev: MessageEvent) => any) | null;
  /**
   * Disconnects the port, so that it is no longer active.
   */
  close(): void;
  /**
   * Posts a message through the channel. Objects listed in transfer are
   * transferred, not just cloned, meaning that they are no longer usable on the
   * sending side.
   *
   * Throws a "DataCloneError" DOMException if transfer contains duplicate
   * objects or port, or if message could not be cloned.
   */
  postMessage(message: any, transfer: Transferable[]): void;
  postMessage(message: any, options?: StructuredSerializeOptions): void;
  /**
   * Begins dispatching messages received on the port. This is implicitly called
   * when assigning a value to `this.onmessage`.
   */
  start(): void;
  addEventListener<K extends keyof MessagePortEventMap>(
    type: K,
    listener: (this: MessagePort, ev: MessagePortEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof MessagePortEventMap>(
    type: K,
    listener: (this: MessagePort, ev: MessagePortEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/**
 * Creates a deep copy of a given value using the structured clone algorithm.
 *
 * Unlike a shallow copy, a deep copy does not hold the same references as the
 * source object, meaning its properties can be changed without affecting the
 * source. For more details, see
 * [MDN](https://developer.mozilla.org/en-US/docs/Glossary/Deep_copy).
 *
 * Throws a `DataCloneError` if any part of the input value is not
 * serializable.
 *
 * @example
 * ```ts
 * const object = { x: 0, y: 1 };
 *
 * const deepCopy = structuredClone(object);
 * deepCopy.x = 1;
 * console.log(deepCopy.x, object.x); // 1 0
 *
 * const shallowCopy = object;
 * shallowCopy.x = 1;
 * // shallowCopy.x is pointing to the same location in memory as object.x
 * console.log(shallowCopy.x, object.x); // 1 1
 * ```
 *
 * @category DOM APIs
 */
declare function structuredClone(
  value: any,
  options?: StructuredSerializeOptions,
): any;

/**
 * An API for compressing a stream of data.
 *
 * @example
 * ```ts
 * await Deno.stdin.readable
 *   .pipeThrough(new CompressionStream("gzip"))
 *   .pipeTo(Deno.stdout.writable);
 * ```
 *
 * @category Compression Streams API
 */
declare class CompressionStream {
  /**
   * Creates a new `CompressionStream` object which compresses a stream of
   * data.
   *
   * Throws a `TypeError` if the format passed to the constructor is not
   * supported.
   */
  constructor(format: string);

  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<Uint8Array>;
}

/**
 * An API for decompressing a stream of data.
 *
 * @example
 * ```ts
 * const input = await Deno.open("./file.txt.gz");
 * const output = await Deno.create("./file.txt");
 *
 * await input.readable
 *   .pipeThrough(new DecompressionStream("gzip"))
 *   .pipeTo(output.writable);
 * ```
 *
 * @category Compression Streams API
 */
declare class DecompressionStream {
  /**
   * Creates a new `DecompressionStream` object which decompresses a stream of
   * data.
   *
   * Throws a `TypeError` if the format passed to the constructor is not
   * supported.
   */
  constructor(format: string);

  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<Uint8Array>;
}

/** Dispatch an uncaught exception. Similar to a synchronous version of:
 * ```ts
 * setTimeout(() => { throw error; }, 0);
 * ```
 * The error can not be caught with a `try/catch` block. An error event will
 * be dispatched to the global scope. You can prevent the error from being
 * reported to the console with `Event.prototype.preventDefault()`:
 * ```ts
 * addEventListener("error", (event) => {
 *   event.preventDefault();
 * });
 * reportError(new Error("foo")); // Will not be reported.
 * ```
 * In Deno, this error will terminate the process if not intercepted like above.
 *
 * @category Web APIs
 */
declare function reportError(
  error: any,
): void;
