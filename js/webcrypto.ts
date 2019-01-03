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
): Uint8Array | null {
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

async function digest(
    algorithm: string,
    data: Uint8Array
) {
    return res(await sendAsync(...req(algorithm, data)));
}

export class SubtleCrypto {
    digest(algorithm: string | { name: string }, data: Uint8Array) {
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
