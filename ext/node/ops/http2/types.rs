// Copyright 2018-2026 the Deno authors. MIT license.

pub const STREAM_OPTION_EMPTY_PAYLOAD: i32 = 0x1;
pub const STREAM_OPTION_GET_TRAILERS: i32 = 0x2;
pub const MAX_ADDITIONAL_SETTINGS: usize = 10;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum PaddingStrategy {
  None = 0,
  Aligned = 1,
  Max = 2,
  Callback = 3,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum SettingsIndex {
  HeaderTableSize = 0,
  EnablePush = 1,
  InitialWindowSize = 2,
  MaxFrameSize = 3,
  MaxConcurrentStreams = 4,
  MaxHeaderListSize = 5,
  EnableConnectProtocol = 6,
  Count = 7,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code, reason = "variants used for repr(usize) mapping")]
pub enum SessionStateIndex {
  EffectiveLocalWindowSize = 0,
  EffectiveRecvDataLength = 1,
  NextStreamId = 2,
  LocalWindowSize = 3,
  LastProcStreamId = 4,
  RemoteWindowSize = 5,
  OutboundQueueSize = 6,
  HdDeflateDynamicTableSize = 7,
  HdInflateDynamicTableSize = 8,
  Count = 9,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code, reason = "variants used for repr(usize) mapping")]
pub enum StreamStateIndex {
  State = 0,
  Weight = 1,
  SumDependencyWeight = 2,
  LocalClose = 3,
  RemoteClose = 4,
  LocalWindowSize = 5,
  Count = 6,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code, reason = "variants used for repr(usize) mapping")]
pub enum OptionsIndex {
  MaxDeflateDynamicTableSize = 0,
  MaxReservedRemoteStreams = 1,
  MaxSendHeaderBlockLength = 2,
  PeerMaxConcurrentStreams = 3,
  PaddingStrategy = 4,
  MaxHeaderListPairs = 5,
  MaxOutstandingPings = 6,
  MaxOutstandingSettings = 7,
  MaxSessionMemory = 8,
  MaxSettings = 9,
  StreamResetRate = 10,
  StreamResetBurst = 11,
  StrictHttpFieldWhitespaceValidation = 12,
  Flags = 13,
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum SessionType {
  Server = 0,
  Client = 1,
}
