/**
 * Collects a ReadableStream into a Blob, then triggers a browser download.
 * Respects an AbortSignal — throws DOMException("Aborted", "AbortError") on cancel.
 */
export async function collectAndDownload(
  stream: ReadableStream<Uint8Array>,
  filename: string,
  signal: AbortSignal,
): Promise<void> {
  const chunks: Uint8Array[] = [];
  const reader = stream.getReader();
  try {
    while (true) {
      if (signal.aborted) throw new DOMException("Aborted", "AbortError");
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const blob = new Blob(chunks);
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
