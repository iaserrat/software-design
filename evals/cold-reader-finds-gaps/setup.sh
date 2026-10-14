#!/bin/bash
# An API whose signature leaves out what callers need: range rules and the result.
set -euo pipefail
mkdir -p src
cat > src/buffer.ts <<'TS'
export class Buffer {
  private chunks: Uint8Array[] = [];

  /** Copy bytes. */
  copy(offset: number, length: number, dest: Uint8Array): number {
    let copied = 0;
    for (const chunk of this.chunks) {
      if (offset >= chunk.length) { offset -= chunk.length; continue; }
      const n = Math.min(chunk.length - offset, length - copied);
      dest.set(chunk.subarray(offset, offset + n), copied);
      copied += n; offset = 0;
      if (copied === length) break;
    }
    return copied;
  }
}
TS
git init -q -b main
git -c user.email=eval@example.com -c user.name=eval add -A
git -c user.email=eval@example.com -c user.name=eval commit -q -m buffer
