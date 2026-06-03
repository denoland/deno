"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.serveStatic = void 0;
const filepath_1 = require("../../utils/filepath");
const mime_1 = require("../../utils/mime");
// @ts-ignore
const { file } = Bun;
const DEFAULT_DOCUMENT = 'index.html';
const serveStatic = (options = { root: '' }) => {
    return async (c, next) => {
        // Do nothing if Response is already set
        if (c.res && c.finalized) {
            await next();
        }
        const url = new URL(c.req.url);
        let path = (0, filepath_1.getFilePath)({
            filename: options.path ?? url.pathname,
            root: options.root,
            defaultDocument: DEFAULT_DOCUMENT,
        });
        path = `./${path}`;
        const content = file(path);
        if (content) {
            const mimeType = (0, mime_1.getMimeType)(path);
            if (mimeType) {
                c.header('Content-Type', mimeType);
            }
            // Return Response object
            return c.body(content);
        }
        else {
            console.warn(`Static file: ${path} is not found`);
            await next();
        }
        return;
    };
};
exports.serveStatic = serveStatic;
