import { inflate } from "fflate";

export interface ZipEntry {
  name: string;
  localHeaderOffset: number;
  compressedSize: number;
  compressionMethod: number; // 0 = stored, 8 = deflated
}

function readUint16LE(buf: Uint8Array, offset: number): number {
  return buf[offset] | (buf[offset + 1] << 8);
}

function readUint32LE(buf: Uint8Array, offset: number): number {
  return (
    (buf[offset] |
      (buf[offset + 1] << 8) |
      (buf[offset + 2] << 16) |
      (buf[offset + 3] << 24)) >>>
    0
  );
}

async function readSlice(file: File, start: number, length: number): Promise<Uint8Array> {
  const slice = file.slice(start, start + length);
  return new Uint8Array(await slice.arrayBuffer());
}

// Locate the End of Central Directory (EOCD) record.
// The EOCD signature is 0x06054b50. It is at least 22 bytes and appears
// at most 65558 bytes from the end of the file (22 + max 65535-byte comment).
async function findEOCD(file: File): Promise<{ cdOffset: number; cdSize: number }> {
  const searchSize = Math.min(file.size, 65558);
  const tail = await readSlice(file, file.size - searchSize, searchSize);

  for (let i = tail.length - 22; i >= 0; i--) {
    if (
      tail[i] === 0x50 &&
      tail[i + 1] === 0x4b &&
      tail[i + 2] === 0x05 &&
      tail[i + 3] === 0x06
    ) {
      const cdSize = readUint32LE(tail, i + 12);
      const cdOffset = readUint32LE(tail, i + 16);
      return { cdOffset, cdSize };
    }
  }
  throw new Error("Not a valid ZIP file: EOCD not found");
}

// Parse the ZIP central directory and return a map of filename → ZipEntry.
export async function parseZipDirectory(file: File): Promise<Map<string, ZipEntry>> {
  const { cdOffset, cdSize } = await findEOCD(file);
  const cd = await readSlice(file, cdOffset, cdSize);
  const entries = new Map<string, ZipEntry>();

  let pos = 0;
  while (pos + 46 <= cd.length) {
    // Central directory file header signature: 0x02014b50
    if (
      cd[pos] !== 0x50 ||
      cd[pos + 1] !== 0x4b ||
      cd[pos + 2] !== 0x01 ||
      cd[pos + 3] !== 0x02
    ) {
      break;
    }

    const compressionMethod = readUint16LE(cd, pos + 10);
    const compressedSize = readUint32LE(cd, pos + 20);
    const localHeaderOffset = readUint32LE(cd, pos + 42);
    const fileNameLength = readUint16LE(cd, pos + 28);
    const extraFieldLength = readUint16LE(cd, pos + 30);
    const fileCommentLength = readUint16LE(cd, pos + 32);

    const name = new TextDecoder().decode(cd.slice(pos + 46, pos + 46 + fileNameLength));

    entries.set(name, {
      name,
      localHeaderOffset,
      compressedSize,
      compressionMethod,
    });

    pos += 46 + fileNameLength + extraFieldLength + fileCommentLength;
  }

  return entries;
}

// Read and extract the data for a single ZIP entry.
// For compression method 0 (stored): returns the raw bytes.
// For compression method 8 (deflated): decompresses with fflate inflate.
export async function readZipEntry(file: File, entry: ZipEntry): Promise<Uint8Array> {
  // Read local file header (30 bytes fixed + variable filename/extra)
  const localHeader = await readSlice(file, entry.localHeaderOffset, 30);
  const fileNameLength = readUint16LE(localHeader, 26);
  const extraFieldLength = readUint16LE(localHeader, 28);
  const dataOffset = entry.localHeaderOffset + 30 + fileNameLength + extraFieldLength;

  const compressedData = await readSlice(file, dataOffset, entry.compressedSize);

  if (entry.compressionMethod === 0) {
    return compressedData;
  }

  if (entry.compressionMethod === 8) {
    return new Promise((resolve, reject) => {
      inflate(compressedData, (err, data) => {
        if (err) reject(err);
        else resolve(data);
      });
    });
  }

  throw new Error(`Unsupported ZIP compression method: ${entry.compressionMethod}`);
}
