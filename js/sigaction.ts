// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert, notImplemented } from "./util";
import { build } from "./build";

function reqListen(
  signo: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.SignalListen.createSignalListen(builder, signo);
  return [builder, msg.Any.SignalListen, inner];
}

function resListen(baseRes: null | msg.Base): number {
  assert(baseRes !== null);
  assert(msg.Any.SignalListenRes === baseRes!.innerType());
  const res = new msg.SignalListenRes();
  assert(baseRes!.inner(res) !== null);
  const rid = res.rid();
  assert(rid !== null);
  return rid!;
}

function reqPoll(
  rid: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.SignalPoll.createSignalPoll(builder, rid);
  return [builder, msg.Any.SignalPoll, inner];
}

function resPoll(baseRes: null | msg.Base): number {
  assert(baseRes !== null);
  assert(msg.Any.SignalPollRes === baseRes!.innerType());
  const res = new msg.SignalPollRes();
  assert(baseRes!.inner(res) !== null);
  const signo = res.signo();
  assert(signo !== null);
  return signo!;
}

type SignalHandler = () => void;

interface SignalInfo {
  rid: number;
  handlers: SignalHandler[];
}

const handlerMap = new Map<number, SignalInfo>();

async function startSignalLoop(rid: number): Promise<void> {
  while (true) {
    const signo = resPoll(await dispatch.sendAsync(...reqPoll(rid)));
    const signalInfo = handlerMap.get(signo);
    assert(!!signalInfo);
    signalInfo!.handlers.forEach((h: SignalHandler): void => h());
  }
}

/** Register a handler for a given signal.
 * Could be called multiple times to register extra handlers for the
 * same signal.
 */
export function sigaction(signo: number, handler: SignalHandler): void {
  if (build.os === "win") {
    notImplemented();
  }

  if (!handlerMap.has(signo)) {
    // register handler;
    const rid = resListen(dispatch.sendSync(...reqListen(signo)));
    handlerMap.set(signo, { rid, handlers: [] });
    startSignalLoop(rid);
  }
  handlerMap.get(signo)!.handlers.push(handler);
}
