type _ByteLengthQueuingStrategy = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ByteLengthQueuingStrategy;
type _CompressionStream = typeof globalThis extends { onmessage: any; ReportingObserver: any } ? {}
    : import("stream/web").CompressionStream;
type _CountQueuingStrategy = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").CountQueuingStrategy;
type _DecompressionStream = typeof globalThis extends { onmessage: any; ReportingObserver: any } ? {}
    : import("stream/web").DecompressionStream;
type _ReadableByteStreamController = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableByteStreamController;
type _ReadableStream<R = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableStream<R>;
type _ReadableStreamBYOBReader = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableStreamBYOBReader;
type _ReadableStreamBYOBRequest = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableStreamBYOBRequest;
type _ReadableStreamDefaultController<R = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableStreamDefaultController<R>;
type _ReadableStreamDefaultReader<R = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").ReadableStreamDefaultReader<R>;
type _TextDecoderStream = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").TextDecoderStream;
type _TextEncoderStream = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").TextEncoderStream;
type _TransformStream<I = any, O = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").TransformStream<I, O>;
type _TransformStreamDefaultController<O = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").TransformStreamDefaultController<O>;
type _WritableStream<W = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").WritableStream<W>;
type _WritableStreamDefaultController = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").WritableStreamDefaultController;
type _WritableStreamDefaultWriter<W = any> = typeof globalThis extends { onmessage: any } ? {}
    : import("stream/web").WritableStreamDefaultWriter<W>;

declare module "stream/web" {
    // stub module, pending copy&paste from .d.ts or manual impl
    // copy from lib.dom.d.ts
    type ReadableWritablePair<R = any, W = any> = globalThis.ReadableWritablePair<R, W>;
    type StreamPipeOptions = globalThis.StreamPipeOptions;
    type ReadableStreamGenericReader = globalThis.ReadableStreamGenericReader;
    type ReadableStreamController<T> = globalThis.ReadableStreamController<T>;
    type ReadableStreamReadValueResult<T> = globalThis.ReadableStreamReadValueResult<T>;
    type ReadableStreamReadDoneResult<T> = globalThis.ReadableStreamReadDoneResult<T>;
    type ReadableStreamReadResult<T> = globalThis.ReadableStreamReadResult<T>;
    interface ReadableByteStreamControllerCallback {
        (controller: ReadableByteStreamController): void | PromiseLike<void>;
    }
    type UnderlyingSinkAbortCallback = globalThis.UnderlyingSinkAbortCallback;
    type UnderlyingSinkCloseCallback = globalThis.UnderlyingSinkCloseCallback;
    type UnderlyingSinkStartCallback = globalThis.UnderlyingSinkStartCallback;
    type UnderlyingSinkWriteCallback<W> = globalThis.UnderlyingSinkWriteCallback<W>;
    type UnderlyingSourceCancelCallback = globalThis.UnderlyingSourceCancelCallback;
    type UnderlyingSourcePullCallback<R> = globalThis.UnderlyingSourcePullCallback<R>;
    type UnderlyingSourceStartCallback<R> = globalThis.UnderlyingSourceStartCallback<R>;
    type TransformerFlushCallback<O> = globalThis.TransformerFlushCallback<O>;
    type TransformerStartCallback<O> = globalThis.TransformerStartCallback<O>;
    type TransformerTransformCallback<I, O> = globalThis.TransformerTransformCallback<I, O>;
    type TransformerCancelCallback = globalThis.TransformerCancelCallback;
    type UnderlyingByteSource = globalThis.UnderlyingByteSource;
    type UnderlyingSource<R = any> = globalThis.UnderlyingSource<R>;
    type UnderlyingSink<W = any> = globalThis.UnderlyingSink<W>;
    interface ReadableStreamErrorCallback {
        (reason: any): void | PromiseLike<void>;
    }
    interface ReadableStreamAsyncIterator<T> extends NodeJS.AsyncIterator<T, NodeJS.BuiltinIteratorReturn, unknown> {
        [Symbol.asyncIterator](): ReadableStreamAsyncIterator<T>;
    }
    type ReadableStream<R = any> = globalThis.ReadableStream<R>;
    const ReadableStream: typeof globalThis.ReadableStream;
    type ReadableStreamReaderMode = globalThis.ReadableStreamReaderMode;
    type ReadableStreamGetReaderOptions = globalThis.ReadableStreamGetReaderOptions;
    type ReadableStreamReader<T> = globalThis.ReadableStreamReader<T>;
    type ReadableStreamDefaultReader<R = any> = globalThis.ReadableStreamDefaultReader<R>;
    type ReadableStreamBYOBReader = globalThis.ReadableStreamBYOBReader;
    const ReadableStreamDefaultReader: typeof globalThis.ReadableStreamDefaultReader;
    const ReadableStreamBYOBReader: typeof globalThis.ReadableStreamBYOBReader;
    type ReadableStreamBYOBRequest = globalThis.ReadableStreamBYOBRequest;
    const ReadableStreamBYOBRequest: typeof globalThis.ReadableStreamBYOBRequest;
    type ReadableByteStreamController = globalThis.ReadableByteStreamController;
    const ReadableByteStreamController: typeof globalThis.ReadableByteStreamController;
    type ReadableStreamDefaultController<R = any> = globalThis.ReadableStreamDefaultController<R>;
    const ReadableStreamDefaultController: typeof globalThis.ReadableStreamDefaultController;
    type Transformer<I = any, O = any> = globalThis.Transformer<I, O>;
    type TransformStream<I = any, O = any> = globalThis.TransformStream<I, O>;
    const TransformStream: typeof globalThis.TransformStream;
    type TransformStreamDefaultController<O = any> = globalThis.TransformStreamDefaultController<O>;
    const TransformStreamDefaultController: typeof globalThis.TransformStreamDefaultController;
    type WritableStream<W = any> = globalThis.WritableStream<W>;
    const WritableStream: typeof globalThis.WritableStream;
    type WritableStreamDefaultWriter<W = any> = globalThis.WritableStreamDefaultWriter<W>;
    const WritableStreamDefaultWriter: typeof globalThis.WritableStreamDefaultWriter;
    type WritableStreamDefaultController = globalThis.WritableStreamDefaultController;
    const WritableStreamDefaultController: typeof globalThis.WritableStreamDefaultController;
    type QueuingStrategy<T = any> = globalThis.QueuingStrategy<T>;
    type QueuingStrategySize<T = any> = globalThis.QueuingStrategySize<T>;
    type QueuingStrategyInit = globalThis.QueuingStrategyInit;
    type ByteLengthQueuingStrategy = globalThis.ByteLengthQueuingStrategy;
    const ByteLengthQueuingStrategy: typeof globalThis.ByteLengthQueuingStrategy;
    type CountQueuingStrategy = globalThis.CountQueuingStrategy;
    const CountQueuingStrategy: typeof globalThis.CountQueuingStrategy;
    type TextEncoderStream = globalThis.TextEncoderStream;
    const TextEncoderStream: typeof globalThis.TextEncoderStream;
    type TextDecoderOptions = globalThis.TextDecoderOptions;
    type BufferSource = ArrayBufferView | ArrayBuffer;
    type TextDecoderStream = globalThis.TextDecoderStream;
    const TextDecoderStream: typeof globalThis.TextDecoderStream;
    type CompressionStream = globalThis.CompressionStream;
    const CompressionStream: typeof globalThis.CompressionStream;
    type DecompressionStream = globalThis.DecompressionStream;
    const DecompressionStream: typeof globalThis.DecompressionStream;
}
declare module "node:stream/web" {
    export * from "stream/web";
}
