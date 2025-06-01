export declare const METHOD_NAME_ALL: "ALL";
export declare const METHOD_NAME_ALL_LOWERCASE: "all";
export interface Router<T> {
    add(method: string, path: string, handler: T): void;
    match(method: string, path: string): Result<T> | null;
}
export interface Result<T> {
    handlers: T[];
    params: Record<string, string>;
}
