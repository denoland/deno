// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

// This module was heavily inspired by ua-parser-js
// (https://www.npmjs.com/package/ua-parser-js) which is MIT licensed and
// Copyright (c) 2012-2023 Faisal Salman <f@faisalman.com>

/** Provides {@linkcode UserAgent} and related types to be able to provide a
 * structured understanding of a user agent string.
 *
 * @module
 */

import { assert } from "../assert/assert.ts";

const ARCHITECTURE = "architecture";
const MODEL = "model";
const NAME = "name";
const TYPE = "type";
const VENDOR = "vendor";
const VERSION = "version";
const EMPTY = "";

const CONSOLE = "console";
const EMBEDDED = "embedded";
const MOBILE = "mobile";
const TABLET = "tablet";
const SMARTTV = "smarttv";
const WEARABLE = "wearable";

const PREFIX_MOBILE = "Mobile ";
const SUFFIX_BROWSER = " Browser";

const AMAZON = "Amazon";
const APPLE = "Apple";
const ASUS = "ASUS";
const BLACKBERRY = "BlackBerry";
const CHROME = "Chrome";
const EDGE = "Edge";
const FACEBOOK = "Facebook";
const FIREFOX = "Firefox";
const GOOGLE = "Google";
const HUAWEI = "Huawei";
const LG = "LG";
const MICROSOFT = "Microsoft";
const MOTOROLA = "Motorola";
const OPERA = "Opera";
const SAMSUNG = "Samsung";
const SHARP = "Sharp";
const SONY = "Sony";
const WINDOWS = "Windows";
const XIAOMI = "Xiaomi";
const ZEBRA = "Zebra";

type ProcessingFn = (value: string) => string | undefined;

type MatchingTuple = [matchers: [RegExp, ...RegExp[]], processors: (
  string | [string, string] | [string, ProcessingFn] | [
    string,
    RegExp,
    string,
    ProcessingFn?,
  ]
)[]];

interface Matchers {
  browser: MatchingTuple[];
  cpu: MatchingTuple[];
  device: MatchingTuple[];
  engine: MatchingTuple[];
  os: MatchingTuple[];
}

export interface Browser {
  /** The major version of a browser as represented by a user agent string. */
  readonly major: string | undefined;
  /** The name of a browser as represented by a user agent string. */
  readonly name: string | undefined;
  /** The version of a browser as represented by a user agent string. */
  readonly version: string | undefined;
}

export interface Device {
  /** The model of a device as represented by a user agent string. */
  readonly model: string | undefined;
  /** The type of device as represented by a user agent string. */
  readonly type:
    | "console"
    | "mobile"
    | "table"
    | "smartv"
    | "wearable"
    | "embedded"
    | undefined;
  /** The vendor of a device as represented by a user agent string. */
  readonly vendor: string | undefined;
}

export interface Engine {
  readonly name: string | undefined;
  readonly version: string | undefined;
}

export interface Os {
  readonly name: string | undefined;
  readonly version: string | undefined;
}

export interface Cpu {
  readonly architecture: string | undefined;
}

function lowerize(str: string): string {
  return str.toLowerCase();
}

function majorize(str: string | undefined): string | undefined {
  return str ? str.replace(/[^\d\.]/g, EMPTY).split(".")[0] : undefined;
}

function trim(str: string): string {
  return str.trimStart();
}

/** A map where the key is the common Windows version and the value is a string
 * or array of strings of potential values parsed from the user-agent string. */
const windowsVersionMap = new Map<string, string | string[]>([
  ["ME", "4.90"],
  ["NT 3.11", "NT3.51"],
  ["NT 4.0", "NT4.0"],
  ["2000", "NT 5.0"],
  ["XP", ["NT 5.1", "NT 5.2"]],
  ["Vista", "NT 6.0"],
  ["7", "NT 6.1"],
  ["8", "NT 6.2"],
  ["8.1", "NT 6.3"],
  ["10", ["NT 6.4", "NT 10.0"]],
  ["RT", "ARM"],
]);

function has(str1: string | string[], str2: string): boolean {
  if (Array.isArray(str1)) {
    for (const el of str1) {
      if (lowerize(el) === lowerize(str2)) {
        return true;
      }
    }
    return false;
  }
  return lowerize(str2).indexOf(lowerize(str1)) !== -1;
}

function mapWinVer(str: string) {
  for (const [key, value] of windowsVersionMap) {
    if (Array.isArray(value)) {
      for (const v of value) {
        if (has(v, str)) {
          return key;
        }
      }
    } else if (has(value, str)) {
      return key;
    }
  }
  return str || undefined;
}

function mapper(
  // deno-lint-ignore no-explicit-any
  target: any,
  ua: string,
  tuples: MatchingTuple[],
): void {
  let matches: RegExpExecArray | null = null;
  for (const [matchers, processors] of tuples) {
    let j = 0;
    let k = 0;
    while (j < matchers.length && !matches) {
      if (!matchers[j]) {
        break;
      }
      matches = matchers[j++].exec(ua);

      if (matches) {
        for (const processor of processors) {
          const match = matches[++k];
          if (Array.isArray(processor)) {
            if (processor.length === 2) {
              const [prop, value] = processor;
              if (typeof value === "function") {
                target[prop] = value.call(
                  target,
                  match,
                );
              } else {
                target[prop] = value;
              }
            } else if (processor.length === 3) {
              const [prop, re, value] = processor;
              target[prop] = match ? match.replace(re, value) : undefined;
            } else {
              const [prop, re, value, fn] = processor;
              assert(fn);
              target[prop] = match
                ? fn.call(prop, match.replace(re, value))
                : undefined;
            }
          } else {
            target[processor] = match ? match : undefined;
          }
        }
      }
    }
  }
}

