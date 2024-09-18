"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getStatusText = void 0;
const getStatusText = (statusCode) => {
    const text = statuses[statusCode];
    return text;
};
exports.getStatusText = getStatusText;
const statuses = {
    100: 'Continue',
    101: 'Switching Protocols',
    102: 'Processing',
    103: 'Early Hints',
    200: 'OK',
    201: 'Created',
    202: 'Accepted',
    204: 'No Content',
    206: 'Partial Content',
    301: 'Moved Permanently',
    302: 'Moved Temporarily',
    303: 'See Other',
    304: 'Not Modified',
    307: 'Temporary Redirect',
    308: 'Permanent Redirect',
    400: 'Bad Request',
    401: 'Unauthorized',
    402: 'Payment Required',
    403: 'Forbidden',
    404: 'Not Found',
    405: 'Not Allowed',
    406: 'Not Acceptable',
    408: 'Request Time-out',
    409: 'Conflict',
    410: 'Gone',
    411: 'Length Required',
    412: 'Precondition Failed',
    413: 'Request Entity Too Large',
    414: 'Request-URI Too Large',
    415: 'Unsupported Media Type',
    416: 'Requested Range Not Satisfiable',
    421: 'Misdirected Request',
    429: 'Too Many Requests',
    500: 'Internal Server Error',
    501: 'Not Implemented',
    502: 'Bad Gateway',
    503: 'Service Temporarily Unavailable',
    504: 'Gateway Time-out',
    505: 'HTTP Version Not Supported',
    507: 'Insufficient Storage',
};
