// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-fmt-ignore-file
// deno-lint-ignore-file
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { AbortController } from "ext:deno_web/03_abort_signal.js";
import { Blob } from "ext:deno_web/09_file.js";

/* esm.sh - esbuild bundle(readable-stream@4.2.0) es2022 production */
const __process$ = { nextTick };import __buffer$ from "ext:deno_node/buffer.ts";import __string_decoder$ from "ext:deno_node/string_decoder.ts";import __events$ from "ext:deno_node/events.ts";var pi=Object.create;var Bt=Object.defineProperty;var wi=Object.getOwnPropertyDescriptor;var yi=Object.getOwnPropertyNames;var gi=Object.getPrototypeOf,Si=Object.prototype.hasOwnProperty;var E=(e=>typeof require<"u"?require:typeof Proxy<"u"?new Proxy(e,{get:(t,n)=>(typeof require<"u"?require:t)[n]}):e)(function(e){if(typeof require<"u")return require.apply(this,arguments);throw new Error('Dynamic require of "'+e+'" is not supported')});var g=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports);var Ei=(e,t,n,r)=>{if(t&&typeof t=="object"||typeof t=="function")for(let i of yi(t))!Si.call(e,i)&&i!==n&&Bt(e,i,{get:()=>t[i],enumerable:!(r=wi(t,i))||r.enumerable});return e};var Ri=(e,t,n)=>(n=e!=null?pi(gi(e)):{},Ei(t||!e||!e.__esModule?Bt(n,"default",{value:e,enumerable:!0}):n,e));var m=g((Yf,Gt)=>{"use strict";Gt.exports={ArrayIsArray(e){return Array.isArray(e)},ArrayPrototypeIncludes(e,t){return e.includes(t)},ArrayPrototypeIndexOf(e,t){return e.indexOf(t)},ArrayPrototypeJoin(e,t){return e.join(t)},ArrayPrototypeMap(e,t){return e.map(t)},ArrayPrototypePop(e,t){return e.pop(t)},ArrayPrototypePush(e,t){return e.push(t)},ArrayPrototypeSlice(e,t,n){return e.slice(t,n)},Error,FunctionPrototypeCall(e,t,...n){return e.call(t,...n)},FunctionPrototypeSymbolHasInstance(e,t){return Function.prototype[Symbol.hasInstance].call(e,t)},MathFloor:Math.floor,Number,NumberIsInteger:Number.isInteger,NumberIsNaN:Number.isNaN,NumberMAX_SAFE_INTEGER:Number.MAX_SAFE_INTEGER,NumberMIN_SAFE_INTEGER:Number.MIN_SAFE_INTEGER,NumberParseInt:Number.parseInt,ObjectDefineProperties(e,t){return Object.defineProperties(e,t)},ObjectDefineProperty(e,t,n){return Object.defineProperty(e,t,n)},ObjectGetOwnPropertyDescriptor(e,t){return Object.getOwnPropertyDescriptor(e,t)},ObjectKeys(e){return Object.keys(e)},ObjectSetPrototypeOf(e,t){return Object.setPrototypeOf(e,t)},Promise,PromisePrototypeCatch(e,t){return e.catch(t)},PromisePrototypeThen(e,t,n){return e.then(t,n)},PromiseReject(e){return Promise.reject(e)},ReflectApply:Reflect.apply,RegExpPrototypeTest(e,t){return e.test(t)},SafeSet:Set,String,StringPrototypeSlice(e,t,n){return e.slice(t,n)},StringPrototypeToLowerCase(e){return e.toLowerCase()},StringPrototypeToUpperCase(e){return e.toUpperCase()},StringPrototypeTrim(e){return e.trim()},Symbol,SymbolAsyncIterator:Symbol.asyncIterator,SymbolHasInstance:Symbol.hasInstance,SymbolIterator:Symbol.iterator,TypedArrayPrototypeSet(e,t,n){return e.set(t,n)},Uint8Array}});var j=g((Kf,Je)=>{"use strict";var Ai=__buffer$,mi=Object.getPrototypeOf(async function(){}).constructor,Ht=Blob||Ai.Blob,Ti=typeof Ht<"u"?function(t){return t instanceof Ht}:function(t){return!1},Xe=class extends Error{constructor(t){if(!Array.isArray(t))throw new TypeError(`Expected input to be an Array, got ${typeof t}`);let n="";for(let r=0;r<t.length;r++)n+=`    ${t[r].stack}
`;super(n),this.name="AggregateError",this.errors=t}};Je.exports={AggregateError:Xe,kEmptyObject:Object.freeze({}),once(e){let t=!1;return function(...n){t||(t=!0,e.apply(this,n))}},createDeferredPromise:function(){let e,t;return{promise:new Promise((r,i)=>{e=r,t=i}),resolve:e,reject:t}},promisify(e){return new Promise((t,n)=>{e((r,...i)=>r?n(r):t(...i))})},debuglog(){return function(){}},format(e,...t){return e.replace(/%([sdifj])/g,function(...[n,r]){let i=t.shift();return r==="f"?i.toFixed(6):r==="j"?JSON.stringify(i):r==="s"&&typeof i=="object"?`${i.constructor!==Object?i.constructor.name:""} {}`.trim():i.toString()})},inspect(e){switch(typeof e){case"string":if(e.includes("'"))if(e.includes('"')){if(!e.includes("`")&&!e.includes("${"))return`\`${e}\``}else return`"${e}"`;return`'${e}'`;case"number":return isNaN(e)?"NaN":Object.is(e,-0)?String(e):e;case"bigint":return`${String(e)}n`;case"boolean":case"undefined":return String(e);case"object":return"{}"}},types:{isAsyncFunction(e){return e instanceof mi},isArrayBufferView(e){return ArrayBuffer.isView(e)}},isBlob:Ti};Je.exports.promisify.custom=Symbol.for("nodejs.util.promisify.custom")});var O=g((zf,Kt)=>{"use strict";var{format:Ii,inspect:Re,AggregateError:Mi}=j(),Ni=globalThis.AggregateError||Mi,Di=Symbol("kIsNodeError"),Oi=["string","function","number","object","Function","Object","boolean","bigint","symbol"],qi=/^([A-Z][a-z0-9]*)+$/,xi="__node_internal_",Ae={};function X(e,t){if(!e)throw new Ae.ERR_INTERNAL_ASSERTION(t)}function Vt(e){let t="",n=e.length,r=e[0]==="-"?1:0;for(;n>=r+4;n-=3)t=`_${e.slice(n-3,n)}${t}`;return`${e.slice(0,n)}${t}`}function Li(e,t,n){if(typeof t=="function")return X(t.length<=n.length,`Code: ${e}; The provided arguments length (${n.length}) does not match the required ones (${t.length}).`),t(...n);let r=(t.match(/%[dfijoOs]/g)||[]).length;return X(r===n.length,`Code: ${e}; The provided arguments length (${n.length}) does not match the required ones (${r}).`),n.length===0?t:Ii(t,...n)}function N(e,t,n){n||(n=Error);class r extends n{constructor(...o){super(Li(e,t,o))}toString(){return`${this.name} [${e}]: ${this.message}`}}Object.defineProperties(r.prototype,{name:{value:n.name,writable:!0,enumerable:!1,configurable:!0},toString:{value(){return`${this.name} [${e}]: ${this.message}`},writable:!0,enumerable:!1,configurable:!0}}),r.prototype.code=e,r.prototype[Di]=!0,Ae[e]=r}function Yt(e){let t=xi+e.name;return Object.defineProperty(e,"name",{value:t}),e}function Pi(e,t){if(e&&t&&e!==t){if(Array.isArray(t.errors))return t.errors.push(e),t;let n=new Ni([t,e],t.message);return n.code=t.code,n}return e||t}var Qe=class extends Error{constructor(t="The operation was aborted",n=void 0){if(n!==void 0&&typeof n!="object")throw new Ae.ERR_INVALID_ARG_TYPE("options","Object",n);super(t,n),this.code="ABORT_ERR",this.name="AbortError"}};N("ERR_ASSERTION","%s",Error);N("ERR_INVALID_ARG_TYPE",(e,t,n)=>{X(typeof e=="string","'name' must be a string"),Array.isArray(t)||(t=[t]);let r="The ";e.endsWith(" argument")?r+=`${e} `:r+=`"${e}" ${e.includes(".")?"property":"argument"} `,r+="must be ";let i=[],o=[],l=[];for(let f of t)X(typeof f=="string","All expected entries have to be of type string"),Oi.includes(f)?i.push(f.toLowerCase()):qi.test(f)?o.push(f):(X(f!=="object",'The value "object" should be written as "Object"'),l.push(f));if(o.length>0){let f=i.indexOf("object");f!==-1&&(i.splice(i,f,1),o.push("Object"))}if(i.length>0){switch(i.length){case 1:r+=`of type ${i[0]}`;break;case 2:r+=`one of type ${i[0]} or ${i[1]}`;break;default:{let f=i.pop();r+=`one of type ${i.join(", ")}, or ${f}`}}(o.length>0||l.length>0)&&(r+=" or ")}if(o.length>0){switch(o.length){case 1:r+=`an instance of ${o[0]}`;break;case 2:r+=`an instance of ${o[0]} or ${o[1]}`;break;default:{let f=o.pop();r+=`an instance of ${o.join(", ")}, or ${f}`}}l.length>0&&(r+=" or ")}switch(l.length){case 0:break;case 1:l[0].toLowerCase()!==l[0]&&(r+="an "),r+=`${l[0]}`;break;case 2:r+=`one of ${l[0]} or ${l[1]}`;break;default:{let f=l.pop();r+=`one of ${l.join(", ")}, or ${f}`}}if(n==null)r+=`. Received ${n}`;else if(typeof n=="function"&&n.name)r+=`. Received function ${n.name}`;else if(typeof n=="object"){var u;(u=n.constructor)!==null&&u!==void 0&&u.name?r+=`. Received an instance of ${n.constructor.name}`:r+=`. Received ${Re(n,{depth:-1})}`}else{let f=Re(n,{colors:!1});f.length>25&&(f=`${f.slice(0,25)}...`),r+=`. Received type ${typeof n} (${f})`}return r},TypeError);N("ERR_INVALID_ARG_VALUE",(e,t,n="is invalid")=>{let r=Re(t);return r.length>128&&(r=r.slice(0,128)+"..."),`The ${e.includes(".")?"property":"argument"} '${e}' ${n}. Received ${r}`},TypeError);N("ERR_INVALID_RETURN_VALUE",(e,t,n)=>{var r;let i=n!=null&&(r=n.constructor)!==null&&r!==void 0&&r.name?`instance of ${n.constructor.name}`:`type ${typeof n}`;return`Expected ${e} to be returned from the "${t}" function but got ${i}.`},TypeError);N("ERR_MISSING_ARGS",(...e)=>{X(e.length>0,"At least one arg needs to be specified");let t,n=e.length;switch(e=(Array.isArray(e)?e:[e]).map(r=>`"${r}"`).join(" or "),n){case 1:t+=`The ${e[0]} argument`;break;case 2:t+=`The ${e[0]} and ${e[1]} arguments`;break;default:{let r=e.pop();t+=`The ${e.join(", ")}, and ${r} arguments`}break}return`${t} must be specified`},TypeError);N("ERR_OUT_OF_RANGE",(e,t,n)=>{X(t,'Missing "range" argument');let r;return Number.isInteger(n)&&Math.abs(n)>2**32?r=Vt(String(n)):typeof n=="bigint"?(r=String(n),(n>2n**32n||n<-(2n**32n))&&(r=Vt(r)),r+="n"):r=Re(n),`The value of "${e}" is out of range. It must be ${t}. Received ${r}`},RangeError);N("ERR_MULTIPLE_CALLBACK","Callback called multiple times",Error);N("ERR_METHOD_NOT_IMPLEMENTED","The %s method is not implemented",Error);N("ERR_STREAM_ALREADY_FINISHED","Cannot call %s after a stream was finished",Error);N("ERR_STREAM_CANNOT_PIPE","Cannot pipe, not readable",Error);N("ERR_STREAM_DESTROYED","Cannot call %s after a stream was destroyed",Error);N("ERR_STREAM_NULL_VALUES","May not write null values to stream",TypeError);N("ERR_STREAM_PREMATURE_CLOSE","Premature close",Error);N("ERR_STREAM_PUSH_AFTER_EOF","stream.push() after EOF",Error);N("ERR_STREAM_UNSHIFT_AFTER_END_EVENT","stream.unshift() after end event",Error);N("ERR_STREAM_WRITE_AFTER_END","write after end",Error);N("ERR_UNKNOWN_ENCODING","Unknown encoding: %s",TypeError);Kt.exports={AbortError:Qe,aggregateTwoErrors:Yt(Pi),hideStackFrames:Yt,codes:Ae}});var _e=g((Xf,nn)=>{"use strict";var{ArrayIsArray:Jt,ArrayPrototypeIncludes:Qt,ArrayPrototypeJoin:Zt,ArrayPrototypeMap:ki,NumberIsInteger:et,NumberIsNaN:Wi,NumberMAX_SAFE_INTEGER:Ci,NumberMIN_SAFE_INTEGER:ji,NumberParseInt:$i,ObjectPrototypeHasOwnProperty:vi,RegExpPrototypeExec:Fi,String:Ui,StringPrototypeToUpperCase:Bi,StringPrototypeTrim:Gi}=m(),{hideStackFrames:k,codes:{ERR_SOCKET_BAD_PORT:Hi,ERR_INVALID_ARG_TYPE:q,ERR_INVALID_ARG_VALUE:me,ERR_OUT_OF_RANGE:J,ERR_UNKNOWN_SIGNAL:zt}}=O(),{normalizeEncoding:Vi}=j(),{isAsyncFunction:Yi,isArrayBufferView:Ki}=j().types,Xt={};function zi(e){return e===(e|0)}function Xi(e){return e===e>>>0}var Ji=/^[0-7]+$/,Qi="must be a 32-bit unsigned integer or an octal string";function Zi(e,t,n){if(typeof e>"u"&&(e=n),typeof e=="string"){if(Fi(Ji,e)===null)throw new me(t,e,Qi);e=$i(e,8)}return en(e,t),e}var eo=k((e,t,n=ji,r=Ci)=>{if(typeof e!="number")throw new q(t,"number",e);if(!et(e))throw new J(t,"an integer",e);if(e<n||e>r)throw new J(t,`>= ${n} && <= ${r}`,e)}),to=k((e,t,n=-2147483648,r=2147483647)=>{if(typeof e!="number")throw new q(t,"number",e);if(!et(e))throw new J(t,"an integer",e);if(e<n||e>r)throw new J(t,`>= ${n} && <= ${r}`,e)}),en=k((e,t,n=!1)=>{if(typeof e!="number")throw new q(t,"number",e);if(!et(e))throw new J(t,"an integer",e);let r=n?1:0,i=4294967295;if(e<r||e>i)throw new J(t,`>= ${r} && <= ${i}`,e)});function tn(e,t){if(typeof e!="string")throw new q(t,"string",e)}function no(e,t,n=void 0,r){if(typeof e!="number")throw new q(t,"number",e);if(n!=null&&e<n||r!=null&&e>r||(n!=null||r!=null)&&Wi(e))throw new J(t,`${n!=null?`>= ${n}`:""}${n!=null&&r!=null?" && ":""}${r!=null?`<= ${r}`:""}`,e)}var ro=k((e,t,n)=>{if(!Qt(n,e)){let r=Zt(ki(n,o=>typeof o=="string"?`'${o}'`:Ui(o)),", "),i="must be one of: "+r;throw new me(t,e,i)}});function io(e,t){if(typeof e!="boolean")throw new q(t,"boolean",e)}function Ze(e,t,n){return e==null||!vi(e,t)?n:e[t]}var oo=k((e,t,n=null)=>{let r=Ze(n,"allowArray",!1),i=Ze(n,"allowFunction",!1);if(!Ze(n,"nullable",!1)&&e===null||!r&&Jt(e)||typeof e!="object"&&(!i||typeof e!="function"))throw new q(t,"Object",e)}),lo=k((e,t,n=0)=>{if(!Jt(e))throw new q(t,"Array",e);if(e.length<n){let r=`must be longer than ${n}`;throw new me(t,e,r)}});function ao(e,t="signal"){if(tn(e,t),Xt[e]===void 0)throw Xt[Bi(e)]!==void 0?new zt(e+" (signals must use all capital letters)"):new zt(e)}var fo=k((e,t="buffer")=>{if(!Ki(e))throw new q(t,["Buffer","TypedArray","DataView"],e)});function uo(e,t){let n=Vi(t),r=e.length;if(n==="hex"&&r%2!==0)throw new me("encoding",t,`is invalid for data of length ${r}`)}function so(e,t="Port",n=!0){if(typeof e!="number"&&typeof e!="string"||typeof e=="string"&&Gi(e).length===0||+e!==+e>>>0||e>65535||e===0&&!n)throw new Hi(t,e,n);return e|0}var co=k((e,t)=>{if(e!==void 0&&(e===null||typeof e!="object"||!("aborted"in e)))throw new q(t,"AbortSignal",e)}),ho=k((e,t)=>{if(typeof e!="function")throw new q(t,"Function",e)}),bo=k((e,t)=>{if(typeof e!="function"||Yi(e))throw new q(t,"Function",e)}),_o=k((e,t)=>{if(e!==void 0)throw new q(t,"undefined",e)});function po(e,t,n){if(!Qt(n,e))throw new q(t,`('${Zt(n,"|")}')`,e)}nn.exports={isInt32:zi,isUint32:Xi,parseFileMode:Zi,validateArray:lo,validateBoolean:io,validateBuffer:fo,validateEncoding:uo,validateFunction:ho,validateInt32:to,validateInteger:eo,validateNumber:no,validateObject:oo,validateOneOf:ro,validatePlainFunction:bo,validatePort:so,validateSignalName:ao,validateString:tn,validateUint32:en,validateUndefined:_o,validateUnion:po,validateAbortSignal:co}});var V=g((Jf,_n)=>{"use strict";var{Symbol:Te,SymbolAsyncIterator:rn,SymbolIterator:on}=m(),ln=Te("kDestroyed"),an=Te("kIsErrored"),tt=Te("kIsReadable"),fn=Te("kIsDisturbed");function Ie(e,t=!1){var n;return!!(e&&typeof e.pipe=="function"&&typeof e.on=="function"&&(!t||typeof e.pause=="function"&&typeof e.resume=="function")&&(!e._writableState||((n=e._readableState)===null||n===void 0?void 0:n.readable)!==!1)&&(!e._writableState||e._readableState))}function Me(e){var t;return!!(e&&typeof e.write=="function"&&typeof e.on=="function"&&(!e._readableState||((t=e._writableState)===null||t===void 0?void 0:t.writable)!==!1))}function wo(e){return!!(e&&typeof e.pipe=="function"&&e._readableState&&typeof e.on=="function"&&typeof e.write=="function")}function Q(e){return e&&(e._readableState||e._writableState||typeof e.write=="function"&&typeof e.on=="function"||typeof e.pipe=="function"&&typeof e.on=="function")}function yo(e,t){return e==null?!1:t===!0?typeof e[rn]=="function":t===!1?typeof e[on]=="function":typeof e[rn]=="function"||typeof e[on]=="function"}function Ne(e){if(!Q(e))return null;let t=e._writableState,n=e._readableState,r=t||n;return!!(e.destroyed||e[ln]||r!=null&&r.destroyed)}function un(e){if(!Me(e))return null;if(e.writableEnded===!0)return!0;let t=e._writableState;return t!=null&&t.errored?!1:typeof t?.ended!="boolean"?null:t.ended}function go(e,t){if(!Me(e))return null;if(e.writableFinished===!0)return!0;let n=e._writableState;return n!=null&&n.errored?!1:typeof n?.finished!="boolean"?null:!!(n.finished||t===!1&&n.ended===!0&&n.length===0)}function So(e){if(!Ie(e))return null;if(e.readableEnded===!0)return!0;let t=e._readableState;return!t||t.errored?!1:typeof t?.ended!="boolean"?null:t.ended}function sn(e,t){if(!Ie(e))return null;let n=e._readableState;return n!=null&&n.errored?!1:typeof n?.endEmitted!="boolean"?null:!!(n.endEmitted||t===!1&&n.ended===!0&&n.length===0)}function dn(e){return e&&e[tt]!=null?e[tt]:typeof e?.readable!="boolean"?null:Ne(e)?!1:Ie(e)&&e.readable&&!sn(e)}function cn(e){return typeof e?.writable!="boolean"?null:Ne(e)?!1:Me(e)&&e.writable&&!un(e)}function Eo(e,t){return Q(e)?Ne(e)?!0:!(t?.readable!==!1&&dn(e)||t?.writable!==!1&&cn(e)):null}function Ro(e){var t,n;return Q(e)?e.writableErrored?e.writableErrored:(t=(n=e._writableState)===null||n===void 0?void 0:n.errored)!==null&&t!==void 0?t:null:null}function Ao(e){var t,n;return Q(e)?e.readableErrored?e.readableErrored:(t=(n=e._readableState)===null||n===void 0?void 0:n.errored)!==null&&t!==void 0?t:null:null}function mo(e){if(!Q(e))return null;if(typeof e.closed=="boolean")return e.closed;let t=e._writableState,n=e._readableState;return typeof t?.closed=="boolean"||typeof n?.closed=="boolean"?t?.closed||n?.closed:typeof e._closed=="boolean"&&hn(e)?e._closed:null}function hn(e){return typeof e._closed=="boolean"&&typeof e._defaultKeepAlive=="boolean"&&typeof e._removedConnection=="boolean"&&typeof e._removedContLen=="boolean"}function bn(e){return typeof e._sent100=="boolean"&&hn(e)}function To(e){var t;return typeof e._consuming=="boolean"&&typeof e._dumped=="boolean"&&((t=e.req)===null||t===void 0?void 0:t.upgradeOrConnect)===void 0}function Io(e){if(!Q(e))return null;let t=e._writableState,n=e._readableState,r=t||n;return!r&&bn(e)||!!(r&&r.autoDestroy&&r.emitClose&&r.closed===!1)}function Mo(e){var t;return!!(e&&((t=e[fn])!==null&&t!==void 0?t:e.readableDidRead||e.readableAborted))}function No(e){var t,n,r,i,o,l,u,f,a,c;return!!(e&&((t=(n=(r=(i=(o=(l=e[an])!==null&&l!==void 0?l:e.readableErrored)!==null&&o!==void 0?o:e.writableErrored)!==null&&i!==void 0?i:(u=e._readableState)===null||u===void 0?void 0:u.errorEmitted)!==null&&r!==void 0?r:(f=e._writableState)===null||f===void 0?void 0:f.errorEmitted)!==null&&n!==void 0?n:(a=e._readableState)===null||a===void 0?void 0:a.errored)!==null&&t!==void 0?t:(c=e._writableState)===null||c===void 0?void 0:c.errored))}_n.exports={kDestroyed:ln,isDisturbed:Mo,kIsDisturbed:fn,isErrored:No,kIsErrored:an,isReadable:dn,kIsReadable:tt,isClosed:mo,isDestroyed:Ne,isDuplexNodeStream:wo,isFinished:Eo,isIterable:yo,isReadableNodeStream:Ie,isReadableEnded:So,isReadableFinished:sn,isReadableErrored:Ao,isNodeStream:Q,isWritable:cn,isWritableNodeStream:Me,isWritableEnded:un,isWritableFinished:go,isWritableErrored:Ro,isServerRequest:To,isServerResponse:bn,willEmitClose:Io}});var Y=g((Qf,rt)=>{var oe=__process$,{AbortError:Do,codes:Oo}=O(),{ERR_INVALID_ARG_TYPE:qo,ERR_STREAM_PREMATURE_CLOSE:pn}=Oo,{kEmptyObject:wn,once:yn}=j(),{validateAbortSignal:xo,validateFunction:Lo,validateObject:Po}=_e(),{Promise:ko}=m(),{isClosed:Wo,isReadable:gn,isReadableNodeStream:nt,isReadableFinished:Sn,isReadableErrored:Co,isWritable:En,isWritableNodeStream:Rn,isWritableFinished:An,isWritableErrored:jo,isNodeStream:$o,willEmitClose:vo}=V();function Fo(e){return e.setHeader&&typeof e.abort=="function"}var Uo=()=>{};function mn(e,t,n){var r,i;arguments.length===2?(n=t,t=wn):t==null?t=wn:Po(t,"options"),Lo(n,"callback"),xo(t.signal,"options.signal"),n=yn(n);let o=(r=t.readable)!==null&&r!==void 0?r:nt(e),l=(i=t.writable)!==null&&i!==void 0?i:Rn(e);if(!$o(e))throw new qo("stream","Stream",e);let u=e._writableState,f=e._readableState,a=()=>{e.writable||b()},c=vo(e)&&nt(e)===o&&Rn(e)===l,s=An(e,!1),b=()=>{s=!0,e.destroyed&&(c=!1),!(c&&(!e.readable||o))&&(!o||d)&&n.call(e)},d=Sn(e,!1),h=()=>{d=!0,e.destroyed&&(c=!1),!(c&&(!e.writable||l))&&(!l||s)&&n.call(e)},D=M=>{n.call(e,M)},L=Wo(e),_=()=>{L=!0;let M=jo(e)||Co(e);if(M&&typeof M!="boolean")return n.call(e,M);if(o&&!d&&nt(e,!0)&&!Sn(e,!1))return n.call(e,new pn);if(l&&!s&&!An(e,!1))return n.call(e,new pn);n.call(e)},p=()=>{e.req.on("finish",b)};Fo(e)?(e.on("complete",b),c||e.on("abort",_),e.req?p():e.on("request",p)):l&&!u&&(e.on("end",a),e.on("close",a)),!c&&typeof e.aborted=="boolean"&&e.on("aborted",_),e.on("end",h),e.on("finish",b),t.error!==!1&&e.on("error",D),e.on("close",_),L?oe.nextTick(_):u!=null&&u.errorEmitted||f!=null&&f.errorEmitted?c||oe.nextTick(_):(!o&&(!c||gn(e))&&(s||En(e)===!1)||!l&&(!c||En(e))&&(d||gn(e)===!1)||f&&e.req&&e.aborted)&&oe.nextTick(_);let I=()=>{n=Uo,e.removeListener("aborted",_),e.removeListener("complete",b),e.removeListener("abort",_),e.removeListener("request",p),e.req&&e.req.removeListener("finish",b),e.removeListener("end",a),e.removeListener("close",a),e.removeListener("finish",b),e.removeListener("end",h),e.removeListener("error",D),e.removeListener("close",_)};if(t.signal&&!L){let M=()=>{let F=n;I(),F.call(e,new Do(void 0,{cause:t.signal.reason}))};if(t.signal.aborted)oe.nextTick(M);else{let F=n;n=yn((...re)=>{t.signal.removeEventListener("abort",M),F.apply(e,re)}),t.signal.addEventListener("abort",M)}}return I}function Bo(e,t){return new ko((n,r)=>{mn(e,t,i=>{i?r(i):n()})})}rt.exports=mn;rt.exports.finished=Bo});var xn=g((Zf,lt)=>{"use strict";var Nn=AbortController,{codes:{ERR_INVALID_ARG_TYPE:pe,ERR_MISSING_ARGS:Go,ERR_OUT_OF_RANGE:Ho},AbortError:$}=O(),{validateAbortSignal:le,validateInteger:Vo,validateObject:ae}=_e(),Yo=m().Symbol("kWeak"),{finished:Ko}=Y(),{ArrayPrototypePush:zo,MathFloor:Xo,Number:Jo,NumberIsNaN:Qo,Promise:Tn,PromiseReject:In,PromisePrototypeThen:Zo,Symbol:Dn}=m(),De=Dn("kEmpty"),Mn=Dn("kEof");function Oe(e,t){if(typeof e!="function")throw new pe("fn",["Function","AsyncFunction"],e);t!=null&&ae(t,"options"),t?.signal!=null&&le(t.signal,"options.signal");let n=1;return t?.concurrency!=null&&(n=Xo(t.concurrency)),Vo(n,"concurrency",1),async function*(){var i,o;let l=new Nn,u=this,f=[],a=l.signal,c={signal:a},s=()=>l.abort();t!=null&&(i=t.signal)!==null&&i!==void 0&&i.aborted&&s(),t==null||(o=t.signal)===null||o===void 0||o.addEventListener("abort",s);let b,d,h=!1;function D(){h=!0}async function L(){try{for await(let I of u){var _;if(h)return;if(a.aborted)throw new $;try{I=e(I,c)}catch(M){I=In(M)}I!==De&&(typeof((_=I)===null||_===void 0?void 0:_.catch)=="function"&&I.catch(D),f.push(I),b&&(b(),b=null),!h&&f.length&&f.length>=n&&await new Tn(M=>{d=M}))}f.push(Mn)}catch(I){let M=In(I);Zo(M,void 0,D),f.push(M)}finally{var p;h=!0,b&&(b(),b=null),t==null||(p=t.signal)===null||p===void 0||p.removeEventListener("abort",s)}}L();try{for(;;){for(;f.length>0;){let _=await f[0];if(_===Mn)return;if(a.aborted)throw new $;_!==De&&(yield _),f.shift(),d&&(d(),d=null)}await new Tn(_=>{b=_})}}finally{l.abort(),h=!0,d&&(d(),d=null)}}.call(this)}function el(e=void 0){return e!=null&&ae(e,"options"),e?.signal!=null&&le(e.signal,"options.signal"),async function*(){let n=0;for await(let i of this){var r;if(e!=null&&(r=e.signal)!==null&&r!==void 0&&r.aborted)throw new $({cause:e.signal.reason});yield[n++,i]}}.call(this)}async function On(e,t=void 0){for await(let n of ot.call(this,e,t))return!0;return!1}async function tl(e,t=void 0){if(typeof e!="function")throw new pe("fn",["Function","AsyncFunction"],e);return!await On.call(this,async(...n)=>!await e(...n),t)}async function nl(e,t){for await(let n of ot.call(this,e,t))return n}async function rl(e,t){if(typeof e!="function")throw new pe("fn",["Function","AsyncFunction"],e);async function n(r,i){return await e(r,i),De}for await(let r of Oe.call(this,n,t));}function ot(e,t){if(typeof e!="function")throw new pe("fn",["Function","AsyncFunction"],e);async function n(r,i){return await e(r,i)?r:De}return Oe.call(this,n,t)}var it=class extends Go{constructor(){super("reduce"),this.message="Reduce of an empty stream requires an initial value"}};async function il(e,t,n){var r;if(typeof e!="function")throw new pe("reducer",["Function","AsyncFunction"],e);n!=null&&ae(n,"options"),n?.signal!=null&&le(n.signal,"options.signal");let i=arguments.length>1;if(n!=null&&(r=n.signal)!==null&&r!==void 0&&r.aborted){let a=new $(void 0,{cause:n.signal.reason});throw this.once("error",()=>{}),await Ko(this.destroy(a)),a}let o=new Nn,l=o.signal;if(n!=null&&n.signal){let a={once:!0,[Yo]:this};n.signal.addEventListener("abort",()=>o.abort(),a)}let u=!1;try{for await(let a of this){var f;if(u=!0,n!=null&&(f=n.signal)!==null&&f!==void 0&&f.aborted)throw new $;i?t=await e(t,a,{signal:l}):(t=a,i=!0)}if(!u&&!i)throw new it}finally{o.abort()}return t}async function ol(e){e!=null&&ae(e,"options"),e?.signal!=null&&le(e.signal,"options.signal");let t=[];for await(let r of this){var n;if(e!=null&&(n=e.signal)!==null&&n!==void 0&&n.aborted)throw new $(void 0,{cause:e.signal.reason});zo(t,r)}return t}function ll(e,t){let n=Oe.call(this,e,t);return async function*(){for await(let i of n)yield*i}.call(this)}function qn(e){if(e=Jo(e),Qo(e))return 0;if(e<0)throw new Ho("number",">= 0",e);return e}function al(e,t=void 0){return t!=null&&ae(t,"options"),t?.signal!=null&&le(t.signal,"options.signal"),e=qn(e),async function*(){var r;if(t!=null&&(r=t.signal)!==null&&r!==void 0&&r.aborted)throw new $;for await(let o of this){var i;if(t!=null&&(i=t.signal)!==null&&i!==void 0&&i.aborted)throw new $;e--<=0&&(yield o)}}.call(this)}function fl(e,t=void 0){return t!=null&&ae(t,"options"),t?.signal!=null&&le(t.signal,"options.signal"),e=qn(e),async function*(){var r;if(t!=null&&(r=t.signal)!==null&&r!==void 0&&r.aborted)throw new $;for await(let o of this){var i;if(t!=null&&(i=t.signal)!==null&&i!==void 0&&i.aborted)throw new $;if(e-- >0)yield o;else return}}.call(this)}lt.exports.streamReturningOperators={asIndexedPairs:el,drop:al,filter:ot,flatMap:ll,map:Oe,take:fl};lt.exports.promiseReturningOperators={every:tl,forEach:rl,reduce:il,toArray:ol,some:On,find:nl}});var Z=g((eu,vn)=>{"use strict";var K=__process$,{aggregateTwoErrors:ul,codes:{ERR_MULTIPLE_CALLBACK:sl},AbortError:dl}=O(),{Symbol:kn}=m(),{kDestroyed:cl,isDestroyed:hl,isFinished:bl,isServerRequest:_l}=V(),Wn=kn("kDestroy"),at=kn("kConstruct");function Cn(e,t,n){e&&(e.stack,t&&!t.errored&&(t.errored=e),n&&!n.errored&&(n.errored=e))}function pl(e,t){let n=this._readableState,r=this._writableState,i=r||n;return r&&r.destroyed||n&&n.destroyed?(typeof t=="function"&&t(),this):(Cn(e,r,n),r&&(r.destroyed=!0),n&&(n.destroyed=!0),i.constructed?Ln(this,e,t):this.once(Wn,function(o){Ln(this,ul(o,e),t)}),this)}function Ln(e,t,n){let r=!1;function i(o){if(r)return;r=!0;let l=e._readableState,u=e._writableState;Cn(o,u,l),u&&(u.closed=!0),l&&(l.closed=!0),typeof n=="function"&&n(o),o?K.nextTick(wl,e,o):K.nextTick(jn,e)}try{e._destroy(t||null,i)}catch(o){i(o)}}function wl(e,t){ft(e,t),jn(e)}function jn(e){let t=e._readableState,n=e._writableState;n&&(n.closeEmitted=!0),t&&(t.closeEmitted=!0),(n&&n.emitClose||t&&t.emitClose)&&e.emit("close")}function ft(e,t){let n=e._readableState,r=e._writableState;r&&r.errorEmitted||n&&n.errorEmitted||(r&&(r.errorEmitted=!0),n&&(n.errorEmitted=!0),e.emit("error",t))}function yl(){let e=this._readableState,t=this._writableState;e&&(e.constructed=!0,e.closed=!1,e.closeEmitted=!1,e.destroyed=!1,e.errored=null,e.errorEmitted=!1,e.reading=!1,e.ended=e.readable===!1,e.endEmitted=e.readable===!1),t&&(t.constructed=!0,t.destroyed=!1,t.closed=!1,t.closeEmitted=!1,t.errored=null,t.errorEmitted=!1,t.finalCalled=!1,t.prefinished=!1,t.ended=t.writable===!1,t.ending=t.writable===!1,t.finished=t.writable===!1)}function ut(e,t,n){let r=e._readableState,i=e._writableState;if(i&&i.destroyed||r&&r.destroyed)return this;r&&r.autoDestroy||i&&i.autoDestroy?e.destroy(t):t&&(t.stack,i&&!i.errored&&(i.errored=t),r&&!r.errored&&(r.errored=t),n?K.nextTick(ft,e,t):ft(e,t))}function gl(e,t){if(typeof e._construct!="function")return;let n=e._readableState,r=e._writableState;n&&(n.constructed=!1),r&&(r.constructed=!1),e.once(at,t),!(e.listenerCount(at)>1)&&K.nextTick(Sl,e)}function Sl(e){let t=!1;function n(r){if(t){ut(e,r??new sl);return}t=!0;let i=e._readableState,o=e._writableState,l=o||i;i&&(i.constructed=!0),o&&(o.constructed=!0),l.destroyed?e.emit(Wn,r):r?ut(e,r,!0):K.nextTick(El,e)}try{e._construct(n)}catch(r){n(r)}}function El(e){e.emit(at)}function Pn(e){return e&&e.setHeader&&typeof e.abort=="function"}function $n(e){e.emit("close")}function Rl(e,t){e.emit("error",t),K.nextTick($n,e)}function Al(e,t){!e||hl(e)||(!t&&!bl(e)&&(t=new dl),_l(e)?(e.socket=null,e.destroy(t)):Pn(e)?e.abort():Pn(e.req)?e.req.abort():typeof e.destroy=="function"?e.destroy(t):typeof e.close=="function"?e.close():t?K.nextTick(Rl,e,t):K.nextTick($n,e),e.destroyed||(e[cl]=!0))}vn.exports={construct:gl,destroyer:Al,destroy:pl,undestroy:yl,errorOrDestroy:ut}});var Le=g((tu,Un)=>{"use strict";var{ArrayIsArray:ml,ObjectSetPrototypeOf:Fn}=m(),{EventEmitter:qe}=__events$;function xe(e){qe.call(this,e)}Fn(xe.prototype,qe.prototype);Fn(xe,qe);xe.prototype.pipe=function(e,t){let n=this;function r(c){e.writable&&e.write(c)===!1&&n.pause&&n.pause()}n.on("data",r);function i(){n.readable&&n.resume&&n.resume()}e.on("drain",i),!e._isStdio&&(!t||t.end!==!1)&&(n.on("end",l),n.on("close",u));let o=!1;function l(){o||(o=!0,e.end())}function u(){o||(o=!0,typeof e.destroy=="function"&&e.destroy())}function f(c){a(),qe.listenerCount(this,"error")===0&&this.emit("error",c)}st(n,"error",f),st(e,"error",f);function a(){n.removeListener("data",r),e.removeListener("drain",i),n.removeListener("end",l),n.removeListener("close",u),n.removeListener("error",f),e.removeListener("error",f),n.removeListener("end",a),n.removeListener("close",a),e.removeListener("close",a)}return n.on("end",a),n.on("close",a),e.on("close",a),e.emit("pipe",n),e};function st(e,t,n){if(typeof e.prependListener=="function")return e.prependListener(t,n);!e._events||!e._events[t]?e.on(t,n):ml(e._events[t])?e._events[t].unshift(n):e._events[t]=[n,e._events[t]]}Un.exports={Stream:xe,prependListener:st}});var ke=g((nu,Pe)=>{"use strict";var{AbortError:Tl,codes:Il}=O(),Ml=Y(),{ERR_INVALID_ARG_TYPE:Bn}=Il,Nl=(e,t)=>{if(typeof e!="object"||!("aborted"in e))throw new Bn(t,"AbortSignal",e)};function Dl(e){return!!(e&&typeof e.pipe=="function")}Pe.exports.addAbortSignal=function(t,n){if(Nl(t,"signal"),!Dl(n))throw new Bn("stream","stream.Stream",n);return Pe.exports.addAbortSignalNoValidate(t,n)};Pe.exports.addAbortSignalNoValidate=function(e,t){if(typeof e!="object"||!("aborted"in e))return t;let n=()=>{t.destroy(new Tl(void 0,{cause:e.reason}))};return e.aborted?n():(e.addEventListener("abort",n),Ml(t,()=>e.removeEventListener("abort",n))),t}});var Vn=g((iu,Hn)=>{"use strict";var{StringPrototypeSlice:Gn,SymbolIterator:Ol,TypedArrayPrototypeSet:We,Uint8Array:ql}=m(),{Buffer:dt}=__buffer$,{inspect:xl}=j();Hn.exports=class{constructor(){this.head=null,this.tail=null,this.length=0}push(t){let n={data:t,next:null};this.length>0?this.tail.next=n:this.head=n,this.tail=n,++this.length}unshift(t){let n={data:t,next:this.head};this.length===0&&(this.tail=n),this.head=n,++this.length}shift(){if(this.length===0)return;let t=this.head.data;return this.length===1?this.head=this.tail=null:this.head=this.head.next,--this.length,t}clear(){this.head=this.tail=null,this.length=0}join(t){if(this.length===0)return"";let n=this.head,r=""+n.data;for(;(n=n.next)!==null;)r+=t+n.data;return r}concat(t){if(this.length===0)return dt.alloc(0);let n=dt.allocUnsafe(t>>>0),r=this.head,i=0;for(;r;)We(n,r.data,i),i+=r.data.length,r=r.next;return n}consume(t,n){let r=this.head.data;if(t<r.length){let i=r.slice(0,t);return this.head.data=r.slice(t),i}return t===r.length?this.shift():n?this._getString(t):this._getBuffer(t)}first(){return this.head.data}*[Ol](){for(let t=this.head;t;t=t.next)yield t.data}_getString(t){let n="",r=this.head,i=0;do{let o=r.data;if(t>o.length)n+=o,t-=o.length;else{t===o.length?(n+=o,++i,r.next?this.head=r.next:this.head=this.tail=null):(n+=Gn(o,0,t),this.head=r,r.data=Gn(o,t));break}++i}while((r=r.next)!==null);return this.length-=i,n}_getBuffer(t){let n=dt.allocUnsafe(t),r=t,i=this.head,o=0;do{let l=i.data;if(t>l.length)We(n,l,r-t),t-=l.length;else{t===l.length?(We(n,l,r-t),++o,i.next?this.head=i.next:this.head=this.tail=null):(We(n,new ql(l.buffer,l.byteOffset,t),r-t),this.head=i,i.data=l.slice(t));break}++o}while((i=i.next)!==null);return this.length-=o,n}[Symbol.for("nodejs.util.inspect.custom")](t,n){return xl(this,{...n,depth:0,customInspect:!1})}}});var Ce=g((ou,Kn)=>{"use strict";var{MathFloor:Ll,NumberIsInteger:Pl}=m(),{ERR_INVALID_ARG_VALUE:kl}=O().codes;function Wl(e,t,n){return e.highWaterMark!=null?e.highWaterMark:t?e[n]:null}function Yn(e){return e?16:16*1024}function Cl(e,t,n,r){let i=Wl(t,r,n);if(i!=null){if(!Pl(i)||i<0){let o=r?`options.${n}`:"options.highWaterMark";throw new kl(o,i)}return Ll(i)}return Yn(e.objectMode)}Kn.exports={getHighWaterMark:Cl,getDefaultHighWaterMark:Yn}});var ct=g((lu,Qn)=>{"use strict";var zn=__process$,{PromisePrototypeThen:jl,SymbolAsyncIterator:Xn,SymbolIterator:Jn}=m(),{Buffer:$l}=__buffer$,{ERR_INVALID_ARG_TYPE:vl,ERR_STREAM_NULL_VALUES:Fl}=O().codes;function Ul(e,t,n){let r;if(typeof t=="string"||t instanceof $l)return new e({objectMode:!0,...n,read(){this.push(t),this.push(null)}});let i;if(t&&t[Xn])i=!0,r=t[Xn]();else if(t&&t[Jn])i=!1,r=t[Jn]();else throw new vl("iterable",["Iterable"],t);let o=new e({objectMode:!0,highWaterMark:1,...n}),l=!1;o._read=function(){l||(l=!0,f())},o._destroy=function(a,c){jl(u(a),()=>zn.nextTick(c,a),s=>zn.nextTick(c,s||a))};async function u(a){let c=a!=null,s=typeof r.throw=="function";if(c&&s){let{value:b,done:d}=await r.throw(a);if(await b,d)return}if(typeof r.return=="function"){let{value:b}=await r.return();await b}}async function f(){for(;;){try{let{value:a,done:c}=i?await r.next():r.next();if(c)o.push(null);else{let s=a&&typeof a.then=="function"?await a:a;if(s===null)throw l=!1,new Fl;if(o.push(s))continue;l=!1}}catch(a){o.destroy(a)}break}}return o}Qn.exports=Ul});var we=g((au,dr)=>{var W=__process$,{ArrayPrototypeIndexOf:Bl,NumberIsInteger:Gl,NumberIsNaN:Hl,NumberParseInt:Vl,ObjectDefineProperties:tr,ObjectKeys:Yl,ObjectSetPrototypeOf:nr,Promise:Kl,SafeSet:zl,SymbolAsyncIterator:Xl,Symbol:Jl}=m();dr.exports=w;w.ReadableState=yt;var{EventEmitter:Ql}=__events$,{Stream:z,prependListener:Zl}=Le(),{Buffer:ht}=__buffer$,{addAbortSignal:ea}=ke(),ta=Y(),y=j().debuglog("stream",e=>{y=e}),na=Vn(),ue=Z(),{getHighWaterMark:ra,getDefaultHighWaterMark:ia}=Ce(),{aggregateTwoErrors:Zn,codes:{ERR_INVALID_ARG_TYPE:oa,ERR_METHOD_NOT_IMPLEMENTED:la,ERR_OUT_OF_RANGE:aa,ERR_STREAM_PUSH_AFTER_EOF:fa,ERR_STREAM_UNSHIFT_AFTER_END_EVENT:ua}}=O(),{validateObject:sa}=_e(),ee=Jl("kPaused"),{StringDecoder:rr}=__string_decoder$,da=ct();nr(w.prototype,z.prototype);nr(w,z);var bt=()=>{},{errorOrDestroy:fe}=ue;function yt(e,t,n){typeof n!="boolean"&&(n=t instanceof v()),this.objectMode=!!(e&&e.objectMode),n&&(this.objectMode=this.objectMode||!!(e&&e.readableObjectMode)),this.highWaterMark=e?ra(this,e,"readableHighWaterMark",n):ia(!1),this.buffer=new na,this.length=0,this.pipes=[],this.flowing=null,this.ended=!1,this.endEmitted=!1,this.reading=!1,this.constructed=!0,this.sync=!0,this.needReadable=!1,this.emittedReadable=!1,this.readableListening=!1,this.resumeScheduled=!1,this[ee]=null,this.errorEmitted=!1,this.emitClose=!e||e.emitClose!==!1,this.autoDestroy=!e||e.autoDestroy!==!1,this.destroyed=!1,this.errored=null,this.closed=!1,this.closeEmitted=!1,this.defaultEncoding=e&&e.defaultEncoding||"utf8",this.awaitDrainWriters=null,this.multiAwaitDrain=!1,this.readingMore=!1,this.dataEmitted=!1,this.decoder=null,this.encoding=null,e&&e.encoding&&(this.decoder=new rr(e.encoding),this.encoding=e.encoding)}function w(e){if(!(this instanceof w))return new w(e);let t=this instanceof v();this._readableState=new yt(e,this,t),e&&(typeof e.read=="function"&&(this._read=e.read),typeof e.destroy=="function"&&(this._destroy=e.destroy),typeof e.construct=="function"&&(this._construct=e.construct),e.signal&&!t&&ea(e.signal,this)),z.call(this,e),ue.construct(this,()=>{this._readableState.needReadable&&je(this,this._readableState)})}w.prototype.destroy=ue.destroy;w.prototype._undestroy=ue.undestroy;w.prototype._destroy=function(e,t){t(e)};w.prototype[Ql.captureRejectionSymbol]=function(e){this.destroy(e)};w.prototype.push=function(e,t){return ir(this,e,t,!1)};w.prototype.unshift=function(e,t){return ir(this,e,t,!0)};function ir(e,t,n,r){y("readableAddChunk",t);let i=e._readableState,o;if(i.objectMode||(typeof t=="string"?(n=n||i.defaultEncoding,i.encoding!==n&&(r&&i.encoding?t=ht.from(t,n).toString(i.encoding):(t=ht.from(t,n),n=""))):t instanceof ht?n="":z._isUint8Array(t)?(t=z._uint8ArrayToBuffer(t),n=""):t!=null&&(o=new oa("chunk",["string","Buffer","Uint8Array"],t))),o)fe(e,o);else if(t===null)i.reading=!1,ba(e,i);else if(i.objectMode||t&&t.length>0)if(r)if(i.endEmitted)fe(e,new ua);else{if(i.destroyed||i.errored)return!1;_t(e,i,t,!0)}else if(i.ended)fe(e,new fa);else{if(i.destroyed||i.errored)return!1;i.reading=!1,i.decoder&&!n?(t=i.decoder.write(t),i.objectMode||t.length!==0?_t(e,i,t,!1):je(e,i)):_t(e,i,t,!1)}else r||(i.reading=!1,je(e,i));return!i.ended&&(i.length<i.highWaterMark||i.length===0)}function _t(e,t,n,r){t.flowing&&t.length===0&&!t.sync&&e.listenerCount("data")>0?(t.multiAwaitDrain?t.awaitDrainWriters.clear():t.awaitDrainWriters=null,t.dataEmitted=!0,e.emit("data",n)):(t.length+=t.objectMode?1:n.length,r?t.buffer.unshift(n):t.buffer.push(n),t.needReadable&&$e(e)),je(e,t)}w.prototype.isPaused=function(){let e=this._readableState;return e[ee]===!0||e.flowing===!1};w.prototype.setEncoding=function(e){let t=new rr(e);this._readableState.decoder=t,this._readableState.encoding=this._readableState.decoder.encoding;let n=this._readableState.buffer,r="";for(let i of n)r+=t.write(i);return n.clear(),r!==""&&n.push(r),this._readableState.length=r.length,this};var ca=1073741824;function ha(e){if(e>ca)throw new aa("size","<= 1GiB",e);return e--,e|=e>>>1,e|=e>>>2,e|=e>>>4,e|=e>>>8,e|=e>>>16,e++,e}function er(e,t){return e<=0||t.length===0&&t.ended?0:t.objectMode?1:Hl(e)?t.flowing&&t.length?t.buffer.first().length:t.length:e<=t.length?e:t.ended?t.length:0}w.prototype.read=function(e){y("read",e),e===void 0?e=NaN:Gl(e)||(e=Vl(e,10));let t=this._readableState,n=e;if(e>t.highWaterMark&&(t.highWaterMark=ha(e)),e!==0&&(t.emittedReadable=!1),e===0&&t.needReadable&&((t.highWaterMark!==0?t.length>=t.highWaterMark:t.length>0)||t.ended))return y("read: emitReadable",t.length,t.ended),t.length===0&&t.ended?pt(this):$e(this),null;if(e=er(e,t),e===0&&t.ended)return t.length===0&&pt(this),null;let r=t.needReadable;if(y("need readable",r),(t.length===0||t.length-e<t.highWaterMark)&&(r=!0,y("length less than watermark",r)),t.ended||t.reading||t.destroyed||t.errored||!t.constructed)r=!1,y("reading, ended or constructing",r);else if(r){y("do read"),t.reading=!0,t.sync=!0,t.length===0&&(t.needReadable=!0);try{this._read(t.highWaterMark)}catch(o){fe(this,o)}t.sync=!1,t.reading||(e=er(n,t))}let i;return e>0?i=ur(e,t):i=null,i===null?(t.needReadable=t.length<=t.highWaterMark,e=0):(t.length-=e,t.multiAwaitDrain?t.awaitDrainWriters.clear():t.awaitDrainWriters=null),t.length===0&&(t.ended||(t.needReadable=!0),n!==e&&t.ended&&pt(this)),i!==null&&!t.errorEmitted&&!t.closeEmitted&&(t.dataEmitted=!0,this.emit("data",i)),i};function ba(e,t){if(y("onEofChunk"),!t.ended){if(t.decoder){let n=t.decoder.end();n&&n.length&&(t.buffer.push(n),t.length+=t.objectMode?1:n.length)}t.ended=!0,t.sync?$e(e):(t.needReadable=!1,t.emittedReadable=!0,or(e))}}function $e(e){let t=e._readableState;y("emitReadable",t.needReadable,t.emittedReadable),t.needReadable=!1,t.emittedReadable||(y("emitReadable",t.flowing),t.emittedReadable=!0,W.nextTick(or,e))}function or(e){let t=e._readableState;y("emitReadable_",t.destroyed,t.length,t.ended),!t.destroyed&&!t.errored&&(t.length||t.ended)&&(e.emit("readable"),t.emittedReadable=!1),t.needReadable=!t.flowing&&!t.ended&&t.length<=t.highWaterMark,ar(e)}function je(e,t){!t.readingMore&&t.constructed&&(t.readingMore=!0,W.nextTick(_a,e,t))}function _a(e,t){for(;!t.reading&&!t.ended&&(t.length<t.highWaterMark||t.flowing&&t.length===0);){let n=t.length;if(y("maybeReadMore read 0"),e.read(0),n===t.length)break}t.readingMore=!1}w.prototype._read=function(e){throw new la("_read()")};w.prototype.pipe=function(e,t){let n=this,r=this._readableState;r.pipes.length===1&&(r.multiAwaitDrain||(r.multiAwaitDrain=!0,r.awaitDrainWriters=new zl(r.awaitDrainWriters?[r.awaitDrainWriters]:[]))),r.pipes.push(e),y("pipe count=%d opts=%j",r.pipes.length,t);let o=(!t||t.end!==!1)&&e!==W.stdout&&e!==W.stderr?u:L;r.endEmitted?W.nextTick(o):n.once("end",o),e.on("unpipe",l);function l(_,p){y("onunpipe"),_===n&&p&&p.hasUnpiped===!1&&(p.hasUnpiped=!0,c())}function u(){y("onend"),e.end()}let f,a=!1;function c(){y("cleanup"),e.removeListener("close",h),e.removeListener("finish",D),f&&e.removeListener("drain",f),e.removeListener("error",d),e.removeListener("unpipe",l),n.removeListener("end",u),n.removeListener("end",L),n.removeListener("data",b),a=!0,f&&r.awaitDrainWriters&&(!e._writableState||e._writableState.needDrain)&&f()}function s(){a||(r.pipes.length===1&&r.pipes[0]===e?(y("false write response, pause",0),r.awaitDrainWriters=e,r.multiAwaitDrain=!1):r.pipes.length>1&&r.pipes.includes(e)&&(y("false write response, pause",r.awaitDrainWriters.size),r.awaitDrainWriters.add(e)),n.pause()),f||(f=pa(n,e),e.on("drain",f))}n.on("data",b);function b(_){y("ondata");let p=e.write(_);y("dest.write",p),p===!1&&s()}function d(_){if(y("onerror",_),L(),e.removeListener("error",d),e.listenerCount("error")===0){let p=e._writableState||e._readableState;p&&!p.errorEmitted?fe(e,_):e.emit("error",_)}}Zl(e,"error",d);function h(){e.removeListener("finish",D),L()}e.once("close",h);function D(){y("onfinish"),e.removeListener("close",h),L()}e.once("finish",D);function L(){y("unpipe"),n.unpipe(e)}return e.emit("pipe",n),e.writableNeedDrain===!0?r.flowing&&s():r.flowing||(y("pipe resume"),n.resume()),e};function pa(e,t){return function(){let r=e._readableState;r.awaitDrainWriters===t?(y("pipeOnDrain",1),r.awaitDrainWriters=null):r.multiAwaitDrain&&(y("pipeOnDrain",r.awaitDrainWriters.size),r.awaitDrainWriters.delete(t)),(!r.awaitDrainWriters||r.awaitDrainWriters.size===0)&&e.listenerCount("data")&&e.resume()}}w.prototype.unpipe=function(e){let t=this._readableState,n={hasUnpiped:!1};if(t.pipes.length===0)return this;if(!e){let i=t.pipes;t.pipes=[],this.pause();for(let o=0;o<i.length;o++)i[o].emit("unpipe",this,{hasUnpiped:!1});return this}let r=Bl(t.pipes,e);return r===-1?this:(t.pipes.splice(r,1),t.pipes.length===0&&this.pause(),e.emit("unpipe",this,n),this)};w.prototype.on=function(e,t){let n=z.prototype.on.call(this,e,t),r=this._readableState;return e==="data"?(r.readableListening=this.listenerCount("readable")>0,r.flowing!==!1&&this.resume()):e==="readable"&&!r.endEmitted&&!r.readableListening&&(r.readableListening=r.needReadable=!0,r.flowing=!1,r.emittedReadable=!1,y("on readable",r.length,r.reading),r.length?$e(this):r.reading||W.nextTick(wa,this)),n};w.prototype.addListener=w.prototype.on;w.prototype.removeListener=function(e,t){let n=z.prototype.removeListener.call(this,e,t);return e==="readable"&&W.nextTick(lr,this),n};w.prototype.off=w.prototype.removeListener;w.prototype.removeAllListeners=function(e){let t=z.prototype.removeAllListeners.apply(this,arguments);return(e==="readable"||e===void 0)&&W.nextTick(lr,this),t};function lr(e){let t=e._readableState;t.readableListening=e.listenerCount("readable")>0,t.resumeScheduled&&t[ee]===!1?t.flowing=!0:e.listenerCount("data")>0?e.resume():t.readableListening||(t.flowing=null)}function wa(e){y("readable nexttick read 0"),e.read(0)}w.prototype.resume=function(){let e=this._readableState;return e.flowing||(y("resume"),e.flowing=!e.readableListening,ya(this,e)),e[ee]=!1,this};function ya(e,t){t.resumeScheduled||(t.resumeScheduled=!0,W.nextTick(ga,e,t))}function ga(e,t){y("resume",t.reading),t.reading||e.read(0),t.resumeScheduled=!1,e.emit("resume"),ar(e),t.flowing&&!t.reading&&e.read(0)}w.prototype.pause=function(){return y("call pause flowing=%j",this._readableState.flowing),this._readableState.flowing!==!1&&(y("pause"),this._readableState.flowing=!1,this.emit("pause")),this._readableState[ee]=!0,this};function ar(e){let t=e._readableState;for(y("flow",t.flowing);t.flowing&&e.read()!==null;);}w.prototype.wrap=function(e){let t=!1;e.on("data",r=>{!this.push(r)&&e.pause&&(t=!0,e.pause())}),e.on("end",()=>{this.push(null)}),e.on("error",r=>{fe(this,r)}),e.on("close",()=>{this.destroy()}),e.on("destroy",()=>{this.destroy()}),this._read=()=>{t&&e.resume&&(t=!1,e.resume())};let n=Yl(e);for(let r=1;r<n.length;r++){let i=n[r];this[i]===void 0&&typeof e[i]=="function"&&(this[i]=e[i].bind(e))}return this};w.prototype[Xl]=function(){return fr(this)};w.prototype.iterator=function(e){return e!==void 0&&sa(e,"options"),fr(this,e)};function fr(e,t){typeof e.read!="function"&&(e=w.wrap(e,{objectMode:!0}));let n=Sa(e,t);return n.stream=e,n}async function*Sa(e,t){let n=bt;function r(l){this===e?(n(),n=bt):n=l}e.on("readable",r);let i,o=ta(e,{writable:!1},l=>{i=l?Zn(i,l):null,n(),n=bt});try{for(;;){let l=e.destroyed?null:e.read();if(l!==null)yield l;else{if(i)throw i;if(i===null)return;await new Kl(r)}}}catch(l){throw i=Zn(i,l),i}finally{(i||t?.destroyOnReturn!==!1)&&(i===void 0||e._readableState.autoDestroy)?ue.destroyer(e,null):(e.off("readable",r),o())}}tr(w.prototype,{readable:{__proto__:null,get(){let e=this._readableState;return!!e&&e.readable!==!1&&!e.destroyed&&!e.errorEmitted&&!e.endEmitted},set(e){this._readableState&&(this._readableState.readable=!!e)}},readableDidRead:{__proto__:null,enumerable:!1,get:function(){return this._readableState.dataEmitted}},readableAborted:{__proto__:null,enumerable:!1,get:function(){return!!(this._readableState.readable!==!1&&(this._readableState.destroyed||this._readableState.errored)&&!this._readableState.endEmitted)}},readableHighWaterMark:{__proto__:null,enumerable:!1,get:function(){return this._readableState.highWaterMark}},readableBuffer:{__proto__:null,enumerable:!1,get:function(){return this._readableState&&this._readableState.buffer}},readableFlowing:{__proto__:null,enumerable:!1,get:function(){return this._readableState.flowing},set:function(e){this._readableState&&(this._readableState.flowing=e)}},readableLength:{__proto__:null,enumerable:!1,get(){return this._readableState.length}},readableObjectMode:{__proto__:null,enumerable:!1,get(){return this._readableState?this._readableState.objectMode:!1}},readableEncoding:{__proto__:null,enumerable:!1,get(){return this._readableState?this._readableState.encoding:null}},errored:{__proto__:null,enumerable:!1,get(){return this._readableState?this._readableState.errored:null}},closed:{__proto__:null,get(){return this._readableState?this._readableState.closed:!1}},destroyed:{__proto__:null,enumerable:!1,get(){return this._readableState?this._readableState.destroyed:!1},set(e){!this._readableState||(this._readableState.destroyed=e)}},readableEnded:{__proto__:null,enumerable:!1,get(){return this._readableState?this._readableState.endEmitted:!1}}});tr(yt.prototype,{pipesCount:{__proto__:null,get(){return this.pipes.length}},paused:{__proto__:null,get(){return this[ee]!==!1},set(e){this[ee]=!!e}}});w._fromList=ur;function ur(e,t){if(t.length===0)return null;let n;return t.objectMode?n=t.buffer.shift():!e||e>=t.length?(t.decoder?n=t.buffer.join(""):t.buffer.length===1?n=t.buffer.first():n=t.buffer.concat(t.length),t.buffer.clear()):n=t.buffer.consume(e,t.decoder),n}function pt(e){let t=e._readableState;y("endReadable",t.endEmitted),t.endEmitted||(t.ended=!0,W.nextTick(Ea,t,e))}function Ea(e,t){if(y("endReadableNT",e.endEmitted,e.length),!e.errored&&!e.closeEmitted&&!e.endEmitted&&e.length===0){if(e.endEmitted=!0,t.emit("end"),t.writable&&t.allowHalfOpen===!1)W.nextTick(Ra,t);else if(e.autoDestroy){let n=t._writableState;(!n||n.autoDestroy&&(n.finished||n.writable===!1))&&t.destroy()}}}function Ra(e){e.writable&&!e.writableEnded&&!e.destroyed&&e.end()}w.from=function(e,t){return da(w,e,t)};var wt;function sr(){return wt===void 0&&(wt={}),wt}w.fromWeb=function(e,t){return sr().newStreamReadableFromReadableStream(e,t)};w.toWeb=function(e,t){return sr().newReadableStreamFromStreamReadable(e,t)};w.wrap=function(e,t){var n,r;return new w({objectMode:(n=(r=e.readableObjectMode)!==null&&r!==void 0?r:e.objectMode)!==null&&n!==void 0?n:!0,...t,destroy(i,o){ue.destroyer(e,i),o(i)}}).wrap(e)}});var Tt=g((fu,Ar)=>{var te=__process$,{ArrayPrototypeSlice:br,Error:Aa,FunctionPrototypeSymbolHasInstance:_r,ObjectDefineProperty:pr,ObjectDefineProperties:ma,ObjectSetPrototypeOf:wr,StringPrototypeToLowerCase:Ta,Symbol:Ia,SymbolHasInstance:Ma}=m();Ar.exports=S;S.WritableState=Se;var{EventEmitter:Na}=__events$,ye=Le().Stream,{Buffer:ve}=__buffer$,Be=Z(),{addAbortSignal:Da}=ke(),{getHighWaterMark:Oa,getDefaultHighWaterMark:qa}=Ce(),{ERR_INVALID_ARG_TYPE:xa,ERR_METHOD_NOT_IMPLEMENTED:La,ERR_MULTIPLE_CALLBACK:yr,ERR_STREAM_CANNOT_PIPE:Pa,ERR_STREAM_DESTROYED:ge,ERR_STREAM_ALREADY_FINISHED:ka,ERR_STREAM_NULL_VALUES:Wa,ERR_STREAM_WRITE_AFTER_END:Ca,ERR_UNKNOWN_ENCODING:gr}=O().codes,{errorOrDestroy:se}=Be;wr(S.prototype,ye.prototype);wr(S,ye);function Et(){}var de=Ia("kOnFinished");function Se(e,t,n){typeof n!="boolean"&&(n=t instanceof v()),this.objectMode=!!(e&&e.objectMode),n&&(this.objectMode=this.objectMode||!!(e&&e.writableObjectMode)),this.highWaterMark=e?Oa(this,e,"writableHighWaterMark",n):qa(!1),this.finalCalled=!1,this.needDrain=!1,this.ending=!1,this.ended=!1,this.finished=!1,this.destroyed=!1;let r=!!(e&&e.decodeStrings===!1);this.decodeStrings=!r,this.defaultEncoding=e&&e.defaultEncoding||"utf8",this.length=0,this.writing=!1,this.corked=0,this.sync=!0,this.bufferProcessing=!1,this.onwrite=$a.bind(void 0,t),this.writecb=null,this.writelen=0,this.afterWriteTickInfo=null,Ue(this),this.pendingcb=0,this.constructed=!0,this.prefinished=!1,this.errorEmitted=!1,this.emitClose=!e||e.emitClose!==!1,this.autoDestroy=!e||e.autoDestroy!==!1,this.errored=null,this.closed=!1,this.closeEmitted=!1,this[de]=[]}function Ue(e){e.buffered=[],e.bufferedIndex=0,e.allBuffers=!0,e.allNoop=!0}Se.prototype.getBuffer=function(){return br(this.buffered,this.bufferedIndex)};pr(Se.prototype,"bufferedRequestCount",{__proto__:null,get(){return this.buffered.length-this.bufferedIndex}});function S(e){let t=this instanceof v();if(!t&&!_r(S,this))return new S(e);this._writableState=new Se(e,this,t),e&&(typeof e.write=="function"&&(this._write=e.write),typeof e.writev=="function"&&(this._writev=e.writev),typeof e.destroy=="function"&&(this._destroy=e.destroy),typeof e.final=="function"&&(this._final=e.final),typeof e.construct=="function"&&(this._construct=e.construct),e.signal&&Da(e.signal,this)),ye.call(this,e),Be.construct(this,()=>{let n=this._writableState;n.writing||At(this,n),mt(this,n)})}pr(S,Ma,{__proto__:null,value:function(e){return _r(this,e)?!0:this!==S?!1:e&&e._writableState instanceof Se}});S.prototype.pipe=function(){se(this,new Pa)};function Sr(e,t,n,r){let i=e._writableState;if(typeof n=="function")r=n,n=i.defaultEncoding;else{if(!n)n=i.defaultEncoding;else if(n!=="buffer"&&!ve.isEncoding(n))throw new gr(n);typeof r!="function"&&(r=Et)}if(t===null)throw new Wa;if(!i.objectMode)if(typeof t=="string")i.decodeStrings!==!1&&(t=ve.from(t,n),n="buffer");else if(t instanceof ve)n="buffer";else if(ye._isUint8Array(t))t=ye._uint8ArrayToBuffer(t),n="buffer";else throw new xa("chunk",["string","Buffer","Uint8Array"],t);let o;return i.ending?o=new Ca:i.destroyed&&(o=new ge("write")),o?(te.nextTick(r,o),se(e,o,!0),o):(i.pendingcb++,ja(e,i,t,n,r))}S.prototype.write=function(e,t,n){return Sr(this,e,t,n)===!0};S.prototype.cork=function(){this._writableState.corked++};S.prototype.uncork=function(){let e=this._writableState;e.corked&&(e.corked--,e.writing||At(this,e))};S.prototype.setDefaultEncoding=function(t){if(typeof t=="string"&&(t=Ta(t)),!ve.isEncoding(t))throw new gr(t);return this._writableState.defaultEncoding=t,this};function ja(e,t,n,r,i){let o=t.objectMode?1:n.length;t.length+=o;let l=t.length<t.highWaterMark;return l||(t.needDrain=!0),t.writing||t.corked||t.errored||!t.constructed?(t.buffered.push({chunk:n,encoding:r,callback:i}),t.allBuffers&&r!=="buffer"&&(t.allBuffers=!1),t.allNoop&&i!==Et&&(t.allNoop=!1)):(t.writelen=o,t.writecb=i,t.writing=!0,t.sync=!0,e._write(n,r,t.onwrite),t.sync=!1),l&&!t.errored&&!t.destroyed}function cr(e,t,n,r,i,o,l){t.writelen=r,t.writecb=l,t.writing=!0,t.sync=!0,t.destroyed?t.onwrite(new ge("write")):n?e._writev(i,t.onwrite):e._write(i,o,t.onwrite),t.sync=!1}function hr(e,t,n,r){--t.pendingcb,r(n),Rt(t),se(e,n)}function $a(e,t){let n=e._writableState,r=n.sync,i=n.writecb;if(typeof i!="function"){se(e,new yr);return}n.writing=!1,n.writecb=null,n.length-=n.writelen,n.writelen=0,t?(t.stack,n.errored||(n.errored=t),e._readableState&&!e._readableState.errored&&(e._readableState.errored=t),r?te.nextTick(hr,e,n,t,i):hr(e,n,t,i)):(n.buffered.length>n.bufferedIndex&&At(e,n),r?n.afterWriteTickInfo!==null&&n.afterWriteTickInfo.cb===i?n.afterWriteTickInfo.count++:(n.afterWriteTickInfo={count:1,cb:i,stream:e,state:n},te.nextTick(va,n.afterWriteTickInfo)):Er(e,n,1,i))}function va({stream:e,state:t,count:n,cb:r}){return t.afterWriteTickInfo=null,Er(e,t,n,r)}function Er(e,t,n,r){for(!t.ending&&!e.destroyed&&t.length===0&&t.needDrain&&(t.needDrain=!1,e.emit("drain"));n-- >0;)t.pendingcb--,r();t.destroyed&&Rt(t),mt(e,t)}function Rt(e){if(e.writing)return;for(let i=e.bufferedIndex;i<e.buffered.length;++i){var t;let{chunk:o,callback:l}=e.buffered[i],u=e.objectMode?1:o.length;e.length-=u,l((t=e.errored)!==null&&t!==void 0?t:new ge("write"))}let n=e[de].splice(0);for(let i=0;i<n.length;i++){var r;n[i]((r=e.errored)!==null&&r!==void 0?r:new ge("end"))}Ue(e)}function At(e,t){if(t.corked||t.bufferProcessing||t.destroyed||!t.constructed)return;let{buffered:n,bufferedIndex:r,objectMode:i}=t,o=n.length-r;if(!o)return;let l=r;if(t.bufferProcessing=!0,o>1&&e._writev){t.pendingcb-=o-1;let u=t.allNoop?Et:a=>{for(let c=l;c<n.length;++c)n[c].callback(a)},f=t.allNoop&&l===0?n:br(n,l);f.allBuffers=t.allBuffers,cr(e,t,!0,t.length,f,"",u),Ue(t)}else{do{let{chunk:u,encoding:f,callback:a}=n[l];n[l++]=null;let c=i?1:u.length;cr(e,t,!1,c,u,f,a)}while(l<n.length&&!t.writing);l===n.length?Ue(t):l>256?(n.splice(0,l),t.bufferedIndex=0):t.bufferedIndex=l}t.bufferProcessing=!1}S.prototype._write=function(e,t,n){if(this._writev)this._writev([{chunk:e,encoding:t}],n);else throw new La("_write()")};S.prototype._writev=null;S.prototype.end=function(e,t,n){let r=this._writableState;typeof e=="function"?(n=e,e=null,t=null):typeof t=="function"&&(n=t,t=null);let i;if(e!=null){let o=Sr(this,e,t);o instanceof Aa&&(i=o)}return r.corked&&(r.corked=1,this.uncork()),i||(!r.errored&&!r.ending?(r.ending=!0,mt(this,r,!0),r.ended=!0):r.finished?i=new ka("end"):r.destroyed&&(i=new ge("end"))),typeof n=="function"&&(i||r.finished?te.nextTick(n,i):r[de].push(n)),this};function Fe(e){return e.ending&&!e.destroyed&&e.constructed&&e.length===0&&!e.errored&&e.buffered.length===0&&!e.finished&&!e.writing&&!e.errorEmitted&&!e.closeEmitted}function Fa(e,t){let n=!1;function r(i){if(n){se(e,i??yr());return}if(n=!0,t.pendingcb--,i){let o=t[de].splice(0);for(let l=0;l<o.length;l++)o[l](i);se(e,i,t.sync)}else Fe(t)&&(t.prefinished=!0,e.emit("prefinish"),t.pendingcb++,te.nextTick(St,e,t))}t.sync=!0,t.pendingcb++;try{e._final(r)}catch(i){r(i)}t.sync=!1}function Ua(e,t){!t.prefinished&&!t.finalCalled&&(typeof e._final=="function"&&!t.destroyed?(t.finalCalled=!0,Fa(e,t)):(t.prefinished=!0,e.emit("prefinish")))}function mt(e,t,n){Fe(t)&&(Ua(e,t),t.pendingcb===0&&(n?(t.pendingcb++,te.nextTick((r,i)=>{Fe(i)?St(r,i):i.pendingcb--},e,t)):Fe(t)&&(t.pendingcb++,St(e,t))))}function St(e,t){t.pendingcb--,t.finished=!0;let n=t[de].splice(0);for(let r=0;r<n.length;r++)n[r]();if(e.emit("finish"),t.autoDestroy){let r=e._readableState;(!r||r.autoDestroy&&(r.endEmitted||r.readable===!1))&&e.destroy()}}ma(S.prototype,{closed:{__proto__:null,get(){return this._writableState?this._writableState.closed:!1}},destroyed:{__proto__:null,get(){return this._writableState?this._writableState.destroyed:!1},set(e){this._writableState&&(this._writableState.destroyed=e)}},writable:{__proto__:null,get(){let e=this._writableState;return!!e&&e.writable!==!1&&!e.destroyed&&!e.errored&&!e.ending&&!e.ended},set(e){this._writableState&&(this._writableState.writable=!!e)}},writableFinished:{__proto__:null,get(){return this._writableState?this._writableState.finished:!1}},writableObjectMode:{__proto__:null,get(){return this._writableState?this._writableState.objectMode:!1}},writableBuffer:{__proto__:null,get(){return this._writableState&&this._writableState.getBuffer()}},writableEnded:{__proto__:null,get(){return this._writableState?this._writableState.ending:!1}},writableNeedDrain:{__proto__:null,get(){let e=this._writableState;return e?!e.destroyed&&!e.ending&&e.needDrain:!1}},writableHighWaterMark:{__proto__:null,get(){return this._writableState&&this._writableState.highWaterMark}},writableCorked:{__proto__:null,get(){return this._writableState?this._writableState.corked:0}},writableLength:{__proto__:null,get(){return this._writableState&&this._writableState.length}},errored:{__proto__:null,enumerable:!1,get(){return this._writableState?this._writableState.errored:null}},writableAborted:{__proto__:null,enumerable:!1,get:function(){return!!(this._writableState.writable!==!1&&(this._writableState.destroyed||this._writableState.errored)&&!this._writableState.finished)}}});var Ba=Be.destroy;S.prototype.destroy=function(e,t){let n=this._writableState;return!n.destroyed&&(n.bufferedIndex<n.buffered.length||n[de].length)&&te.nextTick(Rt,n),Ba.call(this,e,t),this};S.prototype._undestroy=Be.undestroy;S.prototype._destroy=function(e,t){t(e)};S.prototype[Na.captureRejectionSymbol]=function(e){this.destroy(e)};var gt;function Rr(){return gt===void 0&&(gt={}),gt}S.fromWeb=function(e,t){return Rr().newStreamWritableFromWritableStream(e,t)};S.toWeb=function(e){return Rr().newWritableStreamFromStreamWritable(e)}});var kr=g((uu,Pr)=>{var It=__process$,Ga=__buffer$,{isReadable:Ha,isWritable:Va,isIterable:mr,isNodeStream:Ya,isReadableNodeStream:Tr,isWritableNodeStream:Ir,isDuplexNodeStream:Ka}=V(),Mr=Y(),{AbortError:Lr,codes:{ERR_INVALID_ARG_TYPE:za,ERR_INVALID_RETURN_VALUE:Nr}}=O(),{destroyer:ce}=Z(),Xa=v(),Ja=we(),{createDeferredPromise:Dr}=j(),Or=ct(),qr=Blob||Ga.Blob,Qa=typeof qr<"u"?function(t){return t instanceof qr}:function(t){return!1},Za=AbortController,{FunctionPrototypeCall:xr}=m(),ne=class extends Xa{constructor(t){super(t),t?.readable===!1&&(this._readableState.readable=!1,this._readableState.ended=!0,this._readableState.endEmitted=!0),t?.writable===!1&&(this._writableState.writable=!1,this._writableState.ending=!0,this._writableState.ended=!0,this._writableState.finished=!0)}};Pr.exports=function e(t,n){if(Ka(t))return t;if(Tr(t))return Ge({readable:t});if(Ir(t))return Ge({writable:t});if(Ya(t))return Ge({writable:!1,readable:!1});if(typeof t=="function"){let{value:i,write:o,final:l,destroy:u}=ef(t);if(mr(i))return Or(ne,i,{objectMode:!0,write:o,final:l,destroy:u});let f=i?.then;if(typeof f=="function"){let a,c=xr(f,i,s=>{if(s!=null)throw new Nr("nully","body",s)},s=>{ce(a,s)});return a=new ne({objectMode:!0,readable:!1,write:o,final(s){l(async()=>{try{await c,It.nextTick(s,null)}catch(b){It.nextTick(s,b)}})},destroy:u})}throw new Nr("Iterable, AsyncIterable or AsyncFunction",n,i)}if(Qa(t))return e(t.arrayBuffer());if(mr(t))return Or(ne,t,{objectMode:!0,writable:!1});if(typeof t?.writable=="object"||typeof t?.readable=="object"){let i=t!=null&&t.readable?Tr(t?.readable)?t?.readable:e(t.readable):void 0,o=t!=null&&t.writable?Ir(t?.writable)?t?.writable:e(t.writable):void 0;return Ge({readable:i,writable:o})}let r=t?.then;if(typeof r=="function"){let i;return xr(r,t,o=>{o!=null&&i.push(o),i.push(null)},o=>{ce(i,o)}),i=new ne({objectMode:!0,writable:!1,read(){}})}throw new za(n,["Blob","ReadableStream","WritableStream","Stream","Iterable","AsyncIterable","Function","{ readable, writable } pair","Promise"],t)};function ef(e){let{promise:t,resolve:n}=Dr(),r=new Za,i=r.signal;return{value:e(async function*(){for(;;){let l=t;t=null;let{chunk:u,done:f,cb:a}=await l;if(It.nextTick(a),f)return;if(i.aborted)throw new Lr(void 0,{cause:i.reason});({promise:t,resolve:n}=Dr()),yield u}}(),{signal:i}),write(l,u,f){let a=n;n=null,a({chunk:l,done:!1,cb:f})},final(l){let u=n;n=null,u({done:!0,cb:l})},destroy(l,u){r.abort(),u(l)}}}function Ge(e){let t=e.readable&&typeof e.readable.read!="function"?Ja.wrap(e.readable):e.readable,n=e.writable,r=!!Ha(t),i=!!Va(n),o,l,u,f,a;function c(s){let b=f;f=null,b?b(s):s?a.destroy(s):!r&&!i&&a.destroy()}return a=new ne({readableObjectMode:!!(t!=null&&t.readableObjectMode),writableObjectMode:!!(n!=null&&n.writableObjectMode),readable:r,writable:i}),i&&(Mr(n,s=>{i=!1,s&&ce(t,s),c(s)}),a._write=function(s,b,d){n.write(s,b)?d():o=d},a._final=function(s){n.end(),l=s},n.on("drain",function(){if(o){let s=o;o=null,s()}}),n.on("finish",function(){if(l){let s=l;l=null,s()}})),r&&(Mr(t,s=>{r=!1,s&&ce(t,s),c(s)}),t.on("readable",function(){if(u){let s=u;u=null,s()}}),t.on("end",function(){a.push(null)}),a._read=function(){for(;;){let s=t.read();if(s===null){u=a._read;return}if(!a.push(s))return}}),a._destroy=function(s,b){!s&&f!==null&&(s=new Lr),u=null,o=null,l=null,f===null?b(s):(f=b,ce(n,s),ce(t,s))},a}});var v=g((su,jr)=>{"use strict";var{ObjectDefineProperties:tf,ObjectGetOwnPropertyDescriptor:B,ObjectKeys:nf,ObjectSetPrototypeOf:Wr}=m();jr.exports=C;var Dt=we(),x=Tt();Wr(C.prototype,Dt.prototype);Wr(C,Dt);{let e=nf(x.prototype);for(let t=0;t<e.length;t++){let n=e[t];C.prototype[n]||(C.prototype[n]=x.prototype[n])}}function C(e){if(!(this instanceof C))return new C(e);Dt.call(this,e),x.call(this,e),e?(this.allowHalfOpen=e.allowHalfOpen!==!1,e.readable===!1&&(this._readableState.readable=!1,this._readableState.ended=!0,this._readableState.endEmitted=!0),e.writable===!1&&(this._writableState.writable=!1,this._writableState.ending=!0,this._writableState.ended=!0,this._writableState.finished=!0)):this.allowHalfOpen=!0}tf(C.prototype,{writable:{__proto__:null,...B(x.prototype,"writable")},writableHighWaterMark:{__proto__:null,...B(x.prototype,"writableHighWaterMark")},writableObjectMode:{__proto__:null,...B(x.prototype,"writableObjectMode")},writableBuffer:{__proto__:null,...B(x.prototype,"writableBuffer")},writableLength:{__proto__:null,...B(x.prototype,"writableLength")},writableFinished:{__proto__:null,...B(x.prototype,"writableFinished")},writableCorked:{__proto__:null,...B(x.prototype,"writableCorked")},writableEnded:{__proto__:null,...B(x.prototype,"writableEnded")},writableNeedDrain:{__proto__:null,...B(x.prototype,"writableNeedDrain")},destroyed:{__proto__:null,get(){return this._readableState===void 0||this._writableState===void 0?!1:this._readableState.destroyed&&this._writableState.destroyed},set(e){this._readableState&&this._writableState&&(this._readableState.destroyed=e,this._writableState.destroyed=e)}}});var Mt;function Cr(){return Mt===void 0&&(Mt={}),Mt}C.fromWeb=function(e,t){return Cr().newStreamDuplexFromReadableWritablePair(e,t)};C.toWeb=function(e){return Cr().newReadableWritablePairFromDuplex(e)};var Nt;C.from=function(e){return Nt||(Nt=kr()),Nt(e,"body")}});var xt=g((du,vr)=>{"use strict";var{ObjectSetPrototypeOf:$r,Symbol:rf}=m();vr.exports=G;var{ERR_METHOD_NOT_IMPLEMENTED:of}=O().codes,qt=v(),{getHighWaterMark:lf}=Ce();$r(G.prototype,qt.prototype);$r(G,qt);var Ee=rf("kCallback");function G(e){if(!(this instanceof G))return new G(e);let t=e?lf(this,e,"readableHighWaterMark",!0):null;t===0&&(e={...e,highWaterMark:null,readableHighWaterMark:t,writableHighWaterMark:e.writableHighWaterMark||0}),qt.call(this,e),this._readableState.sync=!1,this[Ee]=null,e&&(typeof e.transform=="function"&&(this._transform=e.transform),typeof e.flush=="function"&&(this._flush=e.flush)),this.on("prefinish",af)}function Ot(e){typeof this._flush=="function"&&!this.destroyed?this._flush((t,n)=>{if(t){e?e(t):this.destroy(t);return}n!=null&&this.push(n),this.push(null),e&&e()}):(this.push(null),e&&e())}function af(){this._final!==Ot&&Ot.call(this)}G.prototype._final=Ot;G.prototype._transform=function(e,t,n){throw new of("_transform()")};G.prototype._write=function(e,t,n){let r=this._readableState,i=this._writableState,o=r.length;this._transform(e,t,(l,u)=>{if(l){n(l);return}u!=null&&this.push(u),i.ended||o===r.length||r.length<r.highWaterMark?n():this[Ee]=n})};G.prototype._read=function(){if(this[Ee]){let e=this[Ee];this[Ee]=null,e()}}});var Pt=g((cu,Ur)=>{"use strict";var{ObjectSetPrototypeOf:Fr}=m();Ur.exports=he;var Lt=xt();Fr(he.prototype,Lt.prototype);Fr(he,Lt);function he(e){if(!(this instanceof he))return new he(e);Lt.call(this,e)}he.prototype._transform=function(e,t,n){n(null,e)}});var Ye=g((hu,zr)=>{var He=__process$,{ArrayIsArray:ff,Promise:uf,SymbolAsyncIterator:sf}=m(),Ve=Y(),{once:df}=j(),cf=Z(),Br=v(),{aggregateTwoErrors:hf,codes:{ERR_INVALID_ARG_TYPE:Yr,ERR_INVALID_RETURN_VALUE:kt,ERR_MISSING_ARGS:bf,ERR_STREAM_DESTROYED:_f,ERR_STREAM_PREMATURE_CLOSE:pf},AbortError:wf}=O(),{validateFunction:yf,validateAbortSignal:gf}=_e(),{isIterable:be,isReadable:Wt,isReadableNodeStream:$t,isNodeStream:Gr}=V(),Sf=AbortController,Ct,jt;function Hr(e,t,n){let r=!1;e.on("close",()=>{r=!0});let i=Ve(e,{readable:t,writable:n},o=>{r=!o});return{destroy:o=>{r||(r=!0,cf.destroyer(e,o||new _f("pipe")))},cleanup:i}}function Ef(e){return yf(e[e.length-1],"streams[stream.length - 1]"),e.pop()}function Rf(e){if(be(e))return e;if($t(e))return Af(e);throw new Yr("val",["Readable","Iterable","AsyncIterable"],e)}async function*Af(e){jt||(jt=we()),yield*jt.prototype[sf].call(e)}async function Vr(e,t,n,{end:r}){let i,o=null,l=a=>{if(a&&(i=a),o){let c=o;o=null,c()}},u=()=>new uf((a,c)=>{i?c(i):o=()=>{i?c(i):a()}});t.on("drain",l);let f=Ve(t,{readable:!1},l);try{t.writableNeedDrain&&await u();for await(let a of e)t.write(a)||await u();r&&t.end(),await u(),n()}catch(a){n(i!==a?hf(i,a):a)}finally{f(),t.off("drain",l)}}function mf(...e){return Kr(e,df(Ef(e)))}function Kr(e,t,n){if(e.length===1&&ff(e[0])&&(e=e[0]),e.length<2)throw new bf("streams");let r=new Sf,i=r.signal,o=n?.signal,l=[];gf(o,"options.signal");function u(){d(new wf)}o?.addEventListener("abort",u);let f,a,c=[],s=0;function b(_){d(_,--s===0)}function d(_,p){if(_&&(!f||f.code==="ERR_STREAM_PREMATURE_CLOSE")&&(f=_),!(!f&&!p)){for(;c.length;)c.shift()(f);o?.removeEventListener("abort",u),r.abort(),p&&(f||l.forEach(I=>I()),He.nextTick(t,f,a))}}let h;for(let _=0;_<e.length;_++){let p=e[_],I=_<e.length-1,M=_>0,F=I||n?.end!==!1,re=_===e.length-1;if(Gr(p)){let P=function(U){U&&U.name!=="AbortError"&&U.code!=="ERR_STREAM_PREMATURE_CLOSE"&&b(U)};var L=P;if(F){let{destroy:U,cleanup:ze}=Hr(p,I,M);c.push(U),Wt(p)&&re&&l.push(ze)}p.on("error",P),Wt(p)&&re&&l.push(()=>{p.removeListener("error",P)})}if(_===0)if(typeof p=="function"){if(h=p({signal:i}),!be(h))throw new kt("Iterable, AsyncIterable or Stream","source",h)}else be(p)||$t(p)?h=p:h=Br.from(p);else if(typeof p=="function")if(h=Rf(h),h=p(h,{signal:i}),I){if(!be(h,!0))throw new kt("AsyncIterable",`transform[${_-1}]`,h)}else{var D;Ct||(Ct=Pt());let P=new Ct({objectMode:!0}),U=(D=h)===null||D===void 0?void 0:D.then;if(typeof U=="function")s++,U.call(h,ie=>{a=ie,ie!=null&&P.write(ie),F&&P.end(),He.nextTick(b)},ie=>{P.destroy(ie),He.nextTick(b,ie)});else if(be(h,!0))s++,Vr(h,P,b,{end:F});else throw new kt("AsyncIterable or Promise","destination",h);h=P;let{destroy:ze,cleanup:_i}=Hr(h,!1,!0);c.push(ze),re&&l.push(_i)}else if(Gr(p)){if($t(h)){s+=2;let P=Tf(h,p,b,{end:F});Wt(p)&&re&&l.push(P)}else if(be(h))s++,Vr(h,p,b,{end:F});else throw new Yr("val",["Readable","Iterable","AsyncIterable"],h);h=p}else h=Br.from(p)}return(i!=null&&i.aborted||o!=null&&o.aborted)&&He.nextTick(u),h}function Tf(e,t,n,{end:r}){let i=!1;return t.on("close",()=>{i||n(new pf)}),e.pipe(t,{end:r}),r?e.once("end",()=>{i=!0,t.end()}):n(),Ve(e,{readable:!0,writable:!1},o=>{let l=e._readableState;o&&o.code==="ERR_STREAM_PREMATURE_CLOSE"&&l&&l.ended&&!l.errored&&!l.errorEmitted?e.once("end",n).once("error",n):n(o)}),Ve(t,{readable:!1,writable:!0},n)}zr.exports={pipelineImpl:Kr,pipeline:mf}});var ei=g((bu,Zr)=>{"use strict";var{pipeline:If}=Ye(),Ke=v(),{destroyer:Mf}=Z(),{isNodeStream:Nf,isReadable:Xr,isWritable:Jr}=V(),{AbortError:Df,codes:{ERR_INVALID_ARG_VALUE:Qr,ERR_MISSING_ARGS:Of}}=O();Zr.exports=function(...t){if(t.length===0)throw new Of("streams");if(t.length===1)return Ke.from(t[0]);let n=[...t];if(typeof t[0]=="function"&&(t[0]=Ke.from(t[0])),typeof t[t.length-1]=="function"){let d=t.length-1;t[d]=Ke.from(t[d])}for(let d=0;d<t.length;++d)if(!!Nf(t[d])){if(d<t.length-1&&!Xr(t[d]))throw new Qr(`streams[${d}]`,n[d],"must be readable");if(d>0&&!Jr(t[d]))throw new Qr(`streams[${d}]`,n[d],"must be writable")}let r,i,o,l,u;function f(d){let h=l;l=null,h?h(d):d?u.destroy(d):!b&&!s&&u.destroy()}let a=t[0],c=If(t,f),s=!!Jr(a),b=!!Xr(c);return u=new Ke({writableObjectMode:!!(a!=null&&a.writableObjectMode),readableObjectMode:!!(c!=null&&c.writableObjectMode),writable:s,readable:b}),s&&(u._write=function(d,h,D){a.write(d,h)?D():r=D},u._final=function(d){a.end(),i=d},a.on("drain",function(){if(r){let d=r;r=null,d()}}),c.on("finish",function(){if(i){let d=i;i=null,d()}})),b&&(c.on("readable",function(){if(o){let d=o;o=null,d()}}),c.on("end",function(){u.push(null)}),u._read=function(){for(;;){let d=c.read();if(d===null){o=u._read;return}if(!u.push(d))return}}),u._destroy=function(d,h){!d&&l!==null&&(d=new Df),o=null,r=null,i=null,l===null?h(d):(l=h,Mf(c,d))},u}});var vt=g((_u,ti)=>{"use strict";var{ArrayPrototypePop:qf,Promise:xf}=m(),{isIterable:Lf,isNodeStream:Pf}=V(),{pipelineImpl:kf}=Ye(),{finished:Wf}=Y();function Cf(...e){return new xf((t,n)=>{let r,i,o=e[e.length-1];if(o&&typeof o=="object"&&!Pf(o)&&!Lf(o)){let l=qf(e);r=l.signal,i=l.end}kf(e,(l,u)=>{l?n(l):t(u)},{signal:r,end:i})})}ti.exports={finished:Wf,pipeline:Cf}});var di=g((pu,si)=>{var{Buffer:jf}=__buffer$,{ObjectDefineProperty:H,ObjectKeys:ii,ReflectApply:oi}=m(),{promisify:{custom:li}}=j(),{streamReturningOperators:ni,promiseReturningOperators:ri}=xn(),{codes:{ERR_ILLEGAL_CONSTRUCTOR:ai}}=O(),$f=ei(),{pipeline:fi}=Ye(),{destroyer:vf}=Z(),ui=Y(),Ft=vt(),Ut=V(),R=si.exports=Le().Stream;R.isDisturbed=Ut.isDisturbed;R.isErrored=Ut.isErrored;R.isReadable=Ut.isReadable;R.Readable=we();for(let e of ii(ni)){let n=function(...r){if(new.target)throw ai();return R.Readable.from(oi(t,this,r))};Uf=n;let t=ni[e];H(n,"name",{__proto__:null,value:t.name}),H(n,"length",{__proto__:null,value:t.length}),H(R.Readable.prototype,e,{__proto__:null,value:n,enumerable:!1,configurable:!0,writable:!0})}var Uf;for(let e of ii(ri)){let n=function(...i){if(new.target)throw ai();return oi(t,this,i)};Uf=n;let t=ri[e];H(n,"name",{__proto__:null,value:t.name}),H(n,"length",{__proto__:null,value:t.length}),H(R.Readable.prototype,e,{__proto__:null,value:n,enumerable:!1,configurable:!0,writable:!0})}var Uf;R.Writable=Tt();R.Duplex=v();R.Transform=xt();R.PassThrough=Pt();R.pipeline=fi;var{addAbortSignal:Ff}=ke();R.addAbortSignal=Ff;R.finished=ui;R.destroy=vf;R.compose=$f;H(R,"promises",{__proto__:null,configurable:!0,enumerable:!0,get(){return Ft}});H(fi,li,{__proto__:null,enumerable:!0,get(){return Ft.pipeline}});H(ui,li,{__proto__:null,enumerable:!0,get(){return Ft.finished}});R.Stream=R;R._isUint8Array=function(t){return t instanceof Uint8Array};R._uint8ArrayToBuffer=function(t){return jf.from(t.buffer,t.byteOffset,t.byteLength)}});var ci=g((wu,A)=>{"use strict";var T=di(),Bf=vt(),Gf=T.Readable.destroy;A.exports=T.Readable;A.exports._uint8ArrayToBuffer=T._uint8ArrayToBuffer;A.exports._isUint8Array=T._isUint8Array;A.exports.isDisturbed=T.isDisturbed;A.exports.isErrored=T.isErrored;A.exports.isReadable=T.isReadable;A.exports.Readable=T.Readable;A.exports.Writable=T.Writable;A.exports.Duplex=T.Duplex;A.exports.Transform=T.Transform;A.exports.PassThrough=T.PassThrough;A.exports.addAbortSignal=T.addAbortSignal;A.exports.finished=T.finished;A.exports.destroy=T.destroy;A.exports.destroy=Gf;A.exports.pipeline=T.pipeline;A.exports.compose=T.compose;Object.defineProperty(T,"promises",{configurable:!0,enumerable:!0,get(){return Bf}});A.exports.Stream=T.Stream;A.exports.default=A.exports});var bi=Ri(ci()),{_uint8ArrayToBuffer:yu,_isUint8Array:gu,isDisturbed:Su,isErrored:Eu,isReadable:Ru,Readable:Au,Writable:mu,Duplex:Tu,Transform:Iu,PassThrough:Mu,addAbortSignal:Nu,finished:Du,destroy:Ou,pipeline:qu,compose:xu,Stream:Lu}=bi,{default:hi,...Hf}=bi,Pu=hi!==void 0?hi:Hf;export{Tu as Duplex,Mu as PassThrough,Au as Readable,Lu as Stream,Iu as Transform,mu as Writable,gu as _isUint8Array,yu as _uint8ArrayToBuffer,Nu as addAbortSignal,xu as compose,Pu as default,Ou as destroy,Du as finished,Su as isDisturbed,Eu as isErrored,Ru as isReadable,qu as pipeline};
/* End esm.sh bundle */

