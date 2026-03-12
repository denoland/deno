import type { Context } from '../../context';
import type { Next } from '../../hono';
declare type CORSOptions = {
    origin: string;
    allowMethods?: string[];
    allowHeaders?: string[];
    maxAge?: number;
    credentials?: boolean;
    exposeHeaders?: string[];
};
export declare const cors: (options?: CORSOptions) => (c: Context, next: Next) => Promise<void>;
export {};
