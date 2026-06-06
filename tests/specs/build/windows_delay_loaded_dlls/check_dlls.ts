// reads the deno binary's PE headers and outputs all DLLs
// along with whether they are eagerly or delay-loaded.

function readU16(buf: Uint8Array, offset: number): number {
  return buf[offset] | (buf[offset + 1] << 8);
}

function readU32(buf: Uint8Array, offset: number): number {
  return (
    (buf[offset] |
      (buf[offset + 1] << 8) |
      (buf[offset + 2] << 16) |
      ((buf[offset + 3] << 24) >>> 0)) >>>
    0
  );
}

function readCString(buf: Uint8Array, offset: number): string {
  let end = offset;
  while (end < buf.length && buf[end] !== 0) end++;
  return new TextDecoder().decode(buf.subarray(offset, end));
}

interface Section {
  name: string;
  virtualAddress: number;
  virtualSize: number;
  rawOffset: number;
  rawSize: number;
}

function rvaToOffset(rva: number, sections: Section[]): number | null {
  for (const s of sections) {
    if (rva >= s.virtualAddress && rva < s.virtualAddress + s.rawSize) {
      return s.rawOffset + (rva - s.virtualAddress);
    }
  }
  return null;
}

function parseDllNames(
  buf: Uint8Array,
  tableRva: number,
  sections: Section[],
  entrySize: number,
  nameRvaOffset: number,
): string[] {
  const names: string[] = [];
  const tableOffset = rvaToOffset(tableRva, sections);
  if (tableOffset === null) return names;

  let idx = 0;
  while (true) {
    const entryOff = tableOffset + idx * entrySize;
    const nameRva = readU32(buf, entryOff + nameRvaOffset);
    if (nameRva === 0) break;
    const nameOff = rvaToOffset(nameRva, sections);
    if (nameOff !== null) {
      names.push(readCString(buf, nameOff).toLowerCase());
    }
    idx++;
  }
  return names;
}

const exePath = Deno.execPath();
const buf = Deno.readFileSync(exePath);

// parse PE
const peOffset = readU32(buf, 0x3c);
const numSections = readU16(buf, peOffset + 6);
const sizeOptHeader = readU16(buf, peOffset + 24 - 4);
const optStart = peOffset + 24;
const magic = readU16(buf, optStart);

if (magic !== 0x20b) {
  console.error("not a PE32+ binary");
  Deno.exit(1);
}

// data directories start at optStart + 112, preceded by count at +108
const ddStart = optStart + 112;

// read sections
const secStart = optStart + sizeOptHeader;
const sections: Section[] = [];
for (let i = 0; i < numSections; i++) {
  const off = secStart + i * 40;
  let nameEnd = 8;
  while (nameEnd > 0 && buf[off + nameEnd - 1] === 0) nameEnd--;
  const name = new TextDecoder().decode(buf.subarray(off, off + nameEnd));
  sections.push({
    name,
    virtualSize: readU32(buf, off + 8),
    virtualAddress: readU32(buf, off + 12),
    rawSize: readU32(buf, off + 16),
    rawOffset: readU32(buf, off + 20),
  });
}

// import directory (index 1) - entries are 20 bytes, name RVA at offset 12
const importRva = readU32(buf, ddStart + 1 * 8);
const eagerDlls = parseDllNames(buf, importRva, sections, 20, 12);

// delay import directory (index 13) - entries are 32 bytes, name RVA at offset 4
const delayRva = readU32(buf, ddStart + 13 * 8);
const delayDlls = parseDllNames(buf, delayRva, sections, 32, 4);

// deduplicate and collect all dlls with their load type
const allDlls = new Map<string, string>();
for (const dll of eagerDlls) {
  allDlls.set(dll, "eager");
}
for (const dll of delayDlls) {
  allDlls.set(dll, "delay");
}

const sorted = [...allDlls.entries()].sort((a, b) => a[0].localeCompare(b[0]));
for (const [dll, type] of sorted) {
  console.log(`${dll}: ${type}`);
}
