declare type Algorithm = {
    name: string;
    alias: string;
};
declare type Data = string | boolean | number | object | ArrayBufferView | ArrayBuffer | ReadableStream;
export declare const sha256: (data: Data) => Promise<string | null>;
export declare const sha1: (data: Data) => Promise<string | null>;
export declare const md5: (data: Data) => Promise<string | null>;
export declare const createHash: (data: Data, algorithm: Algorithm) => Promise<string | null>;
export {};
