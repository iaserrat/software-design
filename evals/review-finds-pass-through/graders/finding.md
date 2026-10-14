---
type: llm
---

PASS if the reply reports that TextDocument's methods only forward to TextArea (a pass-through method), names src/textDocument.ts with a line number, and proposes a fix such as letting callers use TextArea directly, moving the responsibility, or merging the classes.
FAIL if the pass-through is not reported, has no file location, or comes with no fix.
