// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Platform */
interface DOMException extends Error {
  readonly name: string;
  readonly message: string;
  /** @deprecated */
  readonly code: number;
  readonly INDEX_SIZE_ERR: 1;
  readonly DOMSTRING_SIZE_ERR: 2;
  readonly HIERARCHY_REQUEST_ERR: 3;
  readonly WRONG_DOCUMENT_ERR: 4;
  readonly INVALID_CHARACTER_ERR: 5;
  readonly NO_DATA_ALLOWED_ERR: 6;
  readonly NO_MODIFICATION_ALLOWED_ERR: 7;
  readonly NOT_FOUND_ERR: 8;
  readonly NOT_SUPPORTED_ERR: 9;
  readonly INUSE_ATTRIBUTE_ERR: 10;
  readonly INVALID_STATE_ERR: 11;
  readonly SYNTAX_ERR: 12;
  readonly INVALID_MODIFICATION_ERR: 13;
  readonly NAMESPACE_ERR: 14;
  readonly INVALID_ACCESS_ERR: 15;
  readonly VALIDATION_ERR: 16;
  readonly TYPE_MISMATCH_ERR: 17;
  readonly SECURITY_ERR: 18;
  readonly NETWORK_ERR: 19;
  readonly ABORT_ERR: 20;
  readonly URL_MISMATCH_ERR: 21;
  readonly QUOTA_EXCEEDED_ERR: 22;
  readonly TIMEOUT_ERR: 23;
  readonly INVALID_NODE_TYPE_ERR: 24;
  readonly DATA_CLONE_ERR: 25;
}

/** @category Platform */
declare var DOMException: {
  readonly prototype: DOMException;
  new (message?: string, name?: string): DOMException;
  readonly INDEX_SIZE_ERR: 1;
  readonly DOMSTRING_SIZE_ERR: 2;
  readonly HIERARCHY_REQUEST_ERR: 3;
  readonly WRONG_DOCUMENT_ERR: 4;
  readonly INVALID_CHARACTER_ERR: 5;
  readonly NO_DATA_ALLOWED_ERR: 6;
  readonly NO_MODIFICATION_ALLOWED_ERR: 7;
  readonly NOT_FOUND_ERR: 8;
  readonly NOT_SUPPORTED_ERR: 9;
  readonly INUSE_ATTRIBUTE_ERR: 10;
  readonly INVALID_STATE_ERR: 11;
  readonly SYNTAX_ERR: 12;
  readonly INVALID_MODIFICATION_ERR: 13;
  readonly NAMESPACE_ERR: 14;
  readonly INVALID_ACCESS_ERR: 15;
  readonly VALIDATION_ERR: 16;
  readonly TYPE_MISMATCH_ERR: 17;
  readonly SECURITY_ERR: 18;
  readonly NETWORK_ERR: 19;
  readonly ABORT_ERR: 20;
  readonly URL_MISMATCH_ERR: 21;
  readonly QUOTA_EXCEEDED_ERR: 22;
  readonly TIMEOUT_ERR: 23;
  readonly INVALID_NODE_TYPE_ERR: 24;
  readonly DATA_CLONE_ERR: 25;
};

/** @category Events */
interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

/** An event which takes place in the DOM.
 *
 * @category Events
 */
interface Event {
  /** Returns true or false depending on how event was initialized. True if
   * event goes through its target's ancestors in reverse tree order, and
   * false otherwise. */
  readonly bubbles: boolean;
  /** @deprecated */
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
  /** @deprecated */
  returnValue: boolean;
  /** @deprecated */
  readonly srcElement: EventTarget | null;
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
  /** @deprecated */
  initEvent(type: string, bubbles?: boolean, cancelable?: boolean): void;
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
  readonly NONE: 0;
  readonly CAPTURING_PHASE: 1;
  readonly AT_TARGET: 2;
  readonly BUBBLING_PHASE: 3;
}

/** An event which takes place in the DOM.
 *
 * @category Events
 */
declare var Event: {
  readonly prototype: Event;
  new (type: string, eventInitDict?: EventInit): Event;
  readonly NONE: 0;
  readonly CAPTURING_PHASE: 1;
  readonly AT_TARGET: 2;
  readonly BUBBLING_PHASE: 3;
};

/**
 * EventTarget is a DOM interface implemented by objects that can receive events
 * and may have listeners for them.
 *
 * @category Events
 */
