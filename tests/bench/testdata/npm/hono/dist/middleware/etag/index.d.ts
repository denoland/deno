import type { Context } from '../../context';
import type { Next } from '../../hono';
declare type ETagOptions = {
    weak: boolean;
};
export declare const etag: (options?: ETagOptions) => (c: Context, next: Next) => Promise<void>;
export {};