// The following code implements Readable.fromWeb(), Writable.fromWeb(), and
// Duplex.fromWeb(). These functions are not properly implemented in the
// readable-stream module yet. This can be removed once the following upstream
// issue is resolved: https://github.com/nodejs/readable-stream/issues/482

import {
  AbortError,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_STREAM_PREMATURE_CLOSE,
} from "ext:deno_node/internal/errors.ts";
import { destroy } from "ext:deno_node/internal/streams/destroy.mjs";
import finished from "ext:deno_node/internal/streams/end-of-stream.mjs";
import {
  isDestroyed,
  isReadable,
  isReadableEnded,
  isWritable,
  isWritableEnded,
} from "ext:deno_node/internal/streams/utils.mjs";
import { createDeferredPromise, kEmptyObject } from "ext:deno_node/internal/util.mjs";
import { validateBoolean, validateObject } from "ext:deno_node/internal/validators.mjs";

const process = __process$;
const { Buffer } = __buffer$;
const Readable = Au;
const Writable = mu;
const Duplex = Tu;

function isReadableStream(object) {
  return object instanceof ReadableStream;
}

function isWritableStream(object) {
  return object instanceof WritableStream;
}

Readable.fromWeb = function (
  readableStream,
  options = kEmptyObject,
) {
  if (!isReadableStream(readableStream)) {
    throw new ERR_INVALID_ARG_TYPE(
      "readableStream",
      "ReadableStream",
      readableStream,
    );
  }

  validateObject(options, "options");
  const {
    highWaterMark,
    encoding,
    objectMode = false,
    signal,
  } = options;

  if (encoding !== undefined && !Buffer.isEncoding(encoding)) {
    throw new ERR_INVALID_ARG_VALUE(encoding, "options.encoding");
  }
  validateBoolean(objectMode, "options.objectMode");

  const reader = readableStream.getReader();
  let closed = false;

  const readable = new Readable({
    objectMode,
    highWaterMark,
    encoding,
    signal,

    read() {
      reader.read().then(
        (chunk) => {
          if (chunk.done) {
            readable.push(null);
          } else {
            readable.push(chunk.value);
          }
        },
        (error) => destroy.call(readable, error),
      );
    },

    destroy(error, callback) {
      function done() {
        try {
          callback(error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => {
            throw error;
          });
        }
      }

      if (!closed) {
        reader.cancel(error).then(done, done);
        return;
      }

      done();
    },
  });

  reader.closed.then(
    () => {
      closed = true;
      if (!isReadableEnded(readable)) {
        readable.push(null);
      }
    },
    (error) => {
      closed = true;
      destroy.call(readable, error);
    },
  );

  return readable;
};

