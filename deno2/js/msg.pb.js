/*eslint-disable block-scoped-var, no-redeclare, no-control-regex, no-prototype-builtins*/
(function(global, factory) { /* global define, require, module */

    /* AMD */ if (typeof define === 'function' && define.amd)
        define(["protobufjs/minimal"], factory);

    /* CommonJS */ else if (typeof require === 'function' && typeof module === 'object' && module && module.exports)
        module.exports = factory(require("protobufjs/minimal"));

})(this, function($protobuf) {
    "use strict";

    // Common aliases
    var $Reader = $protobuf.Reader, $Writer = $protobuf.Writer, $util = $protobuf.util;
    
    // Exported root namespace
    var $root = $protobuf.roots["default"] || ($protobuf.roots["default"] = {});
    
    $root.deno = (function() {
    
        /**
         * Namespace deno.
         * @exports deno
         * @namespace
         */
        var deno = {};
    
        deno.BaseMsg = (function() {
    
            /**
             * Properties of a BaseMsg.
             * @memberof deno
             * @interface IBaseMsg
             * @property {string|null} [channel] BaseMsg channel
             * @property {Uint8Array|null} [payload] BaseMsg payload
             */
    
            /**
             * Constructs a new BaseMsg.
             * @memberof deno
             * @classdesc Represents a BaseMsg.
             * @implements IBaseMsg
             * @constructor
             * @param {deno.IBaseMsg=} [properties] Properties to set
             */
            function BaseMsg(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * BaseMsg channel.
             * @member {string} channel
             * @memberof deno.BaseMsg
             * @instance
             */
            BaseMsg.prototype.channel = "";
    
            /**
             * BaseMsg payload.
             * @member {Uint8Array} payload
             * @memberof deno.BaseMsg
             * @instance
             */
            BaseMsg.prototype.payload = $util.newBuffer([]);
    
            /**
             * Creates a new BaseMsg instance using the specified properties.
             * @function create
             * @memberof deno.BaseMsg
             * @static
             * @param {deno.IBaseMsg=} [properties] Properties to set
             * @returns {deno.BaseMsg} BaseMsg instance
             */
            BaseMsg.create = function create(properties) {
                return new BaseMsg(properties);
            };
    
            /**
             * Encodes the specified BaseMsg message. Does not implicitly {@link deno.BaseMsg.verify|verify} messages.
             * @function encode
             * @memberof deno.BaseMsg
             * @static
             * @param {deno.IBaseMsg} message BaseMsg message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            BaseMsg.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.channel != null && message.hasOwnProperty("channel"))
                    writer.uint32(/* id 1, wireType 2 =*/10).string(message.channel);
                if (message.payload != null && message.hasOwnProperty("payload"))
                    writer.uint32(/* id 2, wireType 2 =*/18).bytes(message.payload);
                return writer;
            };
    
            /**
             * Encodes the specified BaseMsg message, length delimited. Does not implicitly {@link deno.BaseMsg.verify|verify} messages.
             * @function encodeDelimited
             * @memberof deno.BaseMsg
             * @static
             * @param {deno.IBaseMsg} message BaseMsg message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            BaseMsg.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a BaseMsg message from the specified reader or buffer.
             * @function decode
             * @memberof deno.BaseMsg
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {deno.BaseMsg} BaseMsg
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            BaseMsg.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.deno.BaseMsg();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.channel = reader.string();
                        break;
                    case 2:
                        message.payload = reader.bytes();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a BaseMsg message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof deno.BaseMsg
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {deno.BaseMsg} BaseMsg
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            BaseMsg.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a BaseMsg message.
             * @function verify
             * @memberof deno.BaseMsg
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            BaseMsg.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.channel != null && message.hasOwnProperty("channel"))
                    if (!$util.isString(message.channel))
                        return "channel: string expected";
                if (message.payload != null && message.hasOwnProperty("payload"))
                    if (!(message.payload && typeof message.payload.length === "number" || $util.isString(message.payload)))
                        return "payload: buffer expected";
                return null;
            };
    
            /**
             * Creates a BaseMsg message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof deno.BaseMsg
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {deno.BaseMsg} BaseMsg
             */
            BaseMsg.fromObject = function fromObject(object) {
                if (object instanceof $root.deno.BaseMsg)
                    return object;
                var message = new $root.deno.BaseMsg();
                if (object.channel != null)
                    message.channel = String(object.channel);
                if (object.payload != null)
                    if (typeof object.payload === "string")
                        $util.base64.decode(object.payload, message.payload = $util.newBuffer($util.base64.length(object.payload)), 0);
                    else if (object.payload.length)
                        message.payload = object.payload;
                return message;
            };
    
            /**
             * Creates a plain object from a BaseMsg message. Also converts values to other types if specified.
             * @function toObject
             * @memberof deno.BaseMsg
             * @static
             * @param {deno.BaseMsg} message BaseMsg
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            BaseMsg.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    object.channel = "";
                    object.payload = options.bytes === String ? "" : [];
                }
                if (message.channel != null && message.hasOwnProperty("channel"))
                    object.channel = message.channel;
                if (message.payload != null && message.hasOwnProperty("payload"))
                    object.payload = options.bytes === String ? $util.base64.encode(message.payload, 0, message.payload.length) : options.bytes === Array ? Array.prototype.slice.call(message.payload) : message.payload;
                return object;
            };
    
            /**
             * Converts this BaseMsg to JSON.
             * @function toJSON
             * @memberof deno.BaseMsg
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            BaseMsg.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return BaseMsg;
        })();
    
        deno.Msg = (function() {
    
            /**
             * Properties of a Msg.
             * @memberof deno
             * @interface IMsg
             * @property {deno.Msg.Command|null} [command] Msg command
             * @property {string|null} [error] Msg error
             * @property {string|null} [startCwd] Msg startCwd
             * @property {Array.<string>|null} [startArgv] Msg startArgv
             * @property {boolean|null} [startDebugFlag] Msg startDebugFlag
             * @property {string|null} [startMainJs] Msg startMainJs
             * @property {string|null} [startMainMap] Msg startMainMap
             * @property {string|null} [codeFetchModuleSpecifier] Msg codeFetchModuleSpecifier
             * @property {string|null} [codeFetchContainingFile] Msg codeFetchContainingFile
             * @property {string|null} [codeFetchResModuleName] Msg codeFetchResModuleName
             * @property {string|null} [codeFetchResFilename] Msg codeFetchResFilename
             * @property {string|null} [codeFetchResSourceCode] Msg codeFetchResSourceCode
             * @property {string|null} [codeFetchResOutputCode] Msg codeFetchResOutputCode
             * @property {string|null} [codeCacheFilename] Msg codeCacheFilename
             * @property {string|null} [codeCacheSourceCode] Msg codeCacheSourceCode
             * @property {string|null} [codeCacheOutputCode] Msg codeCacheOutputCode
             * @property {number|null} [exitCode] Msg exitCode
             * @property {number|null} [timerStartId] Msg timerStartId
             * @property {boolean|null} [timerStartInterval] Msg timerStartInterval
             * @property {number|null} [timerStartDelay] Msg timerStartDelay
             * @property {number|null} [timerReadyId] Msg timerReadyId
             * @property {boolean|null} [timerReadyDone] Msg timerReadyDone
             * @property {number|null} [timerClearId] Msg timerClearId
             * @property {number|null} [fetchReqId] Msg fetchReqId
             * @property {string|null} [fetchReqUrl] Msg fetchReqUrl
             * @property {number|null} [fetchResId] Msg fetchResId
             * @property {number|null} [fetchResStatus] Msg fetchResStatus
             * @property {Array.<string>|null} [fetchResHeaderLine] Msg fetchResHeaderLine
             * @property {Uint8Array|null} [fetchResBody] Msg fetchResBody
             * @property {string|null} [readFileSyncFilename] Msg readFileSyncFilename
             * @property {Uint8Array|null} [readFileSyncData] Msg readFileSyncData
             * @property {string|null} [writeFileSyncFilename] Msg writeFileSyncFilename
             * @property {Uint8Array|null} [writeFileSyncData] Msg writeFileSyncData
             * @property {number|null} [writeFileSyncPerm] Msg writeFileSyncPerm
             */
    
            /**
             * Constructs a new Msg.
             * @memberof deno
             * @classdesc Represents a Msg.
             * @implements IMsg
             * @constructor
             * @param {deno.IMsg=} [properties] Properties to set
             */
            function Msg(properties) {
                this.startArgv = [];
                this.fetchResHeaderLine = [];
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * Msg command.
             * @member {deno.Msg.Command} command
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.command = 0;
    
            /**
             * Msg error.
             * @member {string} error
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.error = "";
    
            /**
             * Msg startCwd.
             * @member {string} startCwd
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.startCwd = "";
    
            /**
             * Msg startArgv.
             * @member {Array.<string>} startArgv
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.startArgv = $util.emptyArray;
    
            /**
             * Msg startDebugFlag.
             * @member {boolean} startDebugFlag
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.startDebugFlag = false;
    
            /**
             * Msg startMainJs.
             * @member {string} startMainJs
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.startMainJs = "";
    
            /**
             * Msg startMainMap.
             * @member {string} startMainMap
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.startMainMap = "";
    
            /**
             * Msg codeFetchModuleSpecifier.
             * @member {string} codeFetchModuleSpecifier
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchModuleSpecifier = "";
    
            /**
             * Msg codeFetchContainingFile.
             * @member {string} codeFetchContainingFile
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchContainingFile = "";
    
            /**
             * Msg codeFetchResModuleName.
             * @member {string} codeFetchResModuleName
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchResModuleName = "";
    
            /**
             * Msg codeFetchResFilename.
             * @member {string} codeFetchResFilename
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchResFilename = "";
    
            /**
             * Msg codeFetchResSourceCode.
             * @member {string} codeFetchResSourceCode
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchResSourceCode = "";
    
            /**
             * Msg codeFetchResOutputCode.
             * @member {string} codeFetchResOutputCode
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeFetchResOutputCode = "";
    
            /**
             * Msg codeCacheFilename.
             * @member {string} codeCacheFilename
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeCacheFilename = "";
    
            /**
             * Msg codeCacheSourceCode.
             * @member {string} codeCacheSourceCode
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeCacheSourceCode = "";
    
            /**
             * Msg codeCacheOutputCode.
             * @member {string} codeCacheOutputCode
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.codeCacheOutputCode = "";
    
            /**
             * Msg exitCode.
             * @member {number} exitCode
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.exitCode = 0;
    
            /**
             * Msg timerStartId.
             * @member {number} timerStartId
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerStartId = 0;
    
            /**
             * Msg timerStartInterval.
             * @member {boolean} timerStartInterval
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerStartInterval = false;
    
            /**
             * Msg timerStartDelay.
             * @member {number} timerStartDelay
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerStartDelay = 0;
    
            /**
             * Msg timerReadyId.
             * @member {number} timerReadyId
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerReadyId = 0;
    
            /**
             * Msg timerReadyDone.
             * @member {boolean} timerReadyDone
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerReadyDone = false;
    
            /**
             * Msg timerClearId.
             * @member {number} timerClearId
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.timerClearId = 0;
    
            /**
             * Msg fetchReqId.
             * @member {number} fetchReqId
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchReqId = 0;
    
            /**
             * Msg fetchReqUrl.
             * @member {string} fetchReqUrl
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchReqUrl = "";
    
            /**
             * Msg fetchResId.
             * @member {number} fetchResId
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchResId = 0;
    
            /**
             * Msg fetchResStatus.
             * @member {number} fetchResStatus
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchResStatus = 0;
    
            /**
             * Msg fetchResHeaderLine.
             * @member {Array.<string>} fetchResHeaderLine
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchResHeaderLine = $util.emptyArray;
    
            /**
             * Msg fetchResBody.
             * @member {Uint8Array} fetchResBody
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.fetchResBody = $util.newBuffer([]);
    
            /**
             * Msg readFileSyncFilename.
             * @member {string} readFileSyncFilename
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.readFileSyncFilename = "";
    
            /**
             * Msg readFileSyncData.
             * @member {Uint8Array} readFileSyncData
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.readFileSyncData = $util.newBuffer([]);
    
            /**
             * Msg writeFileSyncFilename.
             * @member {string} writeFileSyncFilename
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.writeFileSyncFilename = "";
    
            /**
             * Msg writeFileSyncData.
             * @member {Uint8Array} writeFileSyncData
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.writeFileSyncData = $util.newBuffer([]);
    
            /**
             * Msg writeFileSyncPerm.
             * @member {number} writeFileSyncPerm
             * @memberof deno.Msg
             * @instance
             */
            Msg.prototype.writeFileSyncPerm = 0;
    
            /**
             * Creates a new Msg instance using the specified properties.
             * @function create
             * @memberof deno.Msg
             * @static
             * @param {deno.IMsg=} [properties] Properties to set
             * @returns {deno.Msg} Msg instance
             */
            Msg.create = function create(properties) {
                return new Msg(properties);
            };
    
            /**
             * Encodes the specified Msg message. Does not implicitly {@link deno.Msg.verify|verify} messages.
             * @function encode
             * @memberof deno.Msg
             * @static
             * @param {deno.IMsg} message Msg message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Msg.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.command != null && message.hasOwnProperty("command"))
                    writer.uint32(/* id 1, wireType 0 =*/8).int32(message.command);
                if (message.error != null && message.hasOwnProperty("error"))
                    writer.uint32(/* id 2, wireType 2 =*/18).string(message.error);
                if (message.startCwd != null && message.hasOwnProperty("startCwd"))
                    writer.uint32(/* id 10, wireType 2 =*/82).string(message.startCwd);
                if (message.startArgv != null && message.startArgv.length)
                    for (var i = 0; i < message.startArgv.length; ++i)
                        writer.uint32(/* id 11, wireType 2 =*/90).string(message.startArgv[i]);
                if (message.startDebugFlag != null && message.hasOwnProperty("startDebugFlag"))
                    writer.uint32(/* id 12, wireType 0 =*/96).bool(message.startDebugFlag);
                if (message.startMainJs != null && message.hasOwnProperty("startMainJs"))
                    writer.uint32(/* id 13, wireType 2 =*/106).string(message.startMainJs);
                if (message.startMainMap != null && message.hasOwnProperty("startMainMap"))
                    writer.uint32(/* id 14, wireType 2 =*/114).string(message.startMainMap);
                if (message.codeFetchModuleSpecifier != null && message.hasOwnProperty("codeFetchModuleSpecifier"))
                    writer.uint32(/* id 20, wireType 2 =*/162).string(message.codeFetchModuleSpecifier);
                if (message.codeFetchContainingFile != null && message.hasOwnProperty("codeFetchContainingFile"))
                    writer.uint32(/* id 21, wireType 2 =*/170).string(message.codeFetchContainingFile);
                if (message.codeFetchResModuleName != null && message.hasOwnProperty("codeFetchResModuleName"))
                    writer.uint32(/* id 30, wireType 2 =*/242).string(message.codeFetchResModuleName);
                if (message.codeFetchResFilename != null && message.hasOwnProperty("codeFetchResFilename"))
                    writer.uint32(/* id 31, wireType 2 =*/250).string(message.codeFetchResFilename);
                if (message.codeFetchResSourceCode != null && message.hasOwnProperty("codeFetchResSourceCode"))
                    writer.uint32(/* id 32, wireType 2 =*/258).string(message.codeFetchResSourceCode);
                if (message.codeFetchResOutputCode != null && message.hasOwnProperty("codeFetchResOutputCode"))
                    writer.uint32(/* id 33, wireType 2 =*/266).string(message.codeFetchResOutputCode);
                if (message.codeCacheFilename != null && message.hasOwnProperty("codeCacheFilename"))
                    writer.uint32(/* id 41, wireType 2 =*/330).string(message.codeCacheFilename);
                if (message.codeCacheSourceCode != null && message.hasOwnProperty("codeCacheSourceCode"))
                    writer.uint32(/* id 42, wireType 2 =*/338).string(message.codeCacheSourceCode);
                if (message.codeCacheOutputCode != null && message.hasOwnProperty("codeCacheOutputCode"))
                    writer.uint32(/* id 43, wireType 2 =*/346).string(message.codeCacheOutputCode);
                if (message.exitCode != null && message.hasOwnProperty("exitCode"))
                    writer.uint32(/* id 50, wireType 0 =*/400).int32(message.exitCode);
                if (message.timerStartId != null && message.hasOwnProperty("timerStartId"))
                    writer.uint32(/* id 60, wireType 0 =*/480).int32(message.timerStartId);
                if (message.timerStartInterval != null && message.hasOwnProperty("timerStartInterval"))
                    writer.uint32(/* id 61, wireType 0 =*/488).bool(message.timerStartInterval);
                if (message.timerStartDelay != null && message.hasOwnProperty("timerStartDelay"))
                    writer.uint32(/* id 62, wireType 0 =*/496).int32(message.timerStartDelay);
                if (message.timerReadyId != null && message.hasOwnProperty("timerReadyId"))
                    writer.uint32(/* id 70, wireType 0 =*/560).int32(message.timerReadyId);
                if (message.timerReadyDone != null && message.hasOwnProperty("timerReadyDone"))
                    writer.uint32(/* id 71, wireType 0 =*/568).bool(message.timerReadyDone);
                if (message.timerClearId != null && message.hasOwnProperty("timerClearId"))
                    writer.uint32(/* id 80, wireType 0 =*/640).int32(message.timerClearId);
                if (message.fetchReqId != null && message.hasOwnProperty("fetchReqId"))
                    writer.uint32(/* id 90, wireType 0 =*/720).int32(message.fetchReqId);
                if (message.fetchReqUrl != null && message.hasOwnProperty("fetchReqUrl"))
                    writer.uint32(/* id 91, wireType 2 =*/730).string(message.fetchReqUrl);
                if (message.fetchResId != null && message.hasOwnProperty("fetchResId"))
                    writer.uint32(/* id 100, wireType 0 =*/800).int32(message.fetchResId);
                if (message.fetchResStatus != null && message.hasOwnProperty("fetchResStatus"))
                    writer.uint32(/* id 101, wireType 0 =*/808).int32(message.fetchResStatus);
                if (message.fetchResHeaderLine != null && message.fetchResHeaderLine.length)
                    for (var i = 0; i < message.fetchResHeaderLine.length; ++i)
                        writer.uint32(/* id 102, wireType 2 =*/818).string(message.fetchResHeaderLine[i]);
                if (message.fetchResBody != null && message.hasOwnProperty("fetchResBody"))
                    writer.uint32(/* id 103, wireType 2 =*/826).bytes(message.fetchResBody);
                if (message.readFileSyncFilename != null && message.hasOwnProperty("readFileSyncFilename"))
                    writer.uint32(/* id 110, wireType 2 =*/882).string(message.readFileSyncFilename);
                if (message.readFileSyncData != null && message.hasOwnProperty("readFileSyncData"))
                    writer.uint32(/* id 120, wireType 2 =*/962).bytes(message.readFileSyncData);
                if (message.writeFileSyncFilename != null && message.hasOwnProperty("writeFileSyncFilename"))
                    writer.uint32(/* id 130, wireType 2 =*/1042).string(message.writeFileSyncFilename);
                if (message.writeFileSyncData != null && message.hasOwnProperty("writeFileSyncData"))
                    writer.uint32(/* id 131, wireType 2 =*/1050).bytes(message.writeFileSyncData);
                if (message.writeFileSyncPerm != null && message.hasOwnProperty("writeFileSyncPerm"))
                    writer.uint32(/* id 132, wireType 0 =*/1056).uint32(message.writeFileSyncPerm);
                return writer;
            };
    
            /**
             * Encodes the specified Msg message, length delimited. Does not implicitly {@link deno.Msg.verify|verify} messages.
             * @function encodeDelimited
             * @memberof deno.Msg
             * @static
             * @param {deno.IMsg} message Msg message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Msg.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a Msg message from the specified reader or buffer.
             * @function decode
             * @memberof deno.Msg
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {deno.Msg} Msg
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Msg.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.deno.Msg();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.command = reader.int32();
                        break;
                    case 2:
                        message.error = reader.string();
                        break;
                    case 10:
                        message.startCwd = reader.string();
                        break;
                    case 11:
                        if (!(message.startArgv && message.startArgv.length))
                            message.startArgv = [];
                        message.startArgv.push(reader.string());
                        break;
                    case 12:
                        message.startDebugFlag = reader.bool();
                        break;
                    case 13:
                        message.startMainJs = reader.string();
                        break;
                    case 14:
                        message.startMainMap = reader.string();
                        break;
                    case 20:
                        message.codeFetchModuleSpecifier = reader.string();
                        break;
                    case 21:
                        message.codeFetchContainingFile = reader.string();
                        break;
                    case 30:
                        message.codeFetchResModuleName = reader.string();
                        break;
                    case 31:
                        message.codeFetchResFilename = reader.string();
                        break;
                    case 32:
                        message.codeFetchResSourceCode = reader.string();
                        break;
                    case 33:
                        message.codeFetchResOutputCode = reader.string();
                        break;
                    case 41:
                        message.codeCacheFilename = reader.string();
                        break;
                    case 42:
                        message.codeCacheSourceCode = reader.string();
                        break;
                    case 43:
                        message.codeCacheOutputCode = reader.string();
                        break;
                    case 50:
                        message.exitCode = reader.int32();
                        break;
                    case 60:
                        message.timerStartId = reader.int32();
                        break;
                    case 61:
                        message.timerStartInterval = reader.bool();
                        break;
                    case 62:
                        message.timerStartDelay = reader.int32();
                        break;
                    case 70:
                        message.timerReadyId = reader.int32();
                        break;
                    case 71:
                        message.timerReadyDone = reader.bool();
                        break;
                    case 80:
                        message.timerClearId = reader.int32();
                        break;
                    case 90:
                        message.fetchReqId = reader.int32();
                        break;
                    case 91:
                        message.fetchReqUrl = reader.string();
                        break;
                    case 100:
                        message.fetchResId = reader.int32();
                        break;
                    case 101:
                        message.fetchResStatus = reader.int32();
                        break;
                    case 102:
                        if (!(message.fetchResHeaderLine && message.fetchResHeaderLine.length))
                            message.fetchResHeaderLine = [];
                        message.fetchResHeaderLine.push(reader.string());
                        break;
                    case 103:
                        message.fetchResBody = reader.bytes();
                        break;
                    case 110:
                        message.readFileSyncFilename = reader.string();
                        break;
                    case 120:
                        message.readFileSyncData = reader.bytes();
                        break;
                    case 130:
                        message.writeFileSyncFilename = reader.string();
                        break;
                    case 131:
                        message.writeFileSyncData = reader.bytes();
                        break;
                    case 132:
                        message.writeFileSyncPerm = reader.uint32();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a Msg message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof deno.Msg
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {deno.Msg} Msg
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Msg.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a Msg message.
             * @function verify
             * @memberof deno.Msg
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            Msg.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.command != null && message.hasOwnProperty("command"))
                    switch (message.command) {
                    default:
                        return "command: enum value expected";
                    case 0:
                    case 1:
                    case 2:
                    case 3:
                    case 4:
                    case 5:
                    case 6:
                    case 7:
                    case 8:
                    case 9:
                    case 10:
                    case 11:
                    case 12:
                    case 13:
                        break;
                    }
                if (message.error != null && message.hasOwnProperty("error"))
                    if (!$util.isString(message.error))
                        return "error: string expected";
                if (message.startCwd != null && message.hasOwnProperty("startCwd"))
                    if (!$util.isString(message.startCwd))
                        return "startCwd: string expected";
                if (message.startArgv != null && message.hasOwnProperty("startArgv")) {
                    if (!Array.isArray(message.startArgv))
                        return "startArgv: array expected";
                    for (var i = 0; i < message.startArgv.length; ++i)
                        if (!$util.isString(message.startArgv[i]))
                            return "startArgv: string[] expected";
                }
                if (message.startDebugFlag != null && message.hasOwnProperty("startDebugFlag"))
                    if (typeof message.startDebugFlag !== "boolean")
                        return "startDebugFlag: boolean expected";
                if (message.startMainJs != null && message.hasOwnProperty("startMainJs"))
                    if (!$util.isString(message.startMainJs))
                        return "startMainJs: string expected";
                if (message.startMainMap != null && message.hasOwnProperty("startMainMap"))
                    if (!$util.isString(message.startMainMap))
                        return "startMainMap: string expected";
                if (message.codeFetchModuleSpecifier != null && message.hasOwnProperty("codeFetchModuleSpecifier"))
                    if (!$util.isString(message.codeFetchModuleSpecifier))
                        return "codeFetchModuleSpecifier: string expected";
                if (message.codeFetchContainingFile != null && message.hasOwnProperty("codeFetchContainingFile"))
                    if (!$util.isString(message.codeFetchContainingFile))
                        return "codeFetchContainingFile: string expected";
                if (message.codeFetchResModuleName != null && message.hasOwnProperty("codeFetchResModuleName"))
                    if (!$util.isString(message.codeFetchResModuleName))
                        return "codeFetchResModuleName: string expected";
                if (message.codeFetchResFilename != null && message.hasOwnProperty("codeFetchResFilename"))
                    if (!$util.isString(message.codeFetchResFilename))
                        return "codeFetchResFilename: string expected";
                if (message.codeFetchResSourceCode != null && message.hasOwnProperty("codeFetchResSourceCode"))
                    if (!$util.isString(message.codeFetchResSourceCode))
                        return "codeFetchResSourceCode: string expected";
                if (message.codeFetchResOutputCode != null && message.hasOwnProperty("codeFetchResOutputCode"))
                    if (!$util.isString(message.codeFetchResOutputCode))
                        return "codeFetchResOutputCode: string expected";
                if (message.codeCacheFilename != null && message.hasOwnProperty("codeCacheFilename"))
                    if (!$util.isString(message.codeCacheFilename))
                        return "codeCacheFilename: string expected";
                if (message.codeCacheSourceCode != null && message.hasOwnProperty("codeCacheSourceCode"))
                    if (!$util.isString(message.codeCacheSourceCode))
                        return "codeCacheSourceCode: string expected";
                if (message.codeCacheOutputCode != null && message.hasOwnProperty("codeCacheOutputCode"))
                    if (!$util.isString(message.codeCacheOutputCode))
                        return "codeCacheOutputCode: string expected";
                if (message.exitCode != null && message.hasOwnProperty("exitCode"))
                    if (!$util.isInteger(message.exitCode))
                        return "exitCode: integer expected";
                if (message.timerStartId != null && message.hasOwnProperty("timerStartId"))
                    if (!$util.isInteger(message.timerStartId))
                        return "timerStartId: integer expected";
                if (message.timerStartInterval != null && message.hasOwnProperty("timerStartInterval"))
                    if (typeof message.timerStartInterval !== "boolean")
                        return "timerStartInterval: boolean expected";
                if (message.timerStartDelay != null && message.hasOwnProperty("timerStartDelay"))
                    if (!$util.isInteger(message.timerStartDelay))
                        return "timerStartDelay: integer expected";
                if (message.timerReadyId != null && message.hasOwnProperty("timerReadyId"))
                    if (!$util.isInteger(message.timerReadyId))
                        return "timerReadyId: integer expected";
                if (message.timerReadyDone != null && message.hasOwnProperty("timerReadyDone"))
                    if (typeof message.timerReadyDone !== "boolean")
                        return "timerReadyDone: boolean expected";
                if (message.timerClearId != null && message.hasOwnProperty("timerClearId"))
                    if (!$util.isInteger(message.timerClearId))
                        return "timerClearId: integer expected";
                if (message.fetchReqId != null && message.hasOwnProperty("fetchReqId"))
                    if (!$util.isInteger(message.fetchReqId))
                        return "fetchReqId: integer expected";
                if (message.fetchReqUrl != null && message.hasOwnProperty("fetchReqUrl"))
                    if (!$util.isString(message.fetchReqUrl))
                        return "fetchReqUrl: string expected";
                if (message.fetchResId != null && message.hasOwnProperty("fetchResId"))
                    if (!$util.isInteger(message.fetchResId))
                        return "fetchResId: integer expected";
                if (message.fetchResStatus != null && message.hasOwnProperty("fetchResStatus"))
                    if (!$util.isInteger(message.fetchResStatus))
                        return "fetchResStatus: integer expected";
                if (message.fetchResHeaderLine != null && message.hasOwnProperty("fetchResHeaderLine")) {
                    if (!Array.isArray(message.fetchResHeaderLine))
                        return "fetchResHeaderLine: array expected";
                    for (var i = 0; i < message.fetchResHeaderLine.length; ++i)
                        if (!$util.isString(message.fetchResHeaderLine[i]))
                            return "fetchResHeaderLine: string[] expected";
                }
                if (message.fetchResBody != null && message.hasOwnProperty("fetchResBody"))
                    if (!(message.fetchResBody && typeof message.fetchResBody.length === "number" || $util.isString(message.fetchResBody)))
                        return "fetchResBody: buffer expected";
                if (message.readFileSyncFilename != null && message.hasOwnProperty("readFileSyncFilename"))
                    if (!$util.isString(message.readFileSyncFilename))
                        return "readFileSyncFilename: string expected";
                if (message.readFileSyncData != null && message.hasOwnProperty("readFileSyncData"))
                    if (!(message.readFileSyncData && typeof message.readFileSyncData.length === "number" || $util.isString(message.readFileSyncData)))
                        return "readFileSyncData: buffer expected";
                if (message.writeFileSyncFilename != null && message.hasOwnProperty("writeFileSyncFilename"))
                    if (!$util.isString(message.writeFileSyncFilename))
                        return "writeFileSyncFilename: string expected";
                if (message.writeFileSyncData != null && message.hasOwnProperty("writeFileSyncData"))
                    if (!(message.writeFileSyncData && typeof message.writeFileSyncData.length === "number" || $util.isString(message.writeFileSyncData)))
                        return "writeFileSyncData: buffer expected";
                if (message.writeFileSyncPerm != null && message.hasOwnProperty("writeFileSyncPerm"))
                    if (!$util.isInteger(message.writeFileSyncPerm))
                        return "writeFileSyncPerm: integer expected";
                return null;
            };
    
            /**
             * Creates a Msg message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof deno.Msg
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {deno.Msg} Msg
             */
            Msg.fromObject = function fromObject(object) {
                if (object instanceof $root.deno.Msg)
                    return object;
                var message = new $root.deno.Msg();
                switch (object.command) {
                case "ERROR":
                case 0:
                    message.command = 0;
                    break;
                case "START":
                case 1:
                    message.command = 1;
                    break;
                case "CODE_FETCH":
                case 2:
                    message.command = 2;
                    break;
                case "CODE_FETCH_RES":
                case 3:
                    message.command = 3;
                    break;
                case "CODE_CACHE":
                case 4:
                    message.command = 4;
                    break;
                case "EXIT":
                case 5:
                    message.command = 5;
                    break;
                case "TIMER_START":
                case 6:
                    message.command = 6;
                    break;
                case "TIMER_READY":
                case 7:
                    message.command = 7;
                    break;
                case "TIMER_CLEAR":
                case 8:
                    message.command = 8;
                    break;
                case "FETCH_REQ":
                case 9:
                    message.command = 9;
                    break;
                case "FETCH_RES":
                case 10:
                    message.command = 10;
                    break;
                case "READ_FILE_SYNC":
                case 11:
                    message.command = 11;
                    break;
                case "READ_FILE_SYNC_RES":
                case 12:
                    message.command = 12;
                    break;
                case "WRITE_FILE_SYNC":
                case 13:
                    message.command = 13;
                    break;
                }
                if (object.error != null)
                    message.error = String(object.error);
                if (object.startCwd != null)
                    message.startCwd = String(object.startCwd);
                if (object.startArgv) {
                    if (!Array.isArray(object.startArgv))
                        throw TypeError(".deno.Msg.startArgv: array expected");
                    message.startArgv = [];
                    for (var i = 0; i < object.startArgv.length; ++i)
                        message.startArgv[i] = String(object.startArgv[i]);
                }
                if (object.startDebugFlag != null)
                    message.startDebugFlag = Boolean(object.startDebugFlag);
                if (object.startMainJs != null)
                    message.startMainJs = String(object.startMainJs);
                if (object.startMainMap != null)
                    message.startMainMap = String(object.startMainMap);
                if (object.codeFetchModuleSpecifier != null)
                    message.codeFetchModuleSpecifier = String(object.codeFetchModuleSpecifier);
                if (object.codeFetchContainingFile != null)
                    message.codeFetchContainingFile = String(object.codeFetchContainingFile);
                if (object.codeFetchResModuleName != null)
                    message.codeFetchResModuleName = String(object.codeFetchResModuleName);
                if (object.codeFetchResFilename != null)
                    message.codeFetchResFilename = String(object.codeFetchResFilename);
                if (object.codeFetchResSourceCode != null)
                    message.codeFetchResSourceCode = String(object.codeFetchResSourceCode);
                if (object.codeFetchResOutputCode != null)
                    message.codeFetchResOutputCode = String(object.codeFetchResOutputCode);
                if (object.codeCacheFilename != null)
                    message.codeCacheFilename = String(object.codeCacheFilename);
                if (object.codeCacheSourceCode != null)
                    message.codeCacheSourceCode = String(object.codeCacheSourceCode);
                if (object.codeCacheOutputCode != null)
                    message.codeCacheOutputCode = String(object.codeCacheOutputCode);
                if (object.exitCode != null)
                    message.exitCode = object.exitCode | 0;
                if (object.timerStartId != null)
                    message.timerStartId = object.timerStartId | 0;
                if (object.timerStartInterval != null)
                    message.timerStartInterval = Boolean(object.timerStartInterval);
                if (object.timerStartDelay != null)
                    message.timerStartDelay = object.timerStartDelay | 0;
                if (object.timerReadyId != null)
                    message.timerReadyId = object.timerReadyId | 0;
                if (object.timerReadyDone != null)
                    message.timerReadyDone = Boolean(object.timerReadyDone);
                if (object.timerClearId != null)
                    message.timerClearId = object.timerClearId | 0;
                if (object.fetchReqId != null)
                    message.fetchReqId = object.fetchReqId | 0;
                if (object.fetchReqUrl != null)
                    message.fetchReqUrl = String(object.fetchReqUrl);
                if (object.fetchResId != null)
                    message.fetchResId = object.fetchResId | 0;
                if (object.fetchResStatus != null)
                    message.fetchResStatus = object.fetchResStatus | 0;
                if (object.fetchResHeaderLine) {
                    if (!Array.isArray(object.fetchResHeaderLine))
                        throw TypeError(".deno.Msg.fetchResHeaderLine: array expected");
                    message.fetchResHeaderLine = [];
                    for (var i = 0; i < object.fetchResHeaderLine.length; ++i)
                        message.fetchResHeaderLine[i] = String(object.fetchResHeaderLine[i]);
                }
                if (object.fetchResBody != null)
                    if (typeof object.fetchResBody === "string")
                        $util.base64.decode(object.fetchResBody, message.fetchResBody = $util.newBuffer($util.base64.length(object.fetchResBody)), 0);
                    else if (object.fetchResBody.length)
                        message.fetchResBody = object.fetchResBody;
                if (object.readFileSyncFilename != null)
                    message.readFileSyncFilename = String(object.readFileSyncFilename);
                if (object.readFileSyncData != null)
                    if (typeof object.readFileSyncData === "string")
                        $util.base64.decode(object.readFileSyncData, message.readFileSyncData = $util.newBuffer($util.base64.length(object.readFileSyncData)), 0);
                    else if (object.readFileSyncData.length)
                        message.readFileSyncData = object.readFileSyncData;
                if (object.writeFileSyncFilename != null)
                    message.writeFileSyncFilename = String(object.writeFileSyncFilename);
                if (object.writeFileSyncData != null)
                    if (typeof object.writeFileSyncData === "string")
                        $util.base64.decode(object.writeFileSyncData, message.writeFileSyncData = $util.newBuffer($util.base64.length(object.writeFileSyncData)), 0);
                    else if (object.writeFileSyncData.length)
                        message.writeFileSyncData = object.writeFileSyncData;
                if (object.writeFileSyncPerm != null)
                    message.writeFileSyncPerm = object.writeFileSyncPerm >>> 0;
                return message;
            };
    
            /**
             * Creates a plain object from a Msg message. Also converts values to other types if specified.
             * @function toObject
             * @memberof deno.Msg
             * @static
             * @param {deno.Msg} message Msg
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            Msg.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.arrays || options.defaults) {
                    object.startArgv = [];
                    object.fetchResHeaderLine = [];
                }
                if (options.defaults) {
                    object.command = options.enums === String ? "ERROR" : 0;
                    object.error = "";
                    object.startCwd = "";
                    object.startDebugFlag = false;
                    object.startMainJs = "";
                    object.startMainMap = "";
                    object.codeFetchModuleSpecifier = "";
                    object.codeFetchContainingFile = "";
                    object.codeFetchResModuleName = "";
                    object.codeFetchResFilename = "";
                    object.codeFetchResSourceCode = "";
                    object.codeFetchResOutputCode = "";
                    object.codeCacheFilename = "";
                    object.codeCacheSourceCode = "";
                    object.codeCacheOutputCode = "";
                    object.exitCode = 0;
                    object.timerStartId = 0;
                    object.timerStartInterval = false;
                    object.timerStartDelay = 0;
                    object.timerReadyId = 0;
                    object.timerReadyDone = false;
                    object.timerClearId = 0;
                    object.fetchReqId = 0;
                    object.fetchReqUrl = "";
                    object.fetchResId = 0;
                    object.fetchResStatus = 0;
                    object.fetchResBody = options.bytes === String ? "" : [];
                    object.readFileSyncFilename = "";
                    object.readFileSyncData = options.bytes === String ? "" : [];
                    object.writeFileSyncFilename = "";
                    object.writeFileSyncData = options.bytes === String ? "" : [];
                    object.writeFileSyncPerm = 0;
                }
                if (message.command != null && message.hasOwnProperty("command"))
                    object.command = options.enums === String ? $root.deno.Msg.Command[message.command] : message.command;
                if (message.error != null && message.hasOwnProperty("error"))
                    object.error = message.error;
                if (message.startCwd != null && message.hasOwnProperty("startCwd"))
                    object.startCwd = message.startCwd;
                if (message.startArgv && message.startArgv.length) {
                    object.startArgv = [];
                    for (var j = 0; j < message.startArgv.length; ++j)
                        object.startArgv[j] = message.startArgv[j];
                }
                if (message.startDebugFlag != null && message.hasOwnProperty("startDebugFlag"))
                    object.startDebugFlag = message.startDebugFlag;
                if (message.startMainJs != null && message.hasOwnProperty("startMainJs"))
                    object.startMainJs = message.startMainJs;
                if (message.startMainMap != null && message.hasOwnProperty("startMainMap"))
                    object.startMainMap = message.startMainMap;
                if (message.codeFetchModuleSpecifier != null && message.hasOwnProperty("codeFetchModuleSpecifier"))
                    object.codeFetchModuleSpecifier = message.codeFetchModuleSpecifier;
                if (message.codeFetchContainingFile != null && message.hasOwnProperty("codeFetchContainingFile"))
                    object.codeFetchContainingFile = message.codeFetchContainingFile;
                if (message.codeFetchResModuleName != null && message.hasOwnProperty("codeFetchResModuleName"))
                    object.codeFetchResModuleName = message.codeFetchResModuleName;
                if (message.codeFetchResFilename != null && message.hasOwnProperty("codeFetchResFilename"))
                    object.codeFetchResFilename = message.codeFetchResFilename;
                if (message.codeFetchResSourceCode != null && message.hasOwnProperty("codeFetchResSourceCode"))
                    object.codeFetchResSourceCode = message.codeFetchResSourceCode;
                if (message.codeFetchResOutputCode != null && message.hasOwnProperty("codeFetchResOutputCode"))
                    object.codeFetchResOutputCode = message.codeFetchResOutputCode;
                if (message.codeCacheFilename != null && message.hasOwnProperty("codeCacheFilename"))
                    object.codeCacheFilename = message.codeCacheFilename;
                if (message.codeCacheSourceCode != null && message.hasOwnProperty("codeCacheSourceCode"))
                    object.codeCacheSourceCode = message.codeCacheSourceCode;
                if (message.codeCacheOutputCode != null && message.hasOwnProperty("codeCacheOutputCode"))
                    object.codeCacheOutputCode = message.codeCacheOutputCode;
                if (message.exitCode != null && message.hasOwnProperty("exitCode"))
                    object.exitCode = message.exitCode;
                if (message.timerStartId != null && message.hasOwnProperty("timerStartId"))
                    object.timerStartId = message.timerStartId;
                if (message.timerStartInterval != null && message.hasOwnProperty("timerStartInterval"))
                    object.timerStartInterval = message.timerStartInterval;
                if (message.timerStartDelay != null && message.hasOwnProperty("timerStartDelay"))
                    object.timerStartDelay = message.timerStartDelay;
                if (message.timerReadyId != null && message.hasOwnProperty("timerReadyId"))
                    object.timerReadyId = message.timerReadyId;
                if (message.timerReadyDone != null && message.hasOwnProperty("timerReadyDone"))
                    object.timerReadyDone = message.timerReadyDone;
                if (message.timerClearId != null && message.hasOwnProperty("timerClearId"))
                    object.timerClearId = message.timerClearId;
                if (message.fetchReqId != null && message.hasOwnProperty("fetchReqId"))
                    object.fetchReqId = message.fetchReqId;
                if (message.fetchReqUrl != null && message.hasOwnProperty("fetchReqUrl"))
                    object.fetchReqUrl = message.fetchReqUrl;
                if (message.fetchResId != null && message.hasOwnProperty("fetchResId"))
                    object.fetchResId = message.fetchResId;
                if (message.fetchResStatus != null && message.hasOwnProperty("fetchResStatus"))
                    object.fetchResStatus = message.fetchResStatus;
                if (message.fetchResHeaderLine && message.fetchResHeaderLine.length) {
                    object.fetchResHeaderLine = [];
                    for (var j = 0; j < message.fetchResHeaderLine.length; ++j)
                        object.fetchResHeaderLine[j] = message.fetchResHeaderLine[j];
                }
                if (message.fetchResBody != null && message.hasOwnProperty("fetchResBody"))
                    object.fetchResBody = options.bytes === String ? $util.base64.encode(message.fetchResBody, 0, message.fetchResBody.length) : options.bytes === Array ? Array.prototype.slice.call(message.fetchResBody) : message.fetchResBody;
                if (message.readFileSyncFilename != null && message.hasOwnProperty("readFileSyncFilename"))
                    object.readFileSyncFilename = message.readFileSyncFilename;
                if (message.readFileSyncData != null && message.hasOwnProperty("readFileSyncData"))
                    object.readFileSyncData = options.bytes === String ? $util.base64.encode(message.readFileSyncData, 0, message.readFileSyncData.length) : options.bytes === Array ? Array.prototype.slice.call(message.readFileSyncData) : message.readFileSyncData;
                if (message.writeFileSyncFilename != null && message.hasOwnProperty("writeFileSyncFilename"))
                    object.writeFileSyncFilename = message.writeFileSyncFilename;
                if (message.writeFileSyncData != null && message.hasOwnProperty("writeFileSyncData"))
                    object.writeFileSyncData = options.bytes === String ? $util.base64.encode(message.writeFileSyncData, 0, message.writeFileSyncData.length) : options.bytes === Array ? Array.prototype.slice.call(message.writeFileSyncData) : message.writeFileSyncData;
                if (message.writeFileSyncPerm != null && message.hasOwnProperty("writeFileSyncPerm"))
                    object.writeFileSyncPerm = message.writeFileSyncPerm;
                return object;
            };
    
            /**
             * Converts this Msg to JSON.
             * @function toJSON
             * @memberof deno.Msg
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            Msg.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            /**
             * Command enum.
             * @name deno.Msg.Command
             * @enum {string}
             * @property {number} ERROR=0 ERROR value
             * @property {number} START=1 START value
             * @property {number} CODE_FETCH=2 CODE_FETCH value
             * @property {number} CODE_FETCH_RES=3 CODE_FETCH_RES value
             * @property {number} CODE_CACHE=4 CODE_CACHE value
             * @property {number} EXIT=5 EXIT value
             * @property {number} TIMER_START=6 TIMER_START value
             * @property {number} TIMER_READY=7 TIMER_READY value
             * @property {number} TIMER_CLEAR=8 TIMER_CLEAR value
             * @property {number} FETCH_REQ=9 FETCH_REQ value
             * @property {number} FETCH_RES=10 FETCH_RES value
             * @property {number} READ_FILE_SYNC=11 READ_FILE_SYNC value
             * @property {number} READ_FILE_SYNC_RES=12 READ_FILE_SYNC_RES value
             * @property {number} WRITE_FILE_SYNC=13 WRITE_FILE_SYNC value
             */
            Msg.Command = (function() {
                var valuesById = {}, values = Object.create(valuesById);
                values[valuesById[0] = "ERROR"] = 0;
                values[valuesById[1] = "START"] = 1;
                values[valuesById[2] = "CODE_FETCH"] = 2;
                values[valuesById[3] = "CODE_FETCH_RES"] = 3;
                values[valuesById[4] = "CODE_CACHE"] = 4;
                values[valuesById[5] = "EXIT"] = 5;
                values[valuesById[6] = "TIMER_START"] = 6;
                values[valuesById[7] = "TIMER_READY"] = 7;
                values[valuesById[8] = "TIMER_CLEAR"] = 8;
                values[valuesById[9] = "FETCH_REQ"] = 9;
                values[valuesById[10] = "FETCH_RES"] = 10;
                values[valuesById[11] = "READ_FILE_SYNC"] = 11;
                values[valuesById[12] = "READ_FILE_SYNC_RES"] = 12;
                values[valuesById[13] = "WRITE_FILE_SYNC"] = 13;
                return values;
            })();
    
            return Msg;
        })();
    
        return deno;
    })();

    return $root;
});
