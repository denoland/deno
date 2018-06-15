import * as $protobuf from "protobufjs";

/** Namespace deno. */
export namespace deno {

    /** Properties of a BaseMsg. */
    interface IBaseMsg {

        /** BaseMsg channel */
        channel?: (string|null);

        /** BaseMsg payload */
        payload?: (Uint8Array|null);
    }

    /** Represents a BaseMsg. */
    class BaseMsg implements IBaseMsg {

        /**
         * Constructs a new BaseMsg.
         * @param [properties] Properties to set
         */
        constructor(properties?: deno.IBaseMsg);

        /** BaseMsg channel. */
        public channel: string;

        /** BaseMsg payload. */
        public payload: Uint8Array;

        /**
         * Creates a new BaseMsg instance using the specified properties.
         * @param [properties] Properties to set
         * @returns BaseMsg instance
         */
        public static create(properties?: deno.IBaseMsg): deno.BaseMsg;

        /**
         * Encodes the specified BaseMsg message. Does not implicitly {@link deno.BaseMsg.verify|verify} messages.
         * @param message BaseMsg message or plain object to encode
         * @param [writer] Writer to encode to
         * @returns Writer
         */
        public static encode(message: deno.IBaseMsg, writer?: $protobuf.Writer): $protobuf.Writer;

        /**
         * Encodes the specified BaseMsg message, length delimited. Does not implicitly {@link deno.BaseMsg.verify|verify} messages.
         * @param message BaseMsg message or plain object to encode
         * @param [writer] Writer to encode to
         * @returns Writer
         */
        public static encodeDelimited(message: deno.IBaseMsg, writer?: $protobuf.Writer): $protobuf.Writer;

        /**
         * Decodes a BaseMsg message from the specified reader or buffer.
         * @param reader Reader or buffer to decode from
         * @param [length] Message length if known beforehand
         * @returns BaseMsg
         * @throws {Error} If the payload is not a reader or valid buffer
         * @throws {$protobuf.util.ProtocolError} If required fields are missing
         */
        public static decode(reader: ($protobuf.Reader|Uint8Array), length?: number): deno.BaseMsg;

        /**
         * Decodes a BaseMsg message from the specified reader or buffer, length delimited.
         * @param reader Reader or buffer to decode from
         * @returns BaseMsg
         * @throws {Error} If the payload is not a reader or valid buffer
         * @throws {$protobuf.util.ProtocolError} If required fields are missing
         */
        public static decodeDelimited(reader: ($protobuf.Reader|Uint8Array)): deno.BaseMsg;

        /**
         * Verifies a BaseMsg message.
         * @param message Plain object to verify
         * @returns `null` if valid, otherwise the reason why it is not
         */
        public static verify(message: { [k: string]: any }): (string|null);

        /**
         * Creates a BaseMsg message from a plain object. Also converts values to their respective internal types.
         * @param object Plain object
         * @returns BaseMsg
         */
        public static fromObject(object: { [k: string]: any }): deno.BaseMsg;

        /**
         * Creates a plain object from a BaseMsg message. Also converts values to other types if specified.
         * @param message BaseMsg
         * @param [options] Conversion options
         * @returns Plain object
         */
        public static toObject(message: deno.BaseMsg, options?: $protobuf.IConversionOptions): { [k: string]: any };

        /**
         * Converts this BaseMsg to JSON.
         * @returns JSON object
         */
        public toJSON(): { [k: string]: any };
    }

    /** Properties of a Msg. */
    interface IMsg {

        /** Msg command */
        command?: (deno.Msg.Command|null);

        /** Msg error */
        error?: (string|null);

        /** Msg startCwd */
        startCwd?: (string|null);

        /** Msg startArgv */
        startArgv?: (string[]|null);

        /** Msg startDebugFlag */
        startDebugFlag?: (boolean|null);

        /** Msg startMainJs */
        startMainJs?: (string|null);

        /** Msg startMainMap */
        startMainMap?: (string|null);

        /** Msg codeFetchModuleSpecifier */
        codeFetchModuleSpecifier?: (string|null);