/** An object with properties that are arrays of tuples which provide match
 * patterns and configuration on how to interpret the capture groups. */
const matchers: Matchers = {
  browser: [
    [
      [/\b(?:crmo|crios)\/([\w\.]+)/i], // Chrome for Android/iOS
      [VERSION, [NAME, `${PREFIX_MOBILE}${CHROME}`]],
    ],
    [
      [/edg(?:e|ios|a)?\/([\w\.]+)/i], // Microsoft Edge
      [VERSION, [NAME, "Edge"]],
    ],

    // Presto based
    [
      [
        /(opera mini)\/([-\w\.]+)/i, // Opera Mini
        /(opera [mobiletab]{3,6})\b.+version\/([-\w\.]+)/i, // Opera Mobi/Tablet
        /(opera)(?:.+version\/|[\/ ]+)([\w\.]+)/i, // Opera
      ],
      [NAME, VERSION],
    ],
    [
      [/opios[\/ ]+([\w\.]+)/i],
      [VERSION, [NAME, `${OPERA} Mini`]],
    ],
    [
      [/\bopr\/([\w\.]+)/i],
      [VERSION, [NAME, OPERA]],
    ],

    [
      [
        // Mixed
        /(kindle)\/([\w\.]+)/i, // Kindle
        /(lunascape|maxthon|netfront|jasmine|blazer)[\/ ]?([\w\.]*)/i, // Lunascape/Maxthon/Netfront/Jasmine/Blazer
        // Trident based
        /(avant |iemobile|slim)(?:browser)?[\/ ]?([\w\.]*)/i, // Avant/IEMobile/SlimBrowser
        /(ba?idubrowser)[\/ ]?([\w\.]+)/i, // Baidu Browser
        /(?:ms|\()(ie) ([\w\.]+)/i, // Internet Explorer

        // Webkit/KHTML based
        // Flock/RockMelt/Midori/Epiphany/Silk/Skyfire/Bolt/Iron/Iridium/PhantomJS/Bowser/QupZilla/Falkon/Rekonq/Puffin/Brave/Whale/QQBrowserLite/QQ//Vivaldi/DuckDuckGo
        /(flock|rockmelt|midori|epiphany|silk|skyfire|ovibrowser|bolt|iron|vivaldi|iridium|phantomjs|bowser|quark|qupzilla|falkon|rekonq|puffin|brave|whale(?!.+naver)|qqbrowserlite|qq|duckduckgo)\/([-\w\.]+)/i,
        /(heytap|ovi)browser\/([\d\.]+)/i, // HeyTap/Ovi
        /(weibo)__([\d\.]+)/i, // Weibo
      ],
      [NAME, VERSION],
    ],
    [
      [/(?:\buc? ?browser|(?:juc.+)ucweb)[\/ ]?([\w\.]+)/i],
      [VERSION, [NAME, "UCBrowser"]],
    ],
    [
      [
        /microm.+\bqbcore\/([\w\.]+)/i, // WeChat Desktop for Windows Built-in Browser
        /\bqbcore\/([\w\.]+).+microm/i,
      ],
      [VERSION, [NAME, "WeChat(Win) Desktop"]],
    ],
    [
      [/micromessenger\/([\w\.]+)/i],
      [VERSION, [NAME, "WeChat"]],
    ],
    [
      [/konqueror\/([\w\.]+)/i],
      [VERSION, [NAME, "Konqueror"]],
    ],
    [
      [/trident.+rv[: ]([\w\.]{1,9})\b.+like gecko/i],
      [VERSION, [NAME, "IE"]],
    ],
    [
      [/ya(?:search)?browser\/([\w\.]+)/i],
      [VERSION, [NAME, "Yandex"]],
    ],
    [
      [/(avast|avg)\/([\w\.]+)/i],
      [[NAME, /(.+)/, `$1 Secure${SUFFIX_BROWSER}`], VERSION],
    ],
    [
      [/\bfocus\/([\w\.]+)/i],
      [VERSION, [NAME, `${FIREFOX} Focus`]],
    ],
    [
      [/\bopt\/([\w\.]+)/i],
      [VERSION, [NAME, `${OPERA} Touch`]],
    ],
    [
      [/coc_coc\w+\/([\w\.]+)/i],
      [VERSION, [NAME, "Coc Coc"]],
    ],
    [
      [/dolfin\/([\w\.]+)/i],
      [VERSION, [NAME, "Dolphin"]],
    ],
    [
      [/coast\/([\w\.]+)/i],
      [VERSION, [NAME, `${OPERA} Coast`]],
    ],
    [
      [/miuibrowser\/([\w\.]+)/i],
      [VERSION, [NAME, `MIUI${SUFFIX_BROWSER}`]],
    ],
    [
      [/fxios\/([\w\.-]+)/i],
      [VERSION, [NAME, `${PREFIX_MOBILE}${FIREFOX}`]],
    ],
    [
      [/\bqihu|(qi?ho?o?|360)browser/i],
      [[NAME, `360${SUFFIX_BROWSER}`]],
    ],
    [
      [/(oculus|samsung|sailfish|huawei)browser\/([\w\.]+)/i],
      [[NAME, /(.+)/, "$1" + SUFFIX_BROWSER], VERSION],
    ],
    [
      [/(comodo_dragon)\/([\w\.]+)/i],
      [[NAME, /_/g, " "], VERSION],
    ],
    [
      [
        /(electron)\/([\w\.]+) safari/i, // Electron-based App
        /(tesla)(?: qtcarbrowser|\/(20\d\d\.[-\w\.]+))/i, // Tesla
        /m?(qqbrowser|baiduboxapp|2345Explorer)[\/ ]?([\w\.]+)/i,
      ],
      [NAME, VERSION],
    ],
    [
      [
        /(metasr)[\/ ]?([\w\.]+)/i, // SouGouBrowser
        /(lbbrowser)/i, // LieBao Browser
        /\[(linkedin)app\]/i, // LinkedIn App for iOS & Android
      ],
      [NAME],
    ],
    [
      [/((?:fban\/fbios|fb_iab\/fb4a)(?!.+fbav)|;fbav\/([\w\.]+);)/i],
      [[NAME, FACEBOOK], VERSION],
    ],
    [
      [
        /(kakao(?:talk|story))[\/ ]([\w\.]+)/i, // Kakao App
        /(naver)\(.*?(\d+\.[\w\.]+).*\)/i, // Naver InApp
        /safari (line)\/([\w\.]+)/i, // Line App for iOS
        /\b(line)\/([\w\.]+)\/iab/i, // Line App for Android
        /(chromium|instagram)[\/ ]([-\w\.]+)/i, // Chromium/Instagram
      ],
      [NAME, VERSION],
    ],
    [
      [/\bgsa\/([\w\.]+) .*safari\//i],
      [VERSION, [NAME, "GSA"]],
    ],
    [
      [/musical_ly(?:.+app_?version\/|_)([\w\.]+)/i],
      [VERSION, [NAME, "TikTok"]],
    ],
    [
      [/headlesschrome(?:\/([\w\.]+)| )/i],
      [VERSION, [NAME, `${CHROME} Headless`]],
    ],
    [
      [/ wv\).+(chrome)\/([\w\.]+)/i],
      [[NAME, `${CHROME} WebView`], VERSION],
    ],
    [
      [/droid.+ version\/([\w\.]+)\b.+(?:mobile safari|safari)/i],
      [VERSION, [NAME, `Android${SUFFIX_BROWSER}`]],
    ],
    [
      [/chrome\/([\w\.]+) mobile/i],
      [VERSION, [NAME, `${PREFIX_MOBILE}${CHROME}`]],
    ],
    [
      [/(chrome|omniweb|arora|[tizenoka]{5} ?browser)\/v?([\w\.]+)/i],
      [NAME, VERSION],
    ],
    [
      [/version\/([\w\.\,]+) .*mobile(?:\/\w+ | ?)safari/i],
      [VERSION, [NAME, `${PREFIX_MOBILE}Safari`]],
    ],
    [
      [/iphone .*mobile(?:\/\w+ | ?)safari/i],
      [[NAME, `${PREFIX_MOBILE}Safari`]],
    ],
    [
      [/version\/([\w\.\,]+) .*(safari)/i],
      [VERSION, NAME],
    ],
    [
      [/webkit.+?(mobile ?safari|safari)(\/[\w\.]+)/i],
      [NAME, [VERSION, "1"]],
    ],
    [
      [/(webkit|khtml)\/([\w\.]+)/i],
      [NAME, VERSION],
    ],
    [
      [/(?:mobile|tablet);.*(firefox)\/([\w\.-]+)/i],
      [[NAME, `${PREFIX_MOBILE}${FIREFOX}`], VERSION],
    ],
    [
      [/(navigator|netscape\d?)\/([-\w\.]+)/i],
      [[NAME, "Netscape"], VERSION],
    ],
    [
      [/mobile vr; rv:([\w\.]+)\).+firefox/i],
      [VERSION, [NAME, `${FIREFOX} Reality`]],
    ],
    [
      [
        /ekiohf.+(flow)\/([\w\.]+)/i, // Flow
        /(swiftfox)/i, // Swiftfox
        /(icedragon|iceweasel|camino|chimera|fennec|maemo browser|minimo|conkeror|klar)[\/ ]?([\w\.\+]+)/i,
        // IceDragon/Iceweasel/Camino/Chimera/Fennec/Maemo/Minimo/Conkeror/Klar
        /(seamonkey|k-meleon|icecat|iceape|firebird|phoenix|palemoon|basilisk|waterfox)\/([-\w\.]+)$/i,
        // Firefox/SeaMonkey/K-Meleon/IceCat/IceApe/Firebird/Phoenix
        /(firefox)\/([\w\.]+)/i, // Other Firefox-based
        /(mozilla)\/([\w\.]+) .+rv\:.+gecko\/\d+/i, // Mozilla

        // Other
        /(polaris|lynx|dillo|icab|doris|amaya|w3m|netsurf|sleipnir|obigo|mosaic|(?:go|ice|up)[\. ]?browser)[-\/ ]?v?([\w\.]+)/i,
        // Polaris/Lynx/Dillo/iCab/Doris/Amaya/w3m/NetSurf/Sleipnir/Obigo/Mosaic/Go/ICE/UP.Browser
        /(links) \(([\w\.]+)/i, // Links
        /panasonic;(viera)/i,
      ],
      [NAME, VERSION],
    ],
    [
      [/(cobalt)\/([\w\.]+)/i],
      [NAME, [VERSION, /[^\d\.]+./, EMPTY]],
    ],
  ],
  cpu: [
    [
      [/\b(?:(amd|x|x86[-_]?|wow|win)64)\b/i],
      [[ARCHITECTURE, "amd64"]],
    ],
    [
      [
        /(ia32(?=;))/i, // IA32 (quicktime)
        /((?:i[346]|x)86)[;\)]/i,
      ],
      [[ARCHITECTURE, "ia32"]],
    ],
    [
      [/\b(aarch64|arm(v?8e?l?|_?64))\b/i],
      [[ARCHITECTURE, "arm64"]],
    ],
    [
      [/windows (ce|mobile); ppc;/i],
      [[ARCHITECTURE, "arm"]],
    ],
    [
      [/((?:ppc|powerpc)(?:64)?)(?: mac|;|\))/i],
      [[ARCHITECTURE, /ower/, EMPTY, lowerize]],
    ],
    [
      [/(sun4\w)[;\)]/i],
      [[ARCHITECTURE, "sparc"]],
    ],
    [
      [/((?:avr32|ia64(?=;))|68k(?=\))|\barm(?=v(?:[1-7]|[5-7]1)l?|;|eabi)|(?=atmel )avr|(?:irix|mips|sparc)(?:64)?\b|pa-risc)/i],
      [[ARCHITECTURE, lowerize]],
    ],
  ],
  device: [
    [
      [/\b(sch-i[89]0\d|shw-m380s|sm-[ptx]\w{2,4}|gt-[pn]\d{2,4}|sgh-t8[56]9|nexus 10)/i],
      [MODEL, [VENDOR, SAMSUNG], [TYPE, TABLET]],
    ],
    [
      [
        /\b((?:s[cgp]h|gt|sm)-\w+|sc[g-]?[\d]+a?|galaxy nexus)/i,
        /samsung[- ]([-\w]+)/i,
        /sec-(sgh\w+)/i,
      ],
      [MODEL, [VENDOR, SAMSUNG], [TYPE, MOBILE]],
    ],
    [
      [/(?:\/|\()(ip(?:hone|od)[\w, ]*)(?:\/|;)/i],
      [MODEL, [VENDOR, APPLE], [TYPE, MOBILE]],
    ],
    [
      [
        /\((ipad);[-\w\),; ]+apple/i, // iPad
        /applecoremedia\/[\w\.]+ \((ipad)/i,
        /\b(ipad)\d\d?,\d\d?[;\]].+ios/i,
      ],
      [MODEL, [VENDOR, APPLE], [TYPE, TABLET]],
    ],
    [
      [/(macintosh);/i],
      [MODEL, [VENDOR, APPLE]],
    ],
    [
      [/\b(sh-?[altvz]?\d\d[a-ekm]?)/i],
      [MODEL, [VENDOR, SHARP], [TYPE, MOBILE]],
    ],
    [
      [/\b((?:ag[rs][23]?|bah2?|sht?|btv)-a?[lw]\d{2})\b(?!.+d\/s)/i],
      [MODEL, [VENDOR, HUAWEI], [TYPE, TABLET]],
    ],
    [
      [
        /(?:huawei|honor)([-\w ]+)[;\)]/i,
        /\b(nexus 6p|\w{2,4}e?-[atu]?[ln][\dx][012359c][adn]?)\b(?!.+d\/s)/i,
      ],
      [MODEL, [VENDOR, HUAWEI], [TYPE, MOBILE]],
    ],
    [
      [
        /\b(poco[\w ]+|m2\d{3}j\d\d[a-z]{2})(?: bui|\))/i, // Xiaomi POCO
        /\b; (\w+) build\/hm\1/i, // Xiaomi Hongmi 'numeric' models
        /\b(hm[-_ ]?note?[_ ]?(?:\d\w)?) bui/i, // Xiaomi Hongmi
        /\b(redmi[\-_ ]?(?:note|k)?[\w_ ]+)(?: bui|\))/i, // Xiaomi Redmi
        /\b(mi[-_ ]?(?:a\d|one|one[_ ]plus|note lte|max|cc)?[_ ]?(?:\d?\w?)[_ ]?(?:plus|se|lite)?)(?: bui|\))/i,
      ],
      [[MODEL, /_/g, " "], [VENDOR, XIAOMI], [TYPE, MOBILE]],
    ],
    [
      [/\b(mi[-_ ]?(?:pad)(?:[\w_ ]+))(?: bui|\))/i],
      [[MODEL, /_/g, " "], [VENDOR, XIAOMI], [TYPE, TABLET]],
    ],
    [
      [
        /; (\w+) bui.+ oppo/i,
        /\b(cph[12]\d{3}|p(?:af|c[al]|d\w|e[ar])[mt]\d0|x9007|a101op)\b/i,
      ],
      [MODEL, [VENDOR, "OPPO"], [TYPE, MOBILE]],
    ],
    [
      [/vivo (\w+)(?: bui|\))/i, /\b(v[12]\d{3}\w?[at])(?: bui|;)/i],
      [MODEL, [VENDOR, "Vivo"], [TYPE, MOBILE]],
    ],
    [
      [/\b(rmx[12]\d{3})(?: bui|;|\))/i],
      [MODEL, [VENDOR, "Realme"], [TYPE, MOBILE]],
    ],
    [
      [
        /\b(milestone|droid(?:[2-4x]| (?:bionic|x2|pro|razr))?:?( 4g)?)\b[\w ]+build\//i,
        /\bmot(?:orola)?[- ](\w*)/i,
        /((?:moto[\w\(\) ]+|xt\d{3,4}|nexus 6)(?= bui|\)))/i,
      ],
      [MODEL, [VENDOR, MOTOROLA], [TYPE, MOBILE]],
    ],
    [
      [/\b(mz60\d|xoom[2 ]{0,2}) build\//i],
      [MODEL, [VENDOR, MOTOROLA], [TYPE, TABLET]],
    ],
    [
      [/((?=lg)?[vl]k\-?\d{3}) bui| 3\.[-\w; ]{10}lg?-([06cv9]{3,4})/i],
      [MODEL, [VENDOR, LG], [TYPE, TABLET]],
    ],
    [
      [
        /(lm(?:-?f100[nv]?|-[\w\.]+)(?= bui|\))|nexus [45])/i,
        /\blg[-e;\/ ]+((?!browser|netcast|android tv)\w+)/i,
        /\blg-?([\d\w]+) bui/i,
      ],
      [MODEL, [VENDOR, LG], [TYPE, MOBILE]],
    ],
    [
      [
        /(ideatab[-\w ]+)/i,
        /lenovo ?(s[56]000[-\w]+|tab(?:[\w ]+)|yt[-\d\w]{6}|tb[-\d\w]{6})/i,
      ],
      [MODEL, [VENDOR, "Lenovo"], [TYPE, TABLET]],
    ],
    [
      [/(?:maemo|nokia).*(n900|lumia \d+)/i, /nokia[-_ ]?([-\w\.]*)/i],
      [[MODEL, /_/g, " "], [VENDOR, "Nokia"], [TYPE, MOBILE]],
    ],
    [
      [/(pixel c)\b/i],
      [MODEL, [VENDOR, GOOGLE], [TYPE, TABLET]],
    ],
    [
      [/droid.+; (pixel[\daxl ]{0,6})(?: bui|\))/i],
      [MODEL, [VENDOR, GOOGLE], [TYPE, MOBILE]],
    ],
    [
      [/droid.+ (a?\d[0-2]{2}so|[c-g]\d{4}|so[-gl]\w+|xq-a\w[4-7][12])(?= bui|\).+chrome\/(?![1-6]{0,1}\d\.))/i],
      [MODEL, [VENDOR, SONY], [TYPE, MOBILE]],
    ],
    [
      [/sony tablet [ps]/i, /\b(?:sony)?sgp\w+(?: bui|\))/i],
      [[MODEL, "Xperia Tablet"], [VENDOR, SONY], [TYPE, TABLET]],
    ],
    [
      [
        / (kb2005|in20[12]5|be20[12][59])\b/i,
        /(?:one)?(?:plus)? (a\d0\d\d)(?: b|\))/i,
      ],
      [MODEL, [VENDOR, "OnePlus"], [TYPE, MOBILE]],
    ],
    [
      [
        /(alexa)webm/i,
        /(kf[a-z]{2}wi|aeo[c-r]{2})( bui|\))/i, // Kindle Fire without Silk / Echo Show
        /(kf[a-z]+)( bui|\)).+silk\//i,
      ],
      [MODEL, [VENDOR, AMAZON], [TYPE, TABLET]],
    ],
    [
      [/((?:sd|kf)[0349hijorstuw]+)( bui|\)).+silk\//i],
      [[MODEL, /(.+)/g, "Fire Phone $1"], [VENDOR, AMAZON], [TYPE, MOBILE]],
    ],
    [
      [/(playbook);[-\w\),; ]+(rim)/i],
      [MODEL, VENDOR, [TYPE, TABLET]],
    ],
    [
      [/\b((?:bb[a-f]|st[hv])100-\d)/i, /\(bb10; (\w+)/i],
      [MODEL, [VENDOR, BLACKBERRY], [TYPE, MOBILE]],
    ],
    [
      [/(?:\b|asus_)(transfo[prime ]{4,10} \w+|eeepc|slider \w+|nexus 7|padfone|p00[cj])/i],
      [MODEL, [VENDOR, ASUS], [TYPE, TABLET]],
    ],
    [
      [/ (z[bes]6[027][012][km][ls]|zenfone \d\w?)\b/i],
      [MODEL, [VENDOR, ASUS], [TYPE, MOBILE]],
    ],
    [
      [/(nexus 9)/i],
      [MODEL, [VENDOR, "HTC"], [TYPE, TABLET]],
    ],
    [
      [
        /(htc)[-;_ ]{1,2}([\w ]+(?=\)| bui)|\w+)/i, // HTC
        /(zte)[- ]([\w ]+?)(?: bui|\/|\))/i,
        /(alcatel|geeksphone|nexian|panasonic(?!(?:;|\.))|sony(?!-bra))[-_ ]?([-\w]*)/i,
      ],
      [VENDOR, [MODEL, /_/g, " "], [TYPE, MOBILE]],
    ],
    [
      [/droid.+; ([ab][1-7]-?[0178a]\d\d?)/i],
      [MODEL, [VENDOR, "Acer"], [TYPE, TABLET]],
    ],
    [
      [
        /droid.+; (m[1-5] note) bui/i,
        /\bmz-([-\w]{2,})/i,
      ],
      [MODEL, [VENDOR, "Meizu"], [TYPE, MOBILE]],
    ],
    [
      [
        /(blackberry|benq|palm(?=\-)|sonyericsson|acer|asus|dell|meizu|motorola|polytron|infinix|tecno)[-_ ]?([-\w]*)/i,
        // BlackBerry/BenQ/Palm/Sony-Ericsson/Acer/Asus/Dell/Meizu/Motorola/Polytron
        /(hp) ([\w ]+\w)/i, // HP iPAQ
        /(asus)-?(\w+)/i, // Asus
        /(microsoft); (lumia[\w ]+)/i, // Microsoft Lumia
        /(lenovo)[-_ ]?([-\w]+)/i, // Lenovo
        /(jolla)/i, // Jolla
        /(oppo) ?([\w ]+) bui/i,
      ],
      [VENDOR, MODEL, [TYPE, MOBILE]],
    ],
    [
      [
        /(kobo)\s(ereader|touch)/i, // Kobo
        /(archos) (gamepad2?)/i, // Archos
        /(hp).+(touchpad(?!.+tablet)|tablet)/i, // HP TouchPad
        /(kindle)\/([\w\.]+)/i,
      ],
      [VENDOR, MODEL, [TYPE, TABLET]],
    ],
    [
      [/(surface duo)/i],
      [MODEL, [VENDOR, MICROSOFT], [TYPE, TABLET]],
    ],
    [
      [/droid [\d\.]+; (fp\du?)(?: b|\))/i],
      [MODEL, [VENDOR, "Fairphone"], [TYPE, MOBILE]],
    ],
    [
      [/(shield[\w ]+) b/i],
      [MODEL, [VENDOR, "Nvidia"], [TYPE, TABLET]],
    ],
    [
      [/(sprint) (\w+)/i],
      [VENDOR, MODEL, [TYPE, MOBILE]],
    ],
    [
      [/(kin\.[onetw]{3})/i],
      [[MODEL, /\./g, " "], [VENDOR, MICROSOFT], [TYPE, MOBILE]],
    ],
    [
      [/droid.+; ([c6]+|et5[16]|mc[239][23]x?|vc8[03]x?)\)/i],
      [MODEL, [VENDOR, ZEBRA], [TYPE, TABLET]],
    ],
    [
      [/droid.+; (ec30|ps20|tc[2-8]\d[kx])\)/i],
      [MODEL, [VENDOR, ZEBRA], [TYPE, MOBILE]],
    ],
    [
      [/smart-tv.+(samsung)/i],
      [VENDOR, [TYPE, SMARTTV]],
    ],
    [
      [/hbbtv.+maple;(\d+)/i],
      [[MODEL, /^/, "SmartTV"], [VENDOR, SAMSUNG], [TYPE, SMARTTV]],
    ],
    [
      [/(nux; netcast.+smarttv|lg (netcast\.tv-201\d|android tv))/i],
      [[VENDOR, LG], [TYPE, SMARTTV]],
    ],
    [
      [/(apple) ?tv/i],
      [VENDOR, [MODEL, `${APPLE} TV`], [TYPE, SMARTTV]],
    ],
    [
      [/crkey/i],
      [[MODEL, `${CHROME}cast`], [VENDOR, GOOGLE], [TYPE, SMARTTV]],
    ],
    [
      [/droid.+aft(\w)( bui|\))/i],
      [MODEL, [VENDOR, AMAZON], [TYPE, SMARTTV]],
    ],
    [
      [/\(dtv[\);].+(aquos)/i, /(aquos-tv[\w ]+)\)/i],
      [MODEL, [VENDOR, SHARP], [TYPE, SMARTTV]],
    ],
    [
      [/(bravia[\w ]+)( bui|\))/i],
      [MODEL, [VENDOR, SONY], [TYPE, SMARTTV]],
    ],
    [
      [/(mitv-\w{5}) bui/i],
      [MODEL, [VENDOR, XIAOMI], [TYPE, SMARTTV]],
    ],
    [
      [/Hbbtv.*(technisat) (.*);/i],
      [VENDOR, MODEL, [TYPE, SMARTTV]],
    ],
    [
      [
        /\b(roku)[\dx]*[\)\/]((?:dvp-)?[\d\.]*)/i, // Roku
        /hbbtv\/\d+\.\d+\.\d+ +\([\w\+ ]*; *([\w\d][^;]*);([^;]*)/i,
      ],
      [[VENDOR, trim], [MODEL, trim], [TYPE, SMARTTV]],
    ],
    [
      [/\b(android tv|smart[- ]?tv|opera tv|tv; rv:)\b/i],
      [[TYPE, SMARTTV]],
    ],
    [
      [
        /(ouya)/i, // Ouya
        /(nintendo) (\w+)/i,
      ],
      [VENDOR, MODEL, [TYPE, CONSOLE]],
    ],
    [
      [/droid.+; (shield) bui/i],
      [MODEL, [VENDOR, "Nvidia"], [TYPE, CONSOLE]],
    ],
    [
      [/(playstation \w+)/i],
      [MODEL, [VENDOR, SONY], [TYPE, CONSOLE]],
    ],
    [
      [/\b(xbox(?: one)?(?!; xbox))[\); ]/i],
      [MODEL, [VENDOR, MICROSOFT], [TYPE, CONSOLE]],
    ],
    [
      [/((pebble))app/i],
      [VENDOR, MODEL, [TYPE, WEARABLE]],
    ],
    [
      [/(watch)(?: ?os[,\/]|\d,\d\/)[\d\.]+/i],
      [MODEL, [VENDOR, APPLE], [TYPE, WEARABLE]],
    ],
    [
      [/droid.+; (glass) \d/i],
      [MODEL, [VENDOR, GOOGLE], [TYPE, WEARABLE]],
    ],
    [
      [/droid.+; (wt63?0{2,3})\)/i],
      [MODEL, [VENDOR, ZEBRA], [TYPE, WEARABLE]],
    ],
    [
      [/(quest( 2| pro)?)/i],
      [MODEL, [VENDOR, FACEBOOK], [TYPE, WEARABLE]],
    ],
    [
      [/(tesla)(?: qtcarbrowser|\/[-\w\.]+)/i],
      [VENDOR, [TYPE, EMBEDDED]],
    ],
    [
      [/(aeobc)\b/i],
      [MODEL, [VENDOR, AMAZON], [TYPE, EMBEDDED]],
    ],
    [
      [/droid .+?; ([^;]+?)(?: bui|\) applew).+? mobile safari/i],
      [MODEL, [TYPE, MOBILE]],
    ],
    [
      [/droid .+?; ([^;]+?)(?: bui|\) applew).+?(?! mobile) safari/i],
      [MODEL, [TYPE, TABLET]],
    ],
    [
      [/\b((tablet|tab)[;\/]|focus\/\d(?!.+mobile))/i],
      [[TYPE, TABLET]],
    ],
    [
      [/(phone|mobile(?:[;\/]| [ \w\/\.]*safari)|pda(?=.+windows ce))/i],
      [[TYPE, MOBILE]],
    ],
    [
      [/(android[-\w\. ]{0,9});.+buil/i],
      [MODEL, [VENDOR, "Generic"]],
    ],
  ],
  engine: [
    [
      [/windows.+ edge\/([\w\.]+)/i],
      [VERSION, [NAME, `${EDGE}HTML`]],
    ],
    [
      [/webkit\/537\.36.+chrome\/(?!27)([\w\.]+)/i],
      [VERSION, [NAME, "Blink"]],
    ],
    [
      [
        /(presto)\/([\w\.]+)/i, // Presto
        /(webkit|trident|netfront|netsurf|amaya|lynx|w3m|goanna)\/([\w\.]+)/i, // WebKit/Trident/NetFront/NetSurf/Amaya/Lynx/w3m/Goanna
        /ekioh(flow)\/([\w\.]+)/i, // Flow
        /(khtml|tasman|links)[\/ ]\(?([\w\.]+)/i, // KHTML/Tasman/Links
        /(icab)[\/ ]([23]\.[\d\.]+)/i, // iCab
        /\b(libweb)/i,
      ],
      [NAME, VERSION],
    ],
    [
      [/rv\:([\w\.]{1,9})\b.+(gecko)/i],
      [VERSION, NAME],
    ],
  ],
  os: [
    [
      [/microsoft (windows) (vista|xp)/i],
      [NAME, VERSION],
    ],
    [
      [
        /(windows) nt 6\.2; (arm)/i, // Windows RT
        /(windows (?:phone(?: os)?|mobile))[\/ ]?([\d\.\w ]*)/i, // Windows Phone
        /(windows)[\/ ]?([ntce\d\. ]+\w)(?!.+xbox)/i,
      ],
      [NAME, [VERSION, mapWinVer]],
    ],
    [
      [/(win(?=3|9|n)|win 9x )([nt\d\.]+)/i],
      [[NAME, WINDOWS], [VERSION, mapWinVer]],
    ],
    [
      [
        /ip[honead]{2,4}\b(?:.*os ([\w]+) like mac|; opera)/i, // iOS
        /(?:ios;fbsv\/|iphone.+ios[\/ ])([\d\.]+)/i,
        /cfnetwork\/.+darwin/i,
      ],
      [[VERSION, /_/g, "."], [NAME, "iOS"]],
    ],
    [
      [/(mac os x) ?([\w\. ]*)/i, /(macintosh|mac_powerpc\b)(?!.+haiku)/i],
      [[NAME, "macOS"], [VERSION, /_/g, "."]],
    ],
    [
      [/droid ([\w\.]+)\b.+(android[- ]x86|harmonyos)/i],
      [VERSION, NAME],
    ],
    [
      [
        /(android|webos|qnx|bada|rim tablet os|maemo|meego|sailfish)[-\/ ]?([\w\.]*)/i,
        /(blackberry)\w*\/([\w\.]*)/i, // Blackberry
        /(tizen|kaios)[\/ ]([\w\.]+)/i, // Tizen/KaiOS
        /\((series40);/i,
      ],
      [NAME, VERSION],
    ],
    [
      [/\(bb(10);/i],
      [VERSION, [NAME, BLACKBERRY]],
    ],
    [
      [/(?:symbian ?os|symbos|s60(?=;)|series60)[-\/ ]?([\w\.]*)/i],
      [VERSION, [NAME, "Symbian"]],
    ],
    [
      [/mozilla\/[\d\.]+ \((?:mobile|tablet|tv|mobile; [\w ]+); rv:.+ gecko\/([\w\.]+)/i],
      [VERSION, [NAME, `${FIREFOX} OS`]],
    ],
    [
      [
        /web0s;.+rt(tv)/i,
        /\b(?:hp)?wos(?:browser)?\/([\w\.]+)/i,
      ],
      [VERSION, [NAME, "webOS"]],
    ],
    [
      [/watch(?: ?os[,\/]|\d,\d\/)([\d\.]+)/i],
      [VERSION, [NAME, "watchOS"]],
    ],
    [
      [/crkey\/([\d\.]+)/i],
      [VERSION, [NAME, `${CHROME}cast`]],
    ],
    [
      [/(cros) [\w]+(?:\)| ([\w\.]+)\b)/i],
      [[NAME, "Chrome OS"], VERSION],
    ],
    [
      [
        /panasonic;(viera)/i, // Panasonic Viera
        /(netrange)mmh/i, // Netrange
        /(nettv)\/(\d+\.[\w\.]+)/i, // NetTV

        // Console
        /(nintendo|playstation) (\w+)/i, // Nintendo/Playstation
        /(xbox); +xbox ([^\);]+)/i, // Microsoft Xbox (360, One, X, S, Series X, Series S)

        // Other
        /\b(joli|palm)\b ?(?:os)?\/?([\w\.]*)/i, // Joli/Palm
        /(mint)[\/\(\) ]?(\w*)/i, // Mint
        /(mageia|vectorlinux)[; ]/i, // Mageia/VectorLinux
        /([kxln]?ubuntu|debian|suse|opensuse|gentoo|arch(?= linux)|slackware|fedora|mandriva|centos|pclinuxos|red ?hat|zenwalk|linpus|raspbian|plan 9|minix|risc os|contiki|deepin|manjaro|elementary os|sabayon|linspire)(?: gnu\/linux)?(?: enterprise)?(?:[- ]linux)?(?:-gnu)?[-\/ ]?(?!chrom|package)([-\w\.]*)/i,
        // Ubuntu/Debian/SUSE/Gentoo/Arch/Slackware/Fedora/Mandriva/CentOS/PCLinuxOS/RedHat/Zenwalk/Linpus/Raspbian/Plan9/Minix/RISCOS/Contiki/Deepin/Manjaro/elementary/Sabayon/Linspire
        /(hurd|linux) ?([\w\.]*)/i, // Hurd/Linux
        /(gnu) ?([\w\.]*)/i, // GNU
        /\b([-frentopcghs]{0,5}bsd|dragonfly)[\/ ]?(?!amd|[ix346]{1,2}86)([\w\.]*)/i, // FreeBSD/NetBSD/OpenBSD/PC-BSD/GhostBSD/DragonFly
        /(haiku) (\w+)/i,
      ],
      [NAME, VERSION],
    ],
    [
      [/(sunos) ?([\w\.\d]*)/i],
      [[NAME, "Solaris"], VERSION],
    ],
    [
      [
        /((?:open)?solaris)[-\/ ]?([\w\.]*)/i, // Solaris
        /(aix) ((\d)(?=\.|\)| )[\w\.])*/i, // AIX
        /\b(beos|os\/2|amigaos|morphos|openvms|fuchsia|hp-ux|serenityos)/i, // BeOS/OS2/AmigaOS/MorphOS/OpenVMS/Fuchsia/HP-UX/SerenityOS
        /(unix) ?([\w\.]*)/i,
      ],
      [NAME, VERSION],
    ],
  ],
};

