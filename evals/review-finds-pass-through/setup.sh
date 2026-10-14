#!/bin/bash
# A small repository whose TextDocument only forwards to TextArea.
set -euo pipefail
mkdir -p src
cat > src/textArea.ts <<'TS'
/** Stores the characters of one file and the caret position. */
export class TextArea {
  private text = "";
  private caret = 0;

  /** Inserts `text` before the character at `offset` (0 = start of file). */
  insertString(text: string, offset: number): void {
    this.text = this.text.slice(0, offset) + text + this.text.slice(offset);
    this.caret = offset + text.length;
  }

  /** Index of the character the caret sits before. */
  getCursorOffset(): number {
    return this.caret;
  }
}
TS
cat > src/textDocument.ts <<'TS'
import { TextArea } from "./textArea";

export class TextDocument {
  private area = new TextArea();

  insertString(text: string, offset: number): void { this.area.insertString(text, offset); }
  getCursorOffset(): number { return this.area.getCursorOffset(); }
}
TS
git init -q -b main
git -c user.email=eval@example.com -c user.name=eval add -A
git -c user.email=eval@example.com -c user.name=eval commit -q -m "text document"
