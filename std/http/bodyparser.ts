// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { BufReader, ReadLineResult } from "./../io/bufio.ts";

const CRLF_LEN = 2;
const decoder = new TextDecoder();
const textFieldReg = /^Content-Disposition\:\sform\-data\;\sname\="([^\"]+)?"$/i;
const fileFieldReg = /^Content-Disposition\:\sform\-data\;\sname\="([^\"]+)?";\sfilename="([^\"]+)?"$/i;
const fileTypeReg = /^Content-Type\:\s([^\;]+)?$/i;

export enum FormFieldType {
  text = "text",
  file = "file"
}


export enum FormEnctype {
  urlencoded = "application/x-www-form-urlencoded",
  multipart = "multipart/form-data",
  unknown = "unknown",

  // TODO
  // html = "text/xml",

  // TODO
  // json = "application/json",
}


/** FormFieldData object */
export interface FormFieldData {
  /** input-type */
  type: FormFieldType; // text, file
  /** input-name */
  name: string;
  /** input-value, the value of input, the value is Uint8Array if input-type="file" */
  value: Uint8Array|string;

  /** input-filename, input-filetype, if there is input-file in the form  */
  filename?: string;
  filetype?: string;
}

export async function parseFormUrlencoded(body: Uint8Array): Promise<FormFieldData[]> {
  const decoder = new TextDecoder();
  // const encoder = new TextEncoder();
  // const bodyStr: string = decoder.decode(body);

  const buf = new Deno.Buffer(body);
  const reader = new BufReader(buf);
  const dataList: FormFieldData[] = [];
  const line = await reader.peek(buf.length);
  if (line instanceof Uint8Array) {
    const bodyStr: string = decoder.decode(line);
    const params: URLSearchParams = new URLSearchParams(bodyStr);
    for (const [name, value] of params.entries()) {
      const data: FormFieldData = {
        name,
        value,
        type: FormFieldType.text,
      }
      dataList.push(data);
    }
  }

  return dataList;
}


function parseEnctype(contentType: string): FormEnctype {
  let enctype: FormEnctype = FormEnctype.unknown;
  const urlencoded = "application/x-www-form-urlencoded";
  const multipart = "multipart/form-data;";

  if (contentType.startsWith(urlencoded)) {
    enctype = FormEnctype.urlencoded;
  } else if (contentType.startsWith(multipart)) {
    enctype = FormEnctype.multipart;
  }
  // TODO
  // else if (contentType.startsWith(json)) {
  //   enctype = FormEnctype.json;
  // }
  return enctype;
}


export class BodyParser {

  private _contentType: string;
  private _enctype: FormEnctype;
  private _body: Uint8Array;

  constructor(contentType: string, body: Uint8Array) {
    this._contentType = contentType;
    this._body = body;
    this._enctype = parseEnctype(this._contentType);
  }

  public async getFormData(): Promise<FormFieldData[]> {
    let formData: FormFieldData[] = [];
    if (this._enctype === FormEnctype.urlencoded) {
      formData = await parseFormUrlencoded(this._body);
    } else if (this._enctype === FormEnctype.multipart) {
      formData = await parseMultipartForm(this._contentType, this._body);
    }
    return formData;
  }
}




/**
 * parse data from multipart/form-data
 * @param {string} contentType
 * @param {Uint8Array} body
 * @return {FormFieldData[]}
 *  example [{ name: "myName", value: "helloworld" }, { name: "myFile", value: [0,1,...], type: "file", filetype: "image/jpeg", filename: "xxx.jpg" }]
 */
export async function parseMultipartForm(contentType: string, body: Uint8Array): Promise<FormFieldData[]> {

  const typeData = parseMultipartContentType(contentType);
  const boundary: string = typeData.boundary;
  const fields = await parseMultipartStreamToFields(boundary, body);
  const dataList: FormFieldData[] = [];
  for await (const data of parseMultipartFormField(fields)) {
    dataList.push(data);
  }
  return dataList;
}


/**
 * example: "multipart/form-data; boundary=----WebKitFormBoundaryk7fXm5rwGcU1OJIq"
 * return { enctype: "multipart/form-data", boundary: "----WebKitFormBoundaryk7fXm5rwGcU1OJIq" }
 *
 * @param {string} contentType
 * @return {[key: string]: string}
 */
function parseMultipartContentType(contentType: string): {[key: string]: string} {
  const dataList: string[] = contentType.split("; ");
  const enctype = dataList[0];
  let boundary: string = '';
  if (typeof dataList[1] === "string") {
    const strList = dataList[1].split("=");
    if (strList[0] === "boundary") {
      boundary = strList[1];
    }
  }
  return {
    enctype,
    boundary,
  }
}



/**
 * parse multipart/form-data single data of form
 *
 * @param {string} boundary
 * @param {Uint8Array} stream
 * @return {FormFieldData}
 *  example input: { name: "myName", value: "helloworld" }
 *  example output: { name: "myFile", value: [0,1,...], type: "file", filetype: "image/jpeg", filename: "xxx.jpg" }
 */