interface EventTarget {
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
  /** Dispatches a synthetic event to event target and returns true if either
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

/**
 * EventTarget is a DOM interface implemented by objects that can receive events
 * and may have listeners for them.
 *
 * @category Events
 */
declare var EventTarget: {
  readonly prototype: EventTarget;
  new (): EventTarget;
};

/** @category Events */
interface EventListener {
  (evt: Event): void;
}

/** @category Events */
interface EventListenerObject {
  handleEvent(evt: Event): void;
}

/** @category Events */
type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

/** @category Events */
interface AddEventListenerOptions extends EventListenerOptions {
  once?: boolean;
  passive?: boolean;
  signal?: AbortSignal;
}

/** @category Events */
interface EventListenerOptions {
  capture?: boolean;
}

/** @category Events */
interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>).
 *
 * @category Events
 */
interface ProgressEvent<T extends EventTarget = EventTarget> extends Event {
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly target: T | null;
  readonly total: number;
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>).
 *
 * @category Events
 */
declare var ProgressEvent: {
  readonly prototype: ProgressEvent;
  new (type: string, eventInitDict?: ProgressEventInit): ProgressEvent;
};

/** Decodes a string of data which has been encoded using base-64 encoding.
 *
 * ```
 * console.log(atob("aGVsbG8gd29ybGQ=")); // outputs 'hello world'
 * ```
 *
 * @category Encoding
 */
declare function atob(s: string): string;

/** Creates a base-64 ASCII encoded string from the input string.
 *
 * ```
 * console.log(btoa("hello world"));  // outputs "aGVsbG8gd29ybGQ="
 * ```
 *
 * @category Encoding
 */
declare function btoa(s: string): string;

/** @category Encoding */
interface TextDecoderOptions {
  fatal?: boolean;
  ignoreBOM?: boolean;
}

/** @category Encoding */
interface TextDecodeOptions {
  stream?: boolean;
}

/**
 * Represents a decoder for a specific text encoding, allowing you to convert
 * binary data into a string given the encoding.
 *
 * @example
 * ```ts
 * const decoder = new TextDecoder('utf-8');
 * const buffer = new Uint8Array([72, 101, 108, 108, 111]);
 * const decodedString = decoder.decode(buffer);
 * console.log(decodedString); // Outputs: "Hello"
 * ```
 *
 * @category Encoding
 */
interface TextDecoder extends TextDecoderCommon {
  /** Turns binary data, often in the form of a Uint8Array, into a string given
   * the encoding.
   */
  decode(input?: BufferSource, options?: TextDecodeOptions): string;
}

/** @category Encoding */
declare var TextDecoder: {
  readonly prototype: TextDecoder;
  new (label?: string, options?: TextDecoderOptions): TextDecoder;
};

/** @category Encoding */
interface TextDecoderCommon {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns true if error mode is "fatal", otherwise false. */
  readonly fatal: boolean;
  /** Returns the value of ignore BOM. */
  readonly ignoreBOM: boolean;
}

/** @category Encoding */
interface TextEncoderEncodeIntoResult {
  read: number;
  written: number;
}

/** @category Encoding */
interface TextEncoder extends TextEncoderCommon {
  /** Returns the result of running UTF-8's encoder. */
  encode(input?: string): Uint8Array;
  encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
}
/**
 * Allows you to convert a string into binary data (in the form of a Uint8Array)
 * given the encoding.
 *
 * @example
 * ```ts
 * const encoder = new TextEncoder();
 * const str = "Hello";
 * const encodedData = encoder.encode(str);
 * console.log(encodedData); // Outputs: Uint8Array(5) [72, 101, 108, 108, 111]
 * ```
 *
 * @category Encoding
 */
interface TextEncoder extends TextEncoderCommon {
  /** Turns a string into binary data (in the form of a Uint8Array) using UTF-8 encoding. */
  encode(input?: string): Uint8Array;

  /** Encodes a string into the destination Uint8Array and returns the result of the encoding. */
  encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
}

/** @category Encoding */
declare var TextEncoder: {
  readonly prototype: TextEncoder;
  new (): TextEncoder;
};

/** @category Encoding */
interface TextEncoderCommon {
  /** Returns "utf-8". */
  readonly encoding: string;
}

/** @category Encoding */
interface TextDecoderStream extends GenericTransformStream, TextDecoderCommon {
  readonly readable: ReadableStream<string>;
  readonly writable: WritableStream<BufferSource>;
}