Writable.fromWeb = function (
  writableStream,
  options = kEmptyObject,
) {
  if (!isWritableStream(writableStream)) {
    throw new ERR_INVALID_ARG_TYPE(
      "writableStream",
      "WritableStream",
      writableStream,
    );
  }

  validateObject(options, "options");
  const {
    highWaterMark,
    decodeStrings = true,
    objectMode = false,
    signal,
  } = options;

  validateBoolean(objectMode, "options.objectMode");
  validateBoolean(decodeStrings, "options.decodeStrings");

  const writer = writableStream.getWriter();
  let closed = false;

  const writable = new Writable({
    highWaterMark,
    objectMode,
    decodeStrings,
    signal,

    writev(chunks, callback) {
      function done(error) {
        error = error.filter((e) => e);
        try {
          callback(error.length === 0 ? undefined : error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => destroy.call(writable, error));
        }
      }

      writer.ready.then(
        () =>
          Promise.all(
            chunks.map((data) => writer.write(data.chunk)),
          ).then(done, done),
        done,
      );
    },

    write(chunk, encoding, callback) {
      if (typeof chunk === "string" && decodeStrings && !objectMode) {
        chunk = Buffer.from(chunk, encoding);
        chunk = new Uint8Array(
          chunk.buffer,
          chunk.byteOffset,
          chunk.byteLength,
        );
      }

      function done(error) {
        try {
          callback(error);
        } catch (error) {
          destroy(this, duplex, error);
        }
      }

      writer.ready.then(
        () => writer.write(chunk).then(done, done),
        done,
      );
    },

    destroy(error, callback) {
      function done() {
        try {
          callback(error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => {
            throw error;
          });
        }
      }

      if (!closed) {
        if (error != null) {
          writer.abort(error).then(done, done);
        } else {
          writer.close().then(done, done);
        }
        return;
      }

      done();
    },

    final(callback) {
      function done(error) {
        try {
          callback(error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => destroy.call(writable, error));
        }
      }

      if (!closed) {
        writer.close().then(done, done);
      }
    },
  });

  writer.closed.then(
    () => {
      closed = true;
      if (!isWritableEnded(writable)) {
        destroy.call(writable, new ERR_STREAM_PREMATURE_CLOSE());
      }
    },
    (error) => {
      closed = true;
      destroy.call(writable, error);
    },
  );

  return writable;
};

