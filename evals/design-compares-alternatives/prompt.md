---
description: A design request produces several different interfaces, compares them on caller code, and recommends one.
tags: [design]
max_turns: 40
timeout_seconds: 900
allowed_tools: [Read, Glob, Grep, Skill, Agent, AskUserQuestion, mcp__plugin_software-design_software-design__check, mcp__plugin_software-design_software-design__interface]
---

I need a class that holds the text of a file for a GUI text editor. The editor inserts typed characters at the caret, deletes the character before the caret on backspace, deletes the character after it on delete, and deletes the selected range. Design the class interface in TypeScript. Don't write the implementation yet.