        /** Msg codeFetchContainingFile */
        codeFetchContainingFile?: (string|null);

        /** Msg codeFetchResModuleName */
        codeFetchResModuleName?: (string|null);

        /** Msg codeFetchResFilename */
        codeFetchResFilename?: (string|null);

        /** Msg codeFetchResSourceCode */
        codeFetchResSourceCode?: (string|null);

        /** Msg codeFetchResOutputCode */
        codeFetchResOutputCode?: (string|null);

        /** Msg codeCacheFilename */
        codeCacheFilename?: (string|null);

        /** Msg codeCacheSourceCode */
        codeCacheSourceCode?: (string|null);

        /** Msg codeCacheOutputCode */
        codeCacheOutputCode?: (string|null);

        /** Msg exitCode */
        exitCode?: (number|null);

        /** Msg timerStartId */
        timerStartId?: (number|null);

        /** Msg timerStartInterval */
        timerStartInterval?: (boolean|null);

        /** Msg timerStartDelay */
        timerStartDelay?: (number|null);

        /** Msg timerReadyId */
        timerReadyId?: (number|null);

        /** Msg timerReadyDone */
        timerReadyDone?: (boolean|null);

        /** Msg timerClearId */
        timerClearId?: (number|null);

        /** Msg fetchReqId */
        fetchReqId?: (number|null);

        /** Msg fetchReqUrl */
        fetchReqUrl?: (string|null);

        /** Msg fetchResId */
        fetchResId?: (number|null);

        /** Msg fetchResStatus */
        fetchResStatus?: (number|null);

        /** Msg fetchResHeaderLine */
        fetchResHeaderLine?: (string[]|null);

        /** Msg fetchResBody */
        fetchResBody?: (Uint8Array|null);

        /** Msg readFileSyncFilename */
        readFileSyncFilename?: (string|null);

        /** Msg readFileSyncData */
        readFileSyncData?: (Uint8Array|null);

        /** Msg writeFileSyncFilename */
        writeFileSyncFilename?: (string|null);

        /** Msg writeFileSyncData */
        writeFileSyncData?: (Uint8Array|null);

        /** Msg writeFileSyncPerm */
        writeFileSyncPerm?: (number|null);
    }

    /** Represents a Msg. */
    class Msg implements IMsg {

        /**
         * Constructs a new Msg.
         * @param [properties] Properties to set
         */
        constructor(properties?: deno.IMsg);

        /** Msg command. */
        public command: deno.Msg.Command;

        /** Msg error. */
        public error: string;

        /** Msg startCwd. */
        public startCwd: string;

        /** Msg startArgv. */
        public startArgv: string[];

        /** Msg startDebugFlag. */
        public startDebugFlag: boolean;

        /** Msg startMainJs. */
        public startMainJs: string;

        /** Msg startMainMap. */
        public startMainMap: string;

        /** Msg codeFetchModuleSpecifier. */
        public codeFetchModuleSpecifier: string;

        /** Msg codeFetchContainingFile. */
        public codeFetchContainingFile: string;

        /** Msg codeFetchResModuleName. */
        public codeFetchResModuleName: string;

        /** Msg codeFetchResFilename. */
        public codeFetchResFilename: string;

        /** Msg codeFetchResSourceCode. */
        public codeFetchResSourceCode: string;

        /** Msg codeFetchResOutputCode. */
        public codeFetchResOutputCode: string;

        /** Msg codeCacheFilename. */
        public codeCacheFilename: string;

        /** Msg codeCacheSourceCode. */
        public codeCacheSourceCode: string;

        /** Msg codeCacheOutputCode. */
        public codeCacheOutputCode: string;

        /** Msg exitCode. */
        public exitCode: number;

        /** Msg timerStartId. */
        public timerStartId: number;

        /** Msg timerStartInterval. */
        public timerStartInterval: boolean;

        /** Msg timerStartDelay. */
        public timerStartDelay: number;

        /** Msg timerReadyId. */
        public timerReadyId: number;

        /** Msg timerReadyDone. */
        public timerReadyDone: boolean;

