export const fsaSupported =
  typeof window !== "undefined" && "showOpenFilePicker" in window;

export async function pickFile(): Promise<File | null> {
  if (fsaSupported) {
    const [handle] = await window.showOpenFilePicker();
    return handle.getFile();
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "*/*";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.click();
  });
}
