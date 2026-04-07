/**
 * Renders the first few lines of a text string onto a canvas and returns the
 * result as a WebP Uint8Array. Used to generate thumbnails for .txt and .md
 * vault entries. Returns null if the canvas API is unavailable.
 */
export async function generateTextThumbnail(
  text: string,
  width = 200,
  height = 150,
  quality = 0.85,
): Promise<Uint8Array | null> {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const fontSize = 9;
  const lineHeight = fontSize + 3;
  const padding = 6;
  const maxLines = Math.floor((height - padding * 2) / lineHeight);
  const charsPerLine = 38; // approximate for 200px canvas at 9px monospace

  ctx.fillStyle = "#1e1e2e";
  ctx.fillRect(0, 0, width, height);

  ctx.fillStyle = "#cdd6f4";
  ctx.font = `${fontSize}px monospace`;

  const rawLines = text.split("\n");
  const lines: string[] = [];
  for (const rawLine of rawLines) {
    if (lines.length >= maxLines) break;
    lines.push(rawLine.slice(0, charsPerLine));
  }

  lines.forEach((line, i) => {
    ctx.fillText(line, padding, padding + fontSize + i * lineHeight);
  });

  return new Promise<Uint8Array | null>((resolve) => {
    canvas.toBlob(
      (blob) => {
        if (!blob) { resolve(null); return; }
        blob.arrayBuffer().then((ab) => resolve(new Uint8Array(ab)));
      },
      "image/webp",
      quality,
    );
  });
}
