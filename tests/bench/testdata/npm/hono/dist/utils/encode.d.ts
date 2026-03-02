export declare const encodeBase64: (str: string) => string;
export declare const decodeBase64: (str: string) => string;
export declare const encodeBase64URL: (str: string) => string;
export declare const decodeBase64URL: (str: string) => string;
export declare const utf8ToUint8Array: (str: string) => Uint8Array;
export declare const arrayBufferToBase64: (buf: ArrayBuffer) => Promise<string>;
export declare const arrayBufferToBase64URL: (buf: ArrayBuffer) => Promise<string>;