async function* parseMultipartFormField(fields: Uint8Array[]): AsyncGenerator<FormFieldData> {
  for (let i = 0; i < fields.length; i++) {
    const field = fields[i];
    const reader = new BufReader(new Deno.Buffer(field));
    const contentDescLine: Deno.EOF | ReadLineResult = await reader.readLine();
    if (contentDescLine === Deno.EOF) {
      break;
    }
    if (contentDescLine && contentDescLine.line instanceof Uint8Array) {
      const contentDescChunk = contentDescLine.line;
      const contentDesc = decoder.decode(contentDescChunk);
      if (textFieldReg.test(contentDesc)) {
        const execRs = textFieldReg.exec(contentDesc);
        const nullLine: Deno.EOF | ReadLineResult = await reader.readLine();
        const value: Deno.EOF | ReadLineResult = await reader.readLine();
        if (nullLine === Deno.EOF || value === Deno.EOF) {
          break;
        }
        if (nullLine && nullLine.line instanceof Uint8Array && value && value.line instanceof Uint8Array) {
          const nullLineStr = decoder.decode(nullLine.line);
          const valueStr =  decoder.decode(value.line);
          if (nullLineStr === '') {
            const fieldData: FormFieldData = {
              name: execRs[1],
              value: valueStr,
              type: FormFieldType.text
            }
            yield fieldData
          }
        }
      } else if (fileFieldReg.test(contentDesc)) {
        const execRs = fileFieldReg.exec(contentDesc);

        const contentTypeLine: Deno.EOF | ReadLineResult = await reader.readLine();
        const nullLine: Deno.EOF | ReadLineResult = await reader.readLine();
        if (contentTypeLine === Deno.EOF || nullLine === Deno.EOF) {
          break;
        }
        if (contentTypeLine && contentTypeLine.line instanceof Uint8Array && nullLine && nullLine.line instanceof Uint8Array) {
          const contentTypeChunk = contentTypeLine.line;
          const contentType = decoder.decode(contentTypeChunk);
          const typeRs = fileTypeReg.exec(contentType);

          const nullLineStr = decoder.decode(nullLine.line);
          if (nullLineStr === '') {
            const valueStart = (contentDescChunk.length + CRLF_LEN) + (contentTypeChunk.length + CRLF_LEN) + CRLF_LEN;
            const valueEnd = field.length - CRLF_LEN;
            const fieldData = {
              name: execRs[1],
              type: FormFieldType.file,
              filetype: typeRs[1],
              filename: execRs[2],
              value: field.subarray(valueStart, valueEnd),
            }
            yield fieldData;
          }
        }

      }
    }
  }

}

// Form binary data stream single field-data offset
interface FieldChunkOffset {
  start?: number;
  end?: number;
}


/**
 *  Cut multipart/form-data by field
 *
 * @param {string} boundary
 * @param {Uint8Array} stream
 * @return {Uint8Array[]}
 */
async function parseMultipartStreamToFields(boundary: string, stream: Uint8Array): Promise<Uint8Array[]> {
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();

  const newField = `--${boundary}`;
  const end = `--${boundary}--`;

  const newFieldChunk = encoder.encode(newField);
  const endChunk = encoder.encode(end);

  const bodyBuf = new Deno.Buffer(stream);
  const bufReader = new BufReader(bodyBuf);
  let isFinish: boolean = false;

  const fieldChunkList: Uint8Array[] = [];
  const fieldOffsetList: FieldChunkOffset[] = [];
  let index: number = 0;

  while(!isFinish) {
    const lineResult: Deno.EOF|ReadLineResult = await bufReader.readLine();
    if (lineResult === Deno.EOF) {
      isFinish = true;
      break;
    }

    const lineChunk = lineResult.line;
    const lineChunkLen = lineChunk.length + CRLF_LEN;
    const startIndex = index;
    const endIndex = index + lineChunkLen;

    if (lineChunk.length === endChunk.length) {
      const line: string = decoder.decode(lineChunk);
      if (line === end) {
        isFinish = true;
        if (fieldOffsetList[fieldOffsetList.length - 1]) {
          fieldOffsetList[fieldOffsetList.length - 1].end = endIndex - lineChunkLen;
        }
        break;
      }
    }

    if (lineChunk.length === newFieldChunk.length) {
      const line: string = decoder.decode(lineChunk);
      if (line === newField) {
        if (fieldOffsetList[fieldOffsetList.length - 1]) {
          fieldOffsetList[fieldOffsetList.length - 1].end = startIndex;
        }
        fieldOffsetList.push({
          start: startIndex + lineChunkLen,
        });
      }
    }
    index = endIndex;
  }
  fieldOffsetList.forEach((offset: FieldChunkOffset) => {
    if(offset && offset.start >= 0 && offset.end >= 0) {
      const fieldChunk: Uint8Array = stream.subarray(offset.start, offset.end);
      fieldChunkList.push(fieldChunk);
    }
  });

  return fieldChunkList;
}