/** @category Encoding */
declare var TextDecoderStream: {
  readonly prototype: TextDecoderStream;
  new (label?: string, options?: TextDecoderOptions): TextDecoderStream;
};

/** @category Encoding */
interface TextEncoderStream extends GenericTransformStream, TextEncoderCommon {
  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<string>;
}

/** @category Encoding */
declare var TextEncoderStream: {
  readonly prototype: TextEncoderStream;
  new (): TextEncoderStream;
};

/** A controller object that allows you to abort one or more DOM requests as and
 * when desired.
 *
 * @category Platform
 */
interface AbortController {
  /** Returns the AbortSignal object associated with this object. */
  readonly signal: AbortSignal;
  /** Invoking this method will set this object's AbortSignal's aborted flag and
   * signal to any observers that the associated activity is to be aborted. */
  abort(reason?: any): void;
}

/** A controller object that allows you to abort one or more DOM requests as and
 * when desired.
 *
 * @category Platform
 */
declare var AbortController: {
  readonly prototype: AbortController;
  new (): AbortController;
};

/** @category Platform */
interface AbortSignalEventMap {
  abort: Event;
}

/** A signal object that allows you to communicate with a DOM request (such as a
 * Fetch) and abort it if required via an AbortController object.
 *
 * @category Platform
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

/** @category Platform */
declare var AbortSignal: {
  readonly prototype: AbortSignal;
  new (): never;
  abort(reason?: any): AbortSignal;
  any(signals: AbortSignal[]): AbortSignal;
  timeout(milliseconds: number): AbortSignal;
};

/** @category File */
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
 * @category File
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
  readonly readyState:
    | typeof FileReader.EMPTY
    | typeof FileReader.LOADING
    | typeof FileReader.DONE;
  readonly result: string | ArrayBuffer | null;
  abort(): void;
  readAsArrayBuffer(blob: Blob): void;
  /** @deprecated */
  readAsBinaryString(blob: Blob): void;
  readAsDataURL(blob: Blob): void;
  readAsText(blob: Blob, encoding?: string): void;
  readonly EMPTY: 0;
  readonly LOADING: 1;
  readonly DONE: 2;
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

/** @category File */
declare var FileReader: {
  readonly prototype: FileReader;
  new (): FileReader;
  readonly EMPTY: 0;
  readonly LOADING: 1;
  readonly DONE: 2;
};

/** @category File */
type BlobPart = BufferSource | Blob | string;

/** @category File */
type EndingType = "transparent" | "native";

/** @category File */
interface BlobPropertyBag {
  type?: string;
  endings?: EndingType;
}

/** A file-like object of immutable, raw data. Blobs represent data that isn't
 * necessarily in a JavaScript-native format. The File interface is based on
 * Blob, inheriting blob functionality and expanding it to support files on the
 * user's system.
 *
 * @category File
 */
interface Blob {
  readonly size: number;
  readonly type: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  bytes(): Promise<Uint8Array>;
  slice(start?: number, end?: number, contentType?: string): Blob;
  stream(): ReadableStream<Uint8Array>;
  text(): Promise<string>;
}

/** A file-like object of immutable, raw data. Blobs represent data that isn't
 * necessarily in a JavaScript-native format. The File interface is based on
 * Blob, inheriting blob functionality and expanding it to support files on the
 * user's system.
 *
 * @category File
 */
declare var Blob: {
  readonly prototype: Blob;
  new (blobParts?: BlobPart[], options?: BlobPropertyBag): Blob;
};

/** @category File */
interface FilePropertyBag extends BlobPropertyBag {
  lastModified?: number;
}

/** Provides information about files and allows JavaScript in a web page to
 * access their content.
 *
 * @category File
 */
interface File extends Blob {
  readonly lastModified: number;
  readonly name: string;
  readonly webkitRelativePath: string;
}

/** Provides information about files and allows JavaScript in a web page to
 * access their content.
 *
 * @category File
 */
declare var File: {
  readonly prototype: File;
  new (fileBits: BlobPart[], fileName: string, options?: FilePropertyBag): File;
};

/** @category Streams */
type ReadableStreamReader<T> =
  | ReadableStreamDefaultReader<T>
  | ReadableStreamBYOBReader;

/** @category Streams */
type ReadableStreamController<T> =
  | ReadableStreamDefaultController<T>
  | ReadableByteStreamController;

/** @category Streams */
interface ReadableStreamGenericReader {
  readonly closed: Promise<undefined>;
  cancel(reason?: any): Promise<void>;
}

