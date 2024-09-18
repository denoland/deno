"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getFilePath = void 0;
const getFilePath = (options) => {
    let filename = options.filename;
    let root = options.root || '';
    const defaultDocument = options.defaultDocument || 'index.html';
    if (filename.endsWith('/')) {
        // /top/ => /top/index.html
        filename = filename.concat(defaultDocument);
    }
    else if (!filename.match(/\.[a-zA-Z0-9]+$/)) {
        // /top => /top/index.html
        filename = filename.concat('/' + defaultDocument);
    }
    // /foo.html => foo.html
    filename = filename.replace(/^\.?\//, '');
    // assets/ => assets
    root = root.replace(/\/$/, '');
    // ./assets/foo.html => assets/foo.html
    let path = root ? root + '/' + filename : filename;
    path = path.replace(/^\.?\//, '');
    return path;
};
exports.getFilePath = getFilePath;
