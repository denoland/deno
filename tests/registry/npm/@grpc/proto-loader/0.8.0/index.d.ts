export type Long = string | number | bigint;
export type AnyExtension = Record<string, unknown>;
export type MethodDefinition<TRequest = unknown, TResponse = unknown> = {
  path: string;
  requestStream: boolean;
  responseStream: boolean;
  requestSerialize(value: TRequest): Buffer;
  requestDeserialize(bytes: Buffer): TRequest;
  responseSerialize(value: TResponse): Buffer;
  responseDeserialize(bytes: Buffer): TResponse;
};
export type MessageTypeDefinition = Record<string, unknown>;
export type EnumTypeDefinition = Record<string, unknown>;

export declare function load(...args: unknown[]): never;
export declare function loadSync(...args: unknown[]): never;
export declare function fromJSON(...args: unknown[]): never;
