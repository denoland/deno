import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import {assert} from "./util";
import {sendAsync} from "./dispatch";

function req(
    algorithm: string,
    data: Uint8Array
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
    const builder = flatbuffers.createBuilder();
    const algoLoc = builder.createString(algorithm);
    const dataLoc = msg.WebCryptoDigest.createDataVector(builder, data);
    msg.WebCryptoDigest.startWebCryptoDigest(builder);
    msg.WebCryptoDigest.addAlgorithm(builder, algoLoc);
    msg.WebCryptoDigest.addData(builder, dataLoc);
    const inner = msg.WebCryptoDigest.endWebCryptoDigest(builder);
    return [builder, msg.Any.WebCryptoDigest, inner];
}

function res(
    baseRes: null | msg.Base
): ArrayBuffer | null {
    assert(baseRes !== null);
    assert(msg.Any.WebCryptoDigestRes === baseRes!.innerType());
    const res = new msg.WebCryptoDigestRes();
    assert(baseRes!.inner(res) !== null);
    const result = res.resultArray();
    if (result !== null) {
        return result;
    }
    return null;
}

const kHashFuncs = ["SHA-1", "SHA-256", "SHA-384", "SHA-512"]
type BinaryArray = Int8Array | Int16Array | Int32Array | Uint8Array | Uint16Array | Uint32Array | Uint8ClampedArray | Float32Array | Float64Array | DataView | ArrayBuffer
function binaryArrayToBytes(bin: BinaryArray): Uint8Array {
    const buf = new ArrayBuffer(bin.byteLength);
    let view = new DataView(buf);
    if (bin instanceof Int8Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setInt8(i, bin[i]);
        }
    } else if (bin instanceof Int16Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setInt16(i*2, bin[i]);
        }
    } else if (bin instanceof Int32Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setInt32(i*4, bin[i]);
        }
    } else if (bin instanceof Uint8Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setUint8(i, bin[i]);
        }
    } else if (bin instanceof Uint16Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setUint16(i*2, bin[i]);
        }
    } else if (bin instanceof Uint32Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setUint32(i*4, bin[i]);
        }
    } else if (bin instanceof Uint8ClampedArray) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setUint8(i, bin[i]);
        }
    } else if (bin instanceof Float32Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setFloat32(i*4, bin[i]);
        }
    } else if (bin instanceof Float64Array) {
        for (let i = 0; i< bin.byteLength; i++) {
            view.setFloat64(i*8, bin[i]);
        }
    } else if (bin instanceof ArrayBuffer) {
        view = new DataView(bin);
    } else if (bin instanceof DataView) {
        view = bin;
    }
    const ret = new Uint8Array(view.byteLength);
    for (let i = 0; i < view.byteLength; i++) {
        ret[i] = view.getUint8(i);
    }
    return ret;
}
async function digest(
    algorithm: string,
    bin: BinaryArray
) {
    if (kHashFuncs.indexOf(algorithm) < 0) {
        throw new Error(`Unsupported hash function: ${algorithm}`);
    }
    const data = binaryArrayToBytes(bin);
    return res(await sendAsync(...req(algorithm, data)));
}

export class SubtleCrypto {
    digest(algorithm: string | { name: string }, data: BinaryArray) {
        if (typeof algorithm === "string") {
            return digest(algorithm, data);
        } else {
            return digest(algorithm.name, data);
        }
    }
}

export const crypto = {
    subtle: new SubtleCrypto()
};