export class UserAgent {
  #browser?: Browser;
  #cpu?: Cpu;
  #device?: Device;
  #engine?: Engine;
  #os?: Os;
  #ua: string;

  /** A representation of user agent string, which can be used to determine
   * environmental information represented by the string. All properties are
   * determined lazily.
   *
   * ```ts
   * import { UserAgent } from "https://deno.land/std@$STD_VERSION/http/user_agent.ts";
   *
   * Deno.serve((req) => {
   *   const userAgent = new UserAgent(req.headers.get("user-agent") ?? "");
   *   return new Response(`Hello, ${userAgent.browser.name}
   *     on ${userAgent.os.name} ${userAgent.os.version}!`);
   * });
   * ```
   */
  constructor(ua: string | null) {
    this.#ua = ua ?? "";
  }

  /** The name and version of the browser extracted from the user agent
   * string. */
  get browser(): Browser {
    if (!this.#browser) {
      this.#browser = { name: undefined, version: undefined, major: undefined };
      mapper(this.#browser, this.#ua, matchers.browser);
      // deno-lint-ignore no-explicit-any
      (this.#browser as any).major = majorize(this.#browser.version);
      Object.freeze(this.#browser);
    }
    return this.#browser;
  }

  /** The architecture of the CPU extracted from the user agent string. */
  get cpu(): Cpu {
    if (!this.#cpu) {
      this.#cpu = { architecture: undefined };
      mapper(this.#cpu, this.#ua, matchers.cpu);
      Object.freeze(this.#cpu);
    }
    return this.#cpu;
  }

  /** The model, type, and vendor of a device if present in a user agent
   * string. */
  get device(): Device {
    if (!this.#device) {
      this.#device = { model: undefined, type: undefined, vendor: undefined };
      mapper(this.#device, this.#ua, matchers.device);
      Object.freeze(this.#device);
    }
    return this.#device;
  }

  /** The name and version of the browser engine in a user agent string. */
  get engine(): Engine {
    if (!this.#engine) {
      this.#engine = { name: undefined, version: undefined };
      mapper(this.#engine, this.#ua, matchers.engine);
      Object.freeze(this.#engine);
    }
    return this.#engine;
  }

  /** The name and version of the operating system in a user agent string. */
  get os(): Os {
    if (!this.#os) {
      this.#os = { name: undefined, version: undefined };
      mapper(this.#os, this.#ua, matchers.os);
      Object.freeze(this.#os);
    }
    return this.#os;
  }

  /** A read only version of the user agent string related to the instance. */
  get ua(): string {
    return this.#ua;
  }

  toJSON() {
    const { browser, cpu, device, engine, os, ua } = this;
    return { browser, cpu, device, engine, os, ua };
  }

  toString(): string {
    return this.#ua;
  }

  [Symbol.for("Deno.customInspect")](
    inspect: (value: unknown) => string,
  ): string {
    const { browser, cpu, device, engine, os, ua } = this;
    return `${this.constructor.name} ${
      inspect({ browser, cpu, device, engine, os, ua })
    }`;
  }

  [Symbol.for("nodejs.util.inspect.custom")](
    depth: number,
    // deno-lint-ignore no-explicit-any
    options: any,
    inspect: (value: unknown, options?: unknown) => string,
  ): string {
    if (depth < 0) {
      return options.stylize(`[${this.constructor.name}]`, "special");
    }

    const newOptions = Object.assign({}, options, {
      depth: options.depth === null ? null : options.depth - 1,
    });
    const { browser, cpu, device, engine, os, ua } = this;
    return `${options.stylize(this.constructor.name, "special")} ${
      inspect(
        { browser, cpu, device, engine, os, ua },
        newOptions,
      )
    }`;
  }
}