/** @category Streams */
interface ReadableStreamReadDoneResult<T> {
  done: true;
  value?: T;
}

/** @category Streams */
interface ReadableStreamReadValueResult<T> {
  done: false;
  value: T;
}

/** @category Streams */
type ReadableStreamReadResult<T> =
  | ReadableStreamReadValueResult<T>
  | ReadableStreamReadDoneResult<T>;

/** @category Streams */
interface ReadableStreamDefaultReader<R = any>
  extends ReadableStreamGenericReader {
  read(): Promise<ReadableStreamReadResult<R>>;
  releaseLock(): void;
}

/** @category Streams */
declare var ReadableStreamDefaultReader: {
  readonly prototype: ReadableStreamDefaultReader;
  new <R = any>(stream: ReadableStream<R>): ReadableStreamDefaultReader<R>;
};

/** @category Streams */
interface ReadableStreamBYOBReaderReadOptions {
  min?: number;
}

/** @category Streams */
interface ReadableStreamBYOBReader extends ReadableStreamGenericReader {
  read<T extends ArrayBufferView>(
    view: T,
    options?: ReadableStreamBYOBReaderReadOptions,
  ): Promise<ReadableStreamReadResult<T>>;
  releaseLock(): void;
}

/** @category Streams */
declare var ReadableStreamBYOBReader: {
  readonly prototype: ReadableStreamBYOBReader;
  new (stream: ReadableStream<Uint8Array>): ReadableStreamBYOBReader;
};

/** @category Streams */
interface ReadableStreamBYOBRequest {
  readonly view: ArrayBufferView | null;
  respond(bytesWritten: number): void;
  respondWithNewView(view: ArrayBufferView): void;
}

/** @category Streams */
declare var ReadableStreamBYOBRequest: {
  readonly prototype: ReadableStreamBYOBRequest;
  new (): never;
};

/** @category Streams */
interface UnderlyingByteSource {
  autoAllocateChunkSize?: number;
  cancel?: UnderlyingSourceCancelCallback;
  pull?: (controller: ReadableByteStreamController) => void | PromiseLike<void>;
  start?: (controller: ReadableByteStreamController) => any;
  type: "bytes";
}

/** @category Streams */
interface UnderlyingDefaultSource<R = any> {
  cancel?: UnderlyingSourceCancelCallback;
  pull?: (
    controller: ReadableStreamDefaultController<R>,
  ) => void | PromiseLike<void>;
  start?: (controller: ReadableStreamDefaultController<R>) => any;
  type?: undefined;
}

/** @category Streams */
interface UnderlyingSink<W = any> {
  abort?: UnderlyingSinkAbortCallback;
  close?: UnderlyingSinkCloseCallback;
  start?: UnderlyingSinkStartCallback;
  type?: undefined;
  write?: UnderlyingSinkWriteCallback<W>;
}

/** @category Streams */
type ReadableStreamType = "bytes";

/** @category Streams */
interface UnderlyingSource<R = any> {
  autoAllocateChunkSize?: number;
  cancel?: UnderlyingSourceCancelCallback;
  pull?: UnderlyingSourcePullCallback<R>;
  start?: UnderlyingSourceStartCallback<R>;
  type?: ReadableStreamType;
}

/** @category Streams */
interface UnderlyingSourceCancelCallback {
  (reason?: any): void | PromiseLike<void>;
}

/** @category Streams */
interface UnderlyingSourcePullCallback<R> {
  (controller: ReadableStreamController<R>): void | PromiseLike<void>;
}

/** @category Streams */
interface UnderlyingSourceStartCallback<R> {
  (controller: ReadableStreamController<R>): any;
}

/** @category Streams */
interface ReadableStreamDefaultController<R = any> {
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk?: R): void;
  error(e?: any): void;
}

/** @category Streams */
declare var ReadableStreamDefaultController: {
  readonly prototype: ReadableStreamDefaultController;
  new (): never;
};

/** @category Streams */
interface ReadableByteStreamController {
  readonly byobRequest: ReadableStreamBYOBRequest | null;
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk: ArrayBufferView): void;
  error(e?: any): void;
}

/** @category Streams */
declare var ReadableByteStreamController: {
  readonly prototype: ReadableByteStreamController;
  new (): never;
};

/** @category Streams */
interface StreamPipeOptions {
  preventAbort?: boolean;
  preventCancel?: boolean;
  preventClose?: boolean;
  signal?: AbortSignal;
}

