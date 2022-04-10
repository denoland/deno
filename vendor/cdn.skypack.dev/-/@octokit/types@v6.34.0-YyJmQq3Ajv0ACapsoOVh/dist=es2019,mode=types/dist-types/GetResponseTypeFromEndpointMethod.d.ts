declare type Unwrap<T> = T extends Promise<infer U> ? U : T;
declare type AnyFunction = (...args: any[]) => any;
export declare type GetResponseTypeFromEndpointMethod<T extends AnyFunction> = Unwrap<ReturnType<T>>;
export declare type GetResponseDataTypeFromEndpointMethod<T extends AnyFunction> = Unwrap<ReturnType<T>>["data"];
export {};
