export class TextDocument {
  private area: TextArea;
  insertString(text: string, offset: number): void { this.area.insertString(text, offset); }
  getCursorOffset(): number { return this.area.getCursorOffset(); }
}
