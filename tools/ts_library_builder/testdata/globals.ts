import * as moduleC from "./moduleC";
import * as moduleD from "./moduleD";
import * as moduleE from "./moduleE";

// tslint:disable-next-line:no-any
const foobarbaz: any = {};
foobarbaz.bar = new moduleC.Bar();
foobarbaz.qat = moduleC.qat;
foobarbaz.process = moduleE.process;
foobarbaz.reprocess = moduleD.reprocess;
foobarbaz.Bar = moduleC.Bar;
export type Bar = moduleC.Bar;
