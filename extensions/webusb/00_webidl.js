// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;

  webidl.converters["USBRequestType"] = webidl.createEnumConverter(
    "USBRequestType",
    [
      "standard",
      "class",
      "vendor",
    ],
  );

  webidl.converters["USBDirection"] = webidl.createEnumConverter(
    "USBDirection",
    [
      "in",
      "out",
    ],
  );

  webidl.converters["USBRecipient"] = webidl.createEnumConverter(
    "USBRecipient",
    [
      "device",
      "interface",
      "endpoint",
      "other",
    ],
  );

  const USBControlTransferParameters = [
    {
      key: "requestType",
      converter: webidl.converters["USBRequestType"],
      required: true,
    },
    {
      key: "recipient",
      converter: webidl.converters["USBRecipient"],
      required: true,
    },
    {
      key: "request",
      converter: webidl.converters["octet"],
      required: true,
    },
    {
      key: "value",
      converter: webidl.converters["unsigned short"],
      required: true,
    },
    {
      key: "index",
      converter: webidl.converters["unsigned short"],
      required: true,
    },
  ];

  webidl.converters["USBControlTransferParameters"] = webidl
    .createDictionaryConverter(
      "USBControlTransferParameters",
      USBControlTransferParameters,
    );

  const USBDeviceFilter = [
    {
      key: "vendorId",
      converters: webidl.converters["unsigned short"],
    },
    {
      key: "productId",
      converters: webidl.converters["unsigned short"],
    },
    {
      key: "classCode",
      converters: webidl.converters["octet"],
    },
    {
      key: "subclassCode",
      converters: webidl.converters["octet"],
    },
    {
      key: "protocolCode",
      converters: webidl.converters["octet"],
    },
    {
      key: "serialNumber",
      converters: webidl.converters["DOMString"],
    },
  ];

  webidl.converters["USBDeviceFilter"] = webidl.createDictionaryConverter(
    "USBDeviceFilter",
    USBDeviceFilter,
  );

  webidl.converters["sequence<USBDeviceFilter>"] = webidl
    .createSequenceConverter(webidl.converters["USBDeviceFilter"]);

  const USBDeviceRequestOptions = [
    {
      key: "filters",
      converter: webidl.converters["sequence<USBDeviceFilter>"],
      required: true,
    },
  ];

  webidl.converters["USBDeviceRequestOptions"] = webidl
    .createDictionaryConverter(
      "USBDeviceRequestOptions",
      USBDeviceRequestOptions,
    );
})(this);
