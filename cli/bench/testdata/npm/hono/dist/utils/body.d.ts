export declare type Body = string | object | Record<string, string | File> | ArrayBuffer;
export declare const parseBody: (r: Request | Response) => Promise<Body>;