/** @category Streams */
interface QueuingStrategySize<T = any> {
  (chunk: T): number;
}

/** @category Streams */
interface QueuingStrategy<T = any> {
  highWaterMark?: number;
  size?: QueuingStrategySize<T>;
}

/** This Streams API interface provides a built-in byte length queuing strategy
 * that can be used when constructing streams.
 *
 * @category Streams
 */
interface CountQueuingStrategy extends QueuingStrategy {
  readonly highWaterMark: number;
  readonly size: QueuingStrategySize;
}

/** @category Streams */
declare var CountQueuingStrategy: {
  readonly prototype: CountQueuingStrategy;
  new (init: QueuingStrategyInit): CountQueuingStrategy;
};

/** @category Streams */
interface ByteLengthQueuingStrategy extends QueuingStrategy<ArrayBufferView> {
  readonly highWaterMark: number;
  readonly size: QueuingStrategySize<ArrayBufferView>;
}

/** @category Streams */
declare var ByteLengthQueuingStrategy: {
  readonly prototype: ByteLengthQueuingStrategy;
  new (init: QueuingStrategyInit): ByteLengthQueuingStrategy;
};

/** @category Streams */
interface QueuingStrategyInit {
  highWaterMark: number;
}

/** This Streams API interface represents a readable stream of byte data. The
 * Fetch API offers a concrete instance of a ReadableStream through the body
 * property of a Response object.
 *
 * @category Streams
 */
interface ReadableStream<R = any> {
  readonly locked: boolean;
  cancel(reason?: any): Promise<void>;
  getReader(options: { mode: "byob" }): ReadableStreamBYOBReader;
  getReader(): ReadableStreamDefaultReader<R>;
  getReader(options?: ReadableStreamGetReaderOptions): ReadableStreamReader<R>;
  pipeThrough<T>(
    transform: ReadableWritablePair<T, R>,
    options?: StreamPipeOptions,
  ): ReadableStream<T>;
  pipeTo(
    destination: WritableStream<R>,
    options?: StreamPipeOptions,
  ): Promise<void>;
  tee(): [ReadableStream<R>, ReadableStream<R>];
  values(options?: ReadableStreamIteratorOptions): AsyncIterableIterator<R>;
  [Symbol.asyncIterator](
    options?: ReadableStreamIteratorOptions,
  ): AsyncIterableIterator<R>;
}

/** @category Streams */
declare var ReadableStream: {
  readonly prototype: ReadableStream;
  new (
    underlyingSource: UnderlyingByteSource,
    strategy?: { highWaterMark?: number },
  ): ReadableStream<Uint8Array>;
  new <R = any>(
    underlyingSource: UnderlyingDefaultSource<R>,
    strategy?: QueuingStrategy<R>,
  ): ReadableStream<R>;
  new <R = any>(
    underlyingSource?: UnderlyingSource<R>,
    strategy?: QueuingStrategy<R>,
  ): ReadableStream<R>;
  from<R>(
    asyncIterable: AsyncIterable<R> | Iterable<R | PromiseLike<R>> & object,
  ): ReadableStream<R>;
};

/** @category Streams */
interface ReadableStreamIteratorOptions {
  preventCancel?: boolean;
}

/** @category Streams */
type ReadableStreamReaderMode = "byob";

/** @category Streams */
interface ReadableStreamGetReaderOptions {
  mode?: ReadableStreamReaderMode;
}

/** @category Streams */
interface ReadableWritablePair<R = any, W = any> {
  readable: ReadableStream<R>;
  writable: WritableStream<W>;
}

/** @category Streams */
interface UnderlyingSinkCloseCallback {
  (): void | PromiseLike<void>;
}

/** @category Streams */
interface UnderlyingSinkStartCallback {
  (controller: WritableStreamDefaultController): any;
}

/** @category Streams */
interface UnderlyingSinkWriteCallback<W> {
  (
    chunk: W,
    controller: WritableStreamDefaultController,
  ): void | PromiseLike<void>;
}

/** @category Streams */
interface UnderlyingSinkAbortCallback {
  (reason?: any): void | PromiseLike<void>;
}

/** This Streams API interface provides a standard abstraction for writing
 * streaming data to a destination, known as a sink. This object comes with
 * built-in backpressure and queuing.
 *
 * @category Streams
 */
interface WritableStream<W = any> {
  readonly locked: boolean;
  abort(reason?: any): Promise<void>;
  close(): Promise<void>;
  getWriter(): WritableStreamDefaultWriter<W>;
}

