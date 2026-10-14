export function cleanup(name: string): void {
  try { unsetVariable(name); } catch (e) {}
}
