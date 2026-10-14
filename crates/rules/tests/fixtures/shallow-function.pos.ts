function addNullValueForAttribute(attrs: Map<string, null>, name: string): void { attrs.set(name, null); }
export function reset(attrs: Map<string, null>): void { addNullValueForAttribute(attrs, "id"); }
