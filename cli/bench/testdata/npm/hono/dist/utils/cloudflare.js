"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getContentFromKVAsset = void 0;
const getContentFromKVAsset = async (path, options) => {
    let ASSET_MANIFEST = {};
    if (options && options.manifest) {
        if (typeof options.manifest === 'string') {
            ASSET_MANIFEST = JSON.parse(options.manifest);
        }
        else {
            ASSET_MANIFEST = options.manifest;
        }
    }
    else {
        if (typeof __STATIC_CONTENT_MANIFEST === 'string') {
            ASSET_MANIFEST = JSON.parse(__STATIC_CONTENT_MANIFEST);
        }
        else {
            ASSET_MANIFEST = __STATIC_CONTENT_MANIFEST;
        }
    }
    let ASSET_NAMESPACE;
    if (options && options.namespace) {
        ASSET_NAMESPACE = options.namespace;
    }
    else {
        ASSET_NAMESPACE = __STATIC_CONTENT;
    }
    const key = ASSET_MANIFEST[path] || path;
    if (!key) {
        return null;
    }
    let content = await ASSET_NAMESPACE.get(key, { type: 'arrayBuffer' });
    if (content) {
        content = content;
    }
    return content;
};
exports.getContentFromKVAsset = getContentFromKVAsset;
