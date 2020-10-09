// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type Message = string;

export interface HmacHasher {
  update(data: Message): this;
  digest(): ArrayBuffer;
}
