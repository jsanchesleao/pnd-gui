import { parse } from "marked";
import classes from "./MarkdownView.module.css";

interface Props {
  text: string;
  /** Override max-height. Defaults to "70vh". */
  maxHeight?: string;
}

export const MarkdownView: React.FC<Props> = ({ text, maxHeight }) => {
  const html = parse(text) as string;
  return (
    <div
      className={classes.body}
      style={maxHeight !== undefined ? { maxHeight } : undefined}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
};
