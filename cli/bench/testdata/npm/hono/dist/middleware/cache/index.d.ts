import type { Context } from '../../context';
import type { Next } from '../../hono';
export declare const cache: (options: {
    cacheName: string;
    wait?: boolean;
    cacheControl?: string;
}) => (c: Context, next: Next) => Promise<Response | undefined>;
