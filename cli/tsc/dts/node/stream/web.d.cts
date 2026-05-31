declare module "stream/web" {
    // The Web Streams types in Deno are exposed as globals and the
    // `node:stream/web` module re-exports the same classes. Aliasing the
    // module exports to the global types ensures values produced by the
    // platform (e.g. `new ReadableStream()`) are assignable to the parameter
    // types of `node:stream` helpers such as `Readable.fromWeb()`.
    type ReadableStream<R = any> = globalThis.ReadableStream<R>;
    const ReadableStream: typeof globalThis.ReadableStream;

    type ReadableStreamDefaultReader<R = any> = globalThis.ReadableStreamDefaultReader<R>;
    const ReadableStreamDefaultReader: typeof globalThis.ReadableStreamDefaultReader;

    type ReadableStreamBYOBReader = globalThis.ReadableStreamBYOBReader;
    const ReadableStreamBYOBReader: typeof globalThis.ReadableStreamBYOBReader;

    type ReadableStreamBYOBRequest = globalThis.ReadableStreamBYOBRequest;
    const ReadableStreamBYOBRequest: typeof globalThis.ReadableStreamBYOBRequest;

    type ReadableStreamDefaultController<R = any> = globalThis.ReadableStreamDefaultController<R>;
    const ReadableStreamDefaultController: typeof globalThis.ReadableStreamDefaultController;

    type ReadableByteStreamController = globalThis.ReadableByteStreamController;
    const ReadableByteStreamController: typeof globalThis.ReadableByteStreamController;

    type WritableStream<W = any> = globalThis.WritableStream<W>;
    const WritableStream: typeof globalThis.WritableStream;

    type WritableStreamDefaultWriter<W = any> = globalThis.WritableStreamDefaultWriter<W>;
    const WritableStreamDefaultWriter: typeof globalThis.WritableStreamDefaultWriter;

    type WritableStreamDefaultController = globalThis.WritableStreamDefaultController;
    const WritableStreamDefaultController: typeof globalThis.WritableStreamDefaultController;

    type TransformStream<I = any, O = any> = globalThis.TransformStream<I, O>;
    const TransformStream: typeof globalThis.TransformStream;

    type TransformStreamDefaultController<O = any> = globalThis.TransformStreamDefaultController<O>;
    const TransformStreamDefaultController: typeof globalThis.TransformStreamDefaultController;

    type ByteLengthQueuingStrategy = globalThis.ByteLengthQueuingStrategy;
    const ByteLengthQueuingStrategy: typeof globalThis.ByteLengthQueuingStrategy;

    type CountQueuingStrategy = globalThis.CountQueuingStrategy;
    const CountQueuingStrategy: typeof globalThis.CountQueuingStrategy;

    type TextEncoderStream = globalThis.TextEncoderStream;
    const TextEncoderStream: typeof globalThis.TextEncoderStream;

    type TextDecoderStream = globalThis.TextDecoderStream;
    const TextDecoderStream: typeof globalThis.TextDecoderStream;

    type CompressionStream = globalThis.CompressionStream;
    const CompressionStream: typeof globalThis.CompressionStream;

    type DecompressionStream = globalThis.DecompressionStream;
    const DecompressionStream: typeof globalThis.DecompressionStream;

    // Interfaces and types that are part of the Web Streams API but do not
    // have direct value-level globals. Alias the structural types to their
    // global counterparts so they are interchangeable.
    type ReadableWritablePair<R = any, W = any> = globalThis.ReadableWritablePair<R, W>;
    type StreamPipeOptions = globalThis.StreamPipeOptions;
    type ReadableStreamGenericReader = globalThis.ReadableStreamGenericReader;
    type ReadableStreamController<T> = globalThis.ReadableStreamController<T>;
    type ReadableStreamReader<T> = globalThis.ReadableStreamReader<T>;
    type ReadableStreamReadValueResult<T> = globalThis.ReadableStreamReadValueResult<T>;
    type ReadableStreamReadDoneResult<T> = globalThis.ReadableStreamReadDoneResult<T>;
    type ReadableStreamReadResult<T> = globalThis.ReadableStreamReadResult<T>;
    type ReadableStreamReaderMode = globalThis.ReadableStreamReaderMode;
    type ReadableStreamGetReaderOptions = globalThis.ReadableStreamGetReaderOptions;
    type UnderlyingByteSource = globalThis.UnderlyingByteSource;
    type UnderlyingDefaultSource<R = any> = globalThis.UnderlyingDefaultSource<R>;
    type UnderlyingSink<W = any> = globalThis.UnderlyingSink<W>;
    type UnderlyingSource<R = any> = globalThis.UnderlyingSource<R>;
    type UnderlyingSinkAbortCallback = globalThis.UnderlyingSinkAbortCallback;
    type UnderlyingSinkCloseCallback = globalThis.UnderlyingSinkCloseCallback;
    type UnderlyingSinkStartCallback = globalThis.UnderlyingSinkStartCallback;
    type UnderlyingSinkWriteCallback<W> = globalThis.UnderlyingSinkWriteCallback<W>;
    type UnderlyingSourceCancelCallback = globalThis.UnderlyingSourceCancelCallback;
    type UnderlyingSourcePullCallback<R> = globalThis.UnderlyingSourcePullCallback<R>;
    type UnderlyingSourceStartCallback<R> = globalThis.UnderlyingSourceStartCallback<R>;
    type Transformer<I = any, O = any> = globalThis.Transformer<I, O>;
    type TransformerFlushCallback<O> = globalThis.TransformerFlushCallback<O>;
    type TransformerStartCallback<O> = globalThis.TransformerStartCallback<O>;
    type TransformerTransformCallback<I, O> = globalThis.TransformerTransformCallback<I, O>;
    type TransformerCancelCallback = globalThis.TransformerCancelCallback;
    type QueuingStrategy<T = any> = globalThis.QueuingStrategy<T>;
    type QueuingStrategySize<T = any> = globalThis.QueuingStrategySize<T>;
    type QueuingStrategyInit = globalThis.QueuingStrategyInit;
    type TextDecoderOptions = globalThis.TextDecoderOptions;

    // These types are Node-specific (not part of the Web Streams API).
    type BufferSource = ArrayBufferView | ArrayBuffer;
    interface ReadableStreamErrorCallback {
        (reason: any): void | PromiseLike<void>;
    }
    interface ReadableStreamAsyncIterator<T> extends AsyncIterableIterator<T> {
        [Symbol.asyncIterator](): ReadableStreamAsyncIterator<T>;
    }
}
declare module "node:stream/web" {
    export * from "stream/web";
}
