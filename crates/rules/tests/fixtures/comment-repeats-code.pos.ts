function build(panel: Panel): void {
  // Add a horizontal scroll bar
  const hScrollBar = new ScrollBar(ScrollBar.HORIZONTAL);
  panel.add(hScrollBar);
}
/** Downcast the parameter to the type. */
export function downCastParameter(parameter: string, type: string): unknown { return convert(parameter, type); }
