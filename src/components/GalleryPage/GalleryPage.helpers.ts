export { getMimeType } from "../../utils/mediaTypes";
import { getFileCategory } from "../../utils/mediaTypes";

export function isImageFile(filename: string): boolean {
  return getFileCategory(filename) === "image";
}
