import type { Context } from '../../context';
import type { Next } from '../../hono';
export declare const jwt: (options: {
    secret: string;
    cookie?: string;
    alg?: string;
}) => (ctx: Context, next: Next) => Promise<void>;
