// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { encode, decode } from "./base91.ts";

const encoder = new TextEncoder();
const testCases = [
  ["test", "fPNKd"],
  // source: https://en.everybodywiki.com/BasE91#Example_for_basE91
  [
    "Man is distinguished, not only by his reason, but by this singular passion from other animals, which is a lust of the mind, that by a perseverance of delight in the continued and indefatigable generation of knowledge, exceeds the short vehemence of any carnal pleasure.",
    '8D$J`/wC4!c.hQ;mT8,<p/&Y/H@$]xlL3oDg<W.0$FW6GFMo_D8=8=}AMf][|LfVd/<P1o/1Z2(.I+LR6tQQ0o1a/2/WtN3$3t[x&k)zgZ5=p;LRe.{B[pqa(I.WRT%yxtB92oZB,2,Wzv;Rr#N.cju"JFXiZBMf<WMC&$@+e95p)z01_*UCxT0t88Km=UQJ;WH[#F]4pE>i3o(g7=$e7R2u>xjLxoefB.6Yy#~uex8jEU_1e,MIr%!&=EHnLBn2h>M+;Rl3qxcL5)Wfc,HT$F]4pEsofrFK;W&eh#=#},|iKB,2,W]@fVlx,a<m;i=CY<=Hb%}+},F',
  ],
  // source: https://github.com/mtraver/base91/blob/master/base91_test.go
  [
    "May your trails be crooked, winding, lonesome, dangerous, leading to the most amazing view. May your mountains rise into and above the clouds.",
    '8D9KR`0eLUd/ZQFl62>vb,1RL%%&~8bju"sQ;mmaU=UfU)1T70<^rm?i;Ct)/p;R(&^m5PKimf2+H[QSd/[E<oTPgZh>DZ%y;#,aIl]U>vP:3pPIqSwPmLwre3:W.{6U)/wP;mYBxgP[UCsS)/[EOiqMgZR*Sk<Rd/=8jL=ibg7+b[C',
  ],
  [
    "\x35\x5e\x56\xe0\xc6\x29\x38\xf4\x81\x00\xab\x81\x7e\xd7\x08\x95\x62\x20\xa7\xda\x64\xa2\xce\xb3\xc5",
    "~_J;DXFmbHwEpe5t5FK8VB8T!a>CIdjL;VT!)+vPki2D8pDv",
  ],
  // RFC 4648 examples (adapted from base64 to base91)
  ["", ""],
  ["f", "LB"],
  ["fo", "drD"],
  ["foo", "dr.J"],
  ["foob", "dr/2Y"],
  ["fooba", "dr/2s)A"],
  ["foobar", "dr/2s)uC"],
  // some non-ascii strings from http://kermitproject.org/utf8.html
  [
    "ᚠᛇᚻ᛫ᛒᛦᚦ᛫ᚠᚱᚩᚠᚢᚱ᛫ᚠᛁᚱᚪ᛫ᚷᛖᚻᚹᛦᛚᚳᚢᛗ",
    "4;4k3T3IW.ov3og?f0f|NbU9b:Tn.Sq20ELk$)D29n>tmgQ54;Km3TkJVbbvJSb?vx=$|d=&Q;/ldT:2qEM[&)D2#_b29rh24;am]SSJm.;",
  ],
  [
    "Τη γλώσσα μου έδωσαν ελληνική",
    'vN!`AXlVg[LWE`fu%K8o"WE.g[NW@Fs.apl^[GoVd[8UfIs.ipv9Fq_rg[&U!;!%7pF',
  ],
  [
    "ვეპხის ტყაოსანი შოთა რუსთაველი",
    "^K7j&C[IRXavc/g?|)f|zXfGRXcvz_f?puf|)U<Vl/pF3RiuwE2L_W;V6:pFQSiurE2L]W;VV/pFoSiu1EK?5)[C9_Xe9rU0^KIk&CgD",
  ],
  [
    "ಬಾ ಇಲ್ಲಿ ಸಂಭವಿಸು ಇಂದೆನ್ನ ಹೃದಯದಲಿ",
    'J1QP,jREln;Kig<!^3pM,j_Dq<Fugg{$J1sL,j:Dq<Au5{C?r*Y|RB%AW^@(QPrnn25rhvJ1;OskKDpf`tggb%^30L,j8Dq<}ti;">20*$.sB',
  ],
];

Deno.test({
  name: "[encoding/base91] encode",
  fn(): void {
    for (const [bin, b91] of testCases) {
      assertEquals(encode(encoder.encode(bin)), b91);
    }
  },
});

Deno.test({
  name: "[encoding/base91] decode",
  fn(): void {
    for (const [bin, b91] of testCases) {
      assertEquals(decode(b91), encoder.encode(bin));
    }
  },
});
