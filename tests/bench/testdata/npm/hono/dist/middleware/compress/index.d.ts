import type { Context } from '../../context';
import type { Next } from '../../hono';
declare type EncodingType = 'gzip' | 'deflate';
interface CompressionOptions {
    encoding?: EncodingType;
}
export declare const compress: (options?: CompressionOptions) => (ctx: Context, next: Next) => Promise<void>;
export {};