Duplex.fromWeb = function (pair, options = kEmptyObject) {
  validateObject(pair, "pair");
  const {
    readable: readableStream,
    writable: writableStream,
  } = pair;

  if (!isReadableStream(readableStream)) {
    throw new ERR_INVALID_ARG_TYPE(
      "pair.readable",
      "ReadableStream",
      readableStream,
    );
  }
  if (!isWritableStream(writableStream)) {
    throw new ERR_INVALID_ARG_TYPE(
      "pair.writable",
      "WritableStream",
      writableStream,
    );
  }

  validateObject(options, "options");
  const {
    allowHalfOpen = false,
    objectMode = false,
    encoding,
    decodeStrings = true,
    highWaterMark,
    signal,
  } = options;

  validateBoolean(objectMode, "options.objectMode");
  if (encoding !== undefined && !Buffer.isEncoding(encoding)) {
    throw new ERR_INVALID_ARG_VALUE(encoding, "options.encoding");
  }

  const writer = writableStream.getWriter();
  const reader = readableStream.getReader();
  let writableClosed = false;
  let readableClosed = false;

  const duplex = new Duplex({
    allowHalfOpen,
    highWaterMark,
    objectMode,
    encoding,
    decodeStrings,
    signal,

    writev(chunks, callback) {
      function done(error) {
        error = error.filter((e) => e);
        try {
          callback(error.length === 0 ? undefined : error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => destroy(duplex, error));
        }
      }

      writer.ready.then(
        () =>
          Promise.all(
            chunks.map((data) => writer.write(data.chunk)),
          ).then(done, done),
        done,
      );
    },

    write(chunk, encoding, callback) {
      if (typeof chunk === "string" && decodeStrings && !objectMode) {
        chunk = Buffer.from(chunk, encoding);
        chunk = new Uint8Array(
          chunk.buffer,
          chunk.byteOffset,
          chunk.byteLength,
        );
      }

      function done(error) {
        try {
          callback(error);
        } catch (error) {
          destroy(duplex, error);
        }
      }

      writer.ready.then(
        () => writer.write(chunk).then(done, done),
        done,
      );
    },

    final(callback) {
      function done(error) {
        try {
          callback(error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => destroy(duplex, error));
        }
      }

      if (!writableClosed) {
        writer.close().then(done, done);
      }
    },

    read() {
      reader.read().then(
        (chunk) => {
          if (chunk.done) {
            duplex.push(null);
          } else {
            duplex.push(chunk.value);
          }
        },
        (error) => destroy(duplex, error),
      );
    },

    destroy(error, callback) {
      function done() {
        try {
          callback(error);
        } catch (error) {
          // In a next tick because this is happening within
          // a promise context, and if there are any errors
          // thrown we don't want those to cause an unhandled
          // rejection. Let's just escape the promise and
          // handle it separately.
          process.nextTick(() => {
            throw error;
          });
        }
      }

      async function closeWriter() {
        if (!writableClosed) {
          await writer.abort(error);
        }
      }

      async function closeReader() {
        if (!readableClosed) {
          await reader.cancel(error);
        }
      }

      if (!writableClosed || !readableClosed) {
        Promise.all([
          closeWriter(),
          closeReader(),
        ]).then(done, done);
        return;
      }

      done();
    },
  });

  writer.closed.then(
    () => {
      writableClosed = true;
      if (!isWritableEnded(duplex)) {
        destroy(duplex, new ERR_STREAM_PREMATURE_CLOSE());
      }
    },
    (error) => {
      writableClosed = true;
      readableClosed = true;
      destroy(duplex, error);
    },
  );

  reader.closed.then(
    () => {
      readableClosed = true;
      if (!isReadableEnded(duplex)) {
        duplex.push(null);
      }
    },
    (error) => {
      writableClosed = true;
      readableClosed = true;
      destroy(duplex, error);
    },
  );

  return duplex;
};

