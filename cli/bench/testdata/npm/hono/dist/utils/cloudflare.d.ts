/// <reference types="@cloudflare/workers-types" />
export declare type KVAssetOptions = {
    manifest?: object | string;
    namespace?: KVNamespace;
};
export declare const getContentFromKVAsset: (path: string, options?: KVAssetOptions) => Promise<ArrayBuffer | null>;
