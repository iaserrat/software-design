// Blank space left on both sides of each line of text, in pixels.
const textHorizontalPadding = 4;
// TODO: read this from the theme
const lineHeight = 16;
/** Copies at most `count` bytes; returns how many were copied, 0 past the end. */
export function copyBytes(offset: number, count: number): number { return copy(offset, count); }