// readable-stream attaches these to Readable, but Node.js core does not.
// Delete them here to better match Node.js core. These can be removed once
// https://github.com/nodejs/readable-stream/issues/485 is resolved.
delete Readable.Duplex;
delete Readable.PassThrough;
delete Readable.Readable;
delete Readable.Stream;
delete Readable.Transform;
delete Readable.Writable;
delete Readable._isUint8Array;
delete Readable._uint8ArrayToBuffer;
delete Readable.addAbortSignal;
delete Readable.compose;
delete Readable.destroy;
delete Readable.finished;
delete Readable.isDisturbed;
delete Readable.isErrored;
delete Readable.isReadable;
delete Readable.pipeline;

// The following code implements Readable.toWeb(), Writable.toWeb(), and
// Duplex.toWeb(). These functions are not properly implemented in the
// readable-stream module yet. This can be removed once the following upstream
// issue is resolved: https://github.com/nodejs/readable-stream/issues/482
function newReadableStreamFromStreamReadable(
  streamReadable,
  options = kEmptyObject,
) {
  // Not using the internal/streams/utils isReadableNodeStream utility
  // here because it will return false if streamReadable is a Duplex
  // whose readable option is false. For a Duplex that is not readable,
  // we want it to pass this check but return a closed ReadableStream.
  if (typeof streamReadable?._readableState !== "object") {
    throw new ERR_INVALID_ARG_TYPE(
      "streamReadable",
      "stream.Readable",
      streamReadable,
    );
  }

  if (isDestroyed(streamReadable) || !isReadable(streamReadable)) {
    const readable = new ReadableStream();
    readable.cancel();
    return readable;
  }

  const objectMode = streamReadable.readableObjectMode;
  const highWaterMark = streamReadable.readableHighWaterMark;

  const evaluateStrategyOrFallback = (strategy) => {
    // If there is a strategy available, use it
    if (strategy) {
      return strategy;
    }

    if (objectMode) {
      // When running in objectMode explicitly but no strategy, we just fall
      // back to CountQueuingStrategy
      return new CountQueuingStrategy({ highWaterMark });
    }

    // When not running in objectMode explicitly, we just fall
    // back to a minimal strategy that just specifies the highWaterMark
    // and no size algorithm. Using a ByteLengthQueuingStrategy here
    // is unnecessary.
    return { highWaterMark };
  };

  const strategy = evaluateStrategyOrFallback(options?.strategy);

  let controller;

  function onData(chunk) {
    // Copy the Buffer to detach it from the pool.
    if (Buffer.isBuffer(chunk) && !objectMode) {
      chunk = new Uint8Array(chunk);
    }
    controller.enqueue(chunk);
    if (controller.desiredSize <= 0) {
      streamReadable.pause();
    }
  }

  streamReadable.pause();

  const cleanup = finished(streamReadable, (error) => {
    if (error?.code === "ERR_STREAM_PREMATURE_CLOSE") {
      const err = new AbortError(undefined, { cause: error });
      error = err;
    }

    cleanup();
    // This is a protection against non-standard, legacy streams
    // that happen to emit an error event again after finished is called.
    streamReadable.on("error", () => {});
    if (error) {
      return controller.error(error);
    }
    controller.close();
  });

  streamReadable.on("data", onData);

  return new ReadableStream({
    start(c) {
      controller = c;
    },

    pull() {
      streamReadable.resume();
    },

    cancel(reason) {
      destroy(streamReadable, reason);
    },
  }, strategy);
}

