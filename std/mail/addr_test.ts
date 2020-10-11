import { parseAddr } from "./addr.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test("[mail/addr] singleMailAddress", function (): void {
  assertEquals(
    parseAddr("foo bar <foo@bar.com>"),
    [
      {
        addr: "foo@bar.com",
        addrs: null,
        displayName: "foo bar",
        groupName: null,
      },
    ],
  );
});

Deno.test("[mail/addr] multiMailAddress", function (): void {
  assertEquals(
    parseAddr("foo <ba@r>, jo@e, baz <qu@ux>"),
    [
      { displayName: "foo", addr: "ba@r", groupName: null, addrs: null },
      { displayName: null, addr: "jo@e", groupName: null, addrs: null },
      { displayName: "baz", addr: "qu@ux", groupName: null, addrs: null },
    ],
  );
});

Deno.test("[mail/addr] emptyMailGroup", function (): void {
  assertEquals(
    parseAddr("empty-group:;"),
    [
      { displayName: null, addr: null, groupName: "empty-group", addrs: [] },
    ],
  );
});
Deno.test("[mail/addr] parseBasicGroup", function (): void {
  assertEquals(
    parseAddr("bar-group: foo <foo@bar.com>;"),
    [
      {
        displayName: null,
        addr: null,
        groupName: "bar-group",
        addrs: [
          {
            addr: "foo@bar.com",
            addrs: null,
            displayName: "foo",
            groupName: null,
          },
        ],
      },
    ],
  );
});

Deno.test("[mail/addr] parseMixedAddress", function (): void {
  assertEquals(
    parseAddr("joe@bloe.com, bar-group: foo <foo@bar.com>;"),
    [
      {
        addr: "joe@bloe.com",
        addrs: null,
        displayName: null,
        groupName: null,
      },
      {
        displayName: null,
        addr: null,
        groupName: "bar-group",
        addrs: [
          {
            addr: "foo@bar.com",
            addrs: null,
            displayName: "foo",
            groupName: null,
          },
        ],
      },
    ],
  );
});
