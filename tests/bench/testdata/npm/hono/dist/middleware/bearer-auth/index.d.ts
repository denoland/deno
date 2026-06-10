import type { Context } from '../../context';
import type { Next } from '../../hono';
export declare const bearerAuth: (options: {
    token: string;
    realm?: string;
    prefix?: string;
    hashFunction?: Function;
}) => (c: Context, next: Next) => Promise<void>;