function newWritableStreamFromStreamWritable(streamWritable) {
  // Not using the internal/streams/utils isWritableNodeStream utility
  // here because it will return false if streamWritable is a Duplex
  // whose writable option is false. For a Duplex that is not writable,
  // we want it to pass this check but return a closed WritableStream.
  if (typeof streamWritable?._writableState !== "object") {
    throw new ERR_INVALID_ARG_TYPE(
      "streamWritable",
      "stream.Writable",
      streamWritable,
    );
  }

  if (isDestroyed(streamWritable) || !isWritable(streamWritable)) {
    const writable = new WritableStream();
    writable.close();
    return writable;
  }

  const highWaterMark = streamWritable.writableHighWaterMark;
  const strategy = streamWritable.writableObjectMode
    ? new CountQueuingStrategy({ highWaterMark })
    : { highWaterMark };

  let controller;
  let backpressurePromise;
  let closed;

  function onDrain() {
    if (backpressurePromise !== undefined) {
      backpressurePromise.resolve();
    }
  }

  const cleanup = finished(streamWritable, (error) => {
    if (error?.code === "ERR_STREAM_PREMATURE_CLOSE") {
      const err = new AbortError(undefined, { cause: error });
      error = err;
    }

    cleanup();
    // This is a protection against non-standard, legacy streams
    // that happen to emit an error event again after finished is called.
    streamWritable.on("error", () => {});
    if (error != null) {
      if (backpressurePromise !== undefined) {
        backpressurePromise.reject(error);
      }
      // If closed is not undefined, the error is happening
      // after the WritableStream close has already started.
      // We need to reject it here.
      if (closed !== undefined) {
        closed.reject(error);
        closed = undefined;
      }
      controller.error(error);
      controller = undefined;
      return;
    }

    if (closed !== undefined) {
      closed.resolve();
      closed = undefined;
      return;
    }
    controller.error(new AbortError());
    controller = undefined;
  });

  streamWritable.on("drain", onDrain);

  return new WritableStream({
    start(c) {
      controller = c;
    },

    async write(chunk) {
      if (streamWritable.writableNeedDrain || !streamWritable.write(chunk)) {
        backpressurePromise = createDeferredPromise();
        return backpressurePromise.promise.finally(() => {
          backpressurePromise = undefined;
        });
      }
    },

    abort(reason) {
      destroy(streamWritable, reason);
    },

    close() {
      if (closed === undefined && !isWritableEnded(streamWritable)) {
        closed = createDeferredPromise();
        streamWritable.end();
        return closed.promise;
      }

      controller = undefined;
      return Promise.resolve();
    },
  }, strategy);
}