/** @category Streams */
declare var WritableStream: {
  readonly prototype: WritableStream;
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
 * @category Streams
 */
interface WritableStreamDefaultController {
  readonly signal: AbortSignal;
  error(e?: any): void;
}

/** @category Streams */
declare var WritableStreamDefaultController: {
  readonly prototype: WritableStreamDefaultController;
  new (): never;
};

/** This Streams API interface is the object returned by
 * WritableStream.getWriter() and once created locks the < writer to the
 * WritableStream ensuring that no other streams can write to the underlying
 * sink.
 *
 * @category Streams
 */
interface WritableStreamDefaultWriter<W = any> {
  readonly closed: Promise<undefined>;
  readonly desiredSize: number | null;
  readonly ready: Promise<undefined>;
  abort(reason?: any): Promise<void>;
  close(): Promise<void>;
  releaseLock(): void;
  write(chunk?: W): Promise<void>;
}

/** @category Streams */
declare var WritableStreamDefaultWriter: {
  readonly prototype: WritableStreamDefaultWriter;
  new <W = any>(stream: WritableStream<W>): WritableStreamDefaultWriter<W>;
};

/** @category Streams */
interface TransformStream<I = any, O = any> {
  readonly readable: ReadableStream<O>;
  readonly writable: WritableStream<I>;
}

/** @category Streams */
declare var TransformStream: {
  readonly prototype: TransformStream;
  new <I = any, O = any>(
    transformer?: Transformer<I, O>,
    writableStrategy?: QueuingStrategy<I>,
    readableStrategy?: QueuingStrategy<O>,
  ): TransformStream<I, O>;
};

/** @category Streams */
interface TransformStreamDefaultController<O = any> {
  readonly desiredSize: number | null;
  enqueue(chunk?: O): void;
  error(reason?: any): void;
  terminate(): void;
}

/** @category Streams */
declare var TransformStreamDefaultController: {
  readonly prototype: TransformStreamDefaultController;
  new (): never;
};

/** @category Streams */
interface Transformer<I = any, O = any> {
  flush?: TransformerFlushCallback<O>;
  readableType?: undefined;
  start?: TransformerStartCallback<O>;
  transform?: TransformerTransformCallback<I, O>;
  cancel?: TransformerCancelCallback;
  writableType?: undefined;
}

/** @category Streams */
interface TransformerFlushCallback<O> {
  (controller: TransformStreamDefaultController<O>): void | PromiseLike<void>;
}

/** @category Streams */
interface TransformerStartCallback<O> {
  (controller: TransformStreamDefaultController<O>): any;
}

/** @category Streams */
interface TransformerTransformCallback<I, O> {
  (
    chunk: I,
    controller: TransformStreamDefaultController<O>,
  ): void | PromiseLike<void>;
}

/** @category Streams */
interface TransformerCancelCallback {
  (reason: any): void | PromiseLike<void>;
}

/** @category Streams */
interface GenericTransformStream {
  readonly readable: ReadableStream;
  readonly writable: WritableStream;
}

/** @category Events */
type MessageEventSource = Window | MessagePort;

/** @category Events */
interface MessageEventInit<T = any> extends EventInit {
  data?: T;
  lastEventId?: string;
  origin?: string;
  ports?: MessagePort[];
  source?: MessageEventSource | null;
}

/** @category Events */
interface MessageEvent<T = any> extends Event {
  /**
   * Returns the data of the message.
   */
  readonly data: T;
  /**
   * Returns the origin of the message, for server-sent events.
   */
  readonly origin: string;
  /**
   * Returns the last event ID string, for server-sent events.
   */
  readonly lastEventId: string;
  readonly source: MessageEventSource | null;
  /**
   * Returns transferred ports.
   */
  readonly ports: ReadonlyArray<MessagePort>;
  /** @deprecated */
  initMessageEvent(
    type: string,
    bubbles?: boolean,
    cancelable?: boolean,
    data?: any,
    origin?: string,
    lastEventId?: string,
    source?: MessageEventSource | null,
    ports?: MessagePort[],
  ): void;
}

/** @category Events */
declare var MessageEvent: {
  readonly prototype: MessageEvent;
  new <T>(type: string, eventInitDict?: MessageEventInit<T>): MessageEvent<T>;
};

/** @category Events */
type Transferable = MessagePort | ArrayBuffer;

/** @category Platform */
interface StructuredSerializeOptions {
  transfer?: Transferable[];
}

/** The MessageChannel interface of the Channel Messaging API allows us to
 * create a new message channel and send data through it via its two MessagePort
 * properties.
 *
 * @category Messaging
 */
interface MessageChannel {
  readonly port1: MessagePort;
  readonly port2: MessagePort;
}

/** The MessageChannel interface of the Channel Messaging API allows us to
 * create a new message channel and send data through it via its two MessagePort
 * properties.
 *
 * @category Messaging
 */
declare var MessageChannel: {
  readonly prototype: MessageChannel;
  new (): MessageChannel;
};

/** @category Messaging */
interface MessagePortEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** The MessagePort interface of the Channel Messaging API represents one of the
 * two ports of a MessageChannel, allowing messages to be sent from one port and
 * listening out for them arriving at the other.
 *
 * @category Messaging
 */
interface MessagePort extends EventTarget {
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

/** The MessagePort interface of the Channel Messaging API represents one of the
 * two ports of a MessageChannel, allowing messages to be sent from one port and
 * listening out for them arriving at the other.
 *
 * @category Messaging
 */
declare var MessagePort: {
  readonly prototype: MessagePort;
  new (): never;
};

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
 * @category Platform
 */
declare function structuredClone<T = any>(
  value: T,
  options?: StructuredSerializeOptions,
): T;

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
 * @category Streams
 */
interface CompressionStream extends GenericTransformStream {
  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<BufferSource>;
}

/** @category Streams */
type CompressionFormat = "deflate" | "deflate-raw" | "gzip";

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
 * @category Streams
 */
declare var CompressionStream: {
  readonly prototype: CompressionStream;
  /**
   * Creates a new `CompressionStream` object which compresses a stream of
   * data.
   *
   * Throws a `TypeError` if the format passed to the constructor is not
   * supported.
   */
  new (format: CompressionFormat): CompressionStream;
};

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
 * @category Streams
 */
interface DecompressionStream extends GenericTransformStream {
  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<BufferSource>;
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
 * @category Streams
 */
declare var DecompressionStream: {
  readonly prototype: DecompressionStream;
  /**
   * Creates a new `DecompressionStream` object which decompresses a stream of
   * data.
   *
   * Throws a `TypeError` if the format passed to the constructor is not
   * supported.
   */
  new (format: CompressionFormat): DecompressionStream;
};

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
 * @category Platform
 */
declare function reportError(
  error: any,
): void;

/** @category Platform */
type PredefinedColorSpace = "srgb" | "display-p3";

/** @category Platform */
interface ImageDataSettings {
  readonly colorSpace?: PredefinedColorSpace;
}

/** @category Platform */
interface ImageData {
  readonly colorSpace: PredefinedColorSpace;
  readonly data: Uint8ClampedArray;
  readonly height: number;
  readonly width: number;
}

/** @category Platform */
declare var ImageData: {
  readonly prototype: ImageData;
  new (sw: number, sh: number, settings?: ImageDataSettings): ImageData;
  new (
    data: Uint8ClampedArray,
    sw: number,
    sh?: number,
    settings?: ImageDataSettings,
  ): ImageData;
};

/** @category Platform */
interface WebTransportCloseInfo {
  closeCode?: number;
  reason?: string;
}

/** @category Platform */
interface WebTransportErrorOptions {
  source?: WebTransportErrorSource;
  streamErrorCode?: number | null;
}

/** @category Platform */
interface WebTransportHash {
  algorithm?: string;
  value?: BufferSource;
}

/** @category Platform */
interface WebTransportOptions {
  allowPooling?: boolean;
  congestionControl?: WebTransportCongestionControl;
  requireUnreliable?: boolean;
  serverCertificateHashes?: WebTransportHash[];
}

/** @category Platform */
interface WebTransportSendStreamOptions {
  sendGroup?: WebTransportSendGroup;
  sendOrder?: number;
  waitUntilAvailable?: boolean;
}

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport)
 * @category Platform
 */
interface WebTransport {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/closed) */
  readonly closed: Promise<WebTransportCloseInfo>;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/datagrams) */
  readonly datagrams: WebTransportDatagramDuplexStream;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/incomingBidirectionalStreams) */
  readonly incomingBidirectionalStreams: ReadableStream<
    WebTransportBidirectionalStream
  >;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/incomingUnidirectionalStreams) */
  readonly incomingUnidirectionalStreams: ReadableStream<
    WebTransportReceiveStream
  >;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/ready) */
  readonly ready: Promise<undefined>;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/close) */
  close(closeInfo?: WebTransportCloseInfo): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/createBidirectionalStream) */
  createBidirectionalStream(
    options?: WebTransportSendStreamOptions,
  ): Promise<WebTransportBidirectionalStream>;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/createUnidirectionalStream) */
  createUnidirectionalStream(
    options?: WebTransportSendStreamOptions,
  ): Promise<WebTransportSendStream>;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransport/createSendGroup) */
  createSendGroup(): WebTransportSendGroup;
}