        /** Msg timerClearId. */
        public timerClearId: number;

        /** Msg fetchReqId. */
        public fetchReqId: number;

        /** Msg fetchReqUrl. */
        public fetchReqUrl: string;

        /** Msg fetchResId. */
        public fetchResId: number;

        /** Msg fetchResStatus. */
        public fetchResStatus: number;

        /** Msg fetchResHeaderLine. */
        public fetchResHeaderLine: string[];

        /** Msg fetchResBody. */
        public fetchResBody: Uint8Array;

        /** Msg readFileSyncFilename. */
        public readFileSyncFilename: string;

        /** Msg readFileSyncData. */
        public readFileSyncData: Uint8Array;

        /** Msg writeFileSyncFilename. */
        public writeFileSyncFilename: string;

        /** Msg writeFileSyncData. */
        public writeFileSyncData: Uint8Array;

        /** Msg writeFileSyncPerm. */
        public writeFileSyncPerm: number;

        /**
         * Creates a new Msg instance using the specified properties.
         * @param [properties] Properties to set
         * @returns Msg instance
         */
        public static create(properties?: deno.IMsg): deno.Msg;

        /**
         * Encodes the specified Msg message. Does not implicitly {@link deno.Msg.verify|verify} messages.
         * @param message Msg message or plain object to encode
         * @param [writer] Writer to encode to
         * @returns Writer
         */
        public static encode(message: deno.IMsg, writer?: $protobuf.Writer): $protobuf.Writer;

        /**
         * Encodes the specified Msg message, length delimited. Does not implicitly {@link deno.Msg.verify|verify} messages.
         * @param message Msg message or plain object to encode
         * @param [writer] Writer to encode to
         * @returns Writer
         */
        public static encodeDelimited(message: deno.IMsg, writer?: $protobuf.Writer): $protobuf.Writer;

        /**
         * Decodes a Msg message from the specified reader or buffer.
         * @param reader Reader or buffer to decode from
         * @param [length] Message length if known beforehand
         * @returns Msg
         * @throws {Error} If the payload is not a reader or valid buffer
         * @throws {$protobuf.util.ProtocolError} If required fields are missing
         */
        public static decode(reader: ($protobuf.Reader|Uint8Array), length?: number): deno.Msg;

        /**
         * Decodes a Msg message from the specified reader or buffer, length delimited.
         * @param reader Reader or buffer to decode from
         * @returns Msg
         * @throws {Error} If the payload is not a reader or valid buffer
         * @throws {$protobuf.util.ProtocolError} If required fields are missing
         */
        public static decodeDelimited(reader: ($protobuf.Reader|Uint8Array)): deno.Msg;

        /**
         * Verifies a Msg message.
         * @param message Plain object to verify
         * @returns `null` if valid, otherwise the reason why it is not
         */
        public static verify(message: { [k: string]: any }): (string|null);

        /**
         * Creates a Msg message from a plain object. Also converts values to their respective internal types.
         * @param object Plain object
         * @returns Msg
         */
        public static fromObject(object: { [k: string]: any }): deno.Msg;

        /**
         * Creates a plain object from a Msg message. Also converts values to other types if specified.
         * @param message Msg
         * @param [options] Conversion options
         * @returns Plain object
         */
        public static toObject(message: deno.Msg, options?: $protobuf.IConversionOptions): { [k: string]: any };

        /**
         * Converts this Msg to JSON.
         * @returns JSON object
         */
        public toJSON(): { [k: string]: any };
    }

    namespace Msg {

        /** Command enum. */
        enum Command {
            ERROR = 0,
            START = 1,
            CODE_FETCH = 2,
            CODE_FETCH_RES = 3,
            CODE_CACHE = 4,
            EXIT = 5,
            TIMER_START = 6,
            TIMER_READY = 7,
            TIMER_CLEAR = 8,
            FETCH_REQ = 9,
            FETCH_RES = 10,
            READ_FILE_SYNC = 11,
            READ_FILE_SYNC_RES = 12,
            WRITE_FILE_SYNC = 13
        }
    }
}