function newReadableWritablePairFromDuplex(duplex) {
  // Not using the internal/streams/utils isWritableNodeStream and
  // isReadableNodestream utilities here because they will return false
  // if the duplex was created with writable or readable options set to
  // false. Instead, we'll check the readable and writable state after
  // and return closed WritableStream or closed ReadableStream as
  // necessary.
  if (
    typeof duplex?._writableState !== "object" ||
    typeof duplex?._readableState !== "object"
  ) {
    throw new ERR_INVALID_ARG_TYPE("duplex", "stream.Duplex", duplex);
  }

  if (isDestroyed(duplex)) {
    const writable = new WritableStream();
    const readable = new ReadableStream();
    writable.close();
    readable.cancel();
    return { readable, writable };
  }

  const writable = isWritable(duplex)
    ? newWritableStreamFromStreamWritable(duplex)
    : new WritableStream();

  if (!isWritable(duplex)) {
    writable.close();
  }

  const readable = isReadable(duplex)
    ? newReadableStreamFromStreamReadable(duplex)
    : new ReadableStream();

  if (!isReadable(duplex)) {
    readable.cancel();
  }

  return { writable, readable };
}

Readable.toWeb = newReadableStreamFromStreamReadable;
Writable.toWeb = newWritableStreamFromStreamWritable;
Duplex.toWeb = newReadableWritablePairFromDuplex;