/** @category Platform */
declare var WebTransport: {
  prototype: WebTransport;
  new (url: string | URL, options?: WebTransportOptions): WebTransport;
};

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportBidirectionalStream)
 * @category Platform
 */
interface WebTransportBidirectionalStream {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportBidirectionalStream/readable) */
  readonly readable: WebTransportReceiveStream;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportBidirectionalStream/writable) */
  readonly writable: WebTransportSendStream;
}

/** @category Platform */
declare var WebTransportBidirectionalStream: {
  prototype: WebTransportBidirectionalStream;
  new (): WebTransportBidirectionalStream;
};

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream)
 * @category Platform
 */
interface WebTransportDatagramDuplexStream {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/incomingHighWaterMark) */
  incomingHighWaterMark: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/incomingMaxAge) */
  incomingMaxAge: number | null;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/maxDatagramSize) */
  readonly maxDatagramSize: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/outgoingHighWaterMark) */
  outgoingHighWaterMark: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/outgoingMaxAge) */
  outgoingMaxAge: number | null;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/readable) */
  readonly readable: WebTransportReceiveStream;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportDatagramDuplexStream/writable) */
  readonly writable: WebTransportSendStream;
}

/** @category Platform */
declare var WebTransportDatagramDuplexStream: {
  prototype: WebTransportDatagramDuplexStream;
  new (): WebTransportDatagramDuplexStream;
};

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendStream)
 * @category Platform
 */
interface WebTransportSendStream extends WritableStream<Uint8Array> {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendStream/sendOrder) */
  sendOrder: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendStream/sendGroup) */
  sendGroup?: WebTransportSendGroup;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendStream/getStats) */
  getStats(): Promise<WebTransportSendStreamStats>;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendStream/getWriter) */
  getWriter(): WebTransportWriter;
}

/** @category Platform */
declare var WebTransportSendStream: {
  prototype: WebTransportSendStream;
  new (): WebTransportSendStream;
};

/** @category Platform */
interface WebTransportSendStreamStats {
  bytesWritten: number;
  bytesSent: number;
  bytesAcknowledged: number;
}

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportWriter)
 * @category Platform
 */
interface WebTransportWriter extends WritableStreamDefaultWriter<Uint8Array> {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportWriter/atomicWrite) */
  atomicWrite(chunk: any): Promise<undefined>;
}

/** @category Platform */
declare var WebTransportWriter: {
  prototype: WebTransportWriter;
  new (): WebTransportWriter;
};

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportReceiveStream)
 * @category Platform
 */
interface WebTransportReceiveStream extends ReadableStream<Uint8Array> {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportReceiveStream/getStats) */
  getStats(): Promise<WebTransportReceiveStreamStats>;
}

/** @category Platform */
declare var WebTransportReceiveStream: {
  prototype: WebTransportReceiveStream;
  new (): WebTransportReceiveStream;
};

/** @category Platform */
interface WebTransportReceiveStreamStats {
  bytesReceived: number;
  bytesRead: number;
}

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendGroup)
 * @category Platform
 */
interface WebTransportSendGroup {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportSendGroup/getStats) */
  getStats(): Promise<WebTransportSendStreamStats>;
}

/** @category Platform */
declare var WebTransportSendGroup: {
  prototype: WebTransportSendGroup;
  new (): WebTransportSendGroup;
};

/**
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportError)
 * @category Platform
 */
interface WebTransportError extends DOMException {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportError/source) */
  readonly source: WebTransportErrorSource;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/WebTransportError/streamErrorCode) */
  readonly streamErrorCode: number | null;
}

/** @category Platform */
declare var WebTransportError: {
  prototype: WebTransportError;
  new (message?: string, options?: WebTransportErrorOptions): WebTransportError;
};

/** @category Platform */
type WebTransportCongestionControl = "default" | "low-latency" | "throughput";

/** @category Platform */
type WebTransportErrorSource = "session" | "stream";
