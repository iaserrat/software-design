// The pi adapter: pi has no hooks file or plugin MCP config, so this extension
// registers the MCP server and turns pi events into the binary's hook protocol.
import { spawn } from "node:child_process";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const launcher = join(dirname(fileURLToPath(import.meta.url)), "..", "scripts", "software-design");
// The binary keeps per-session state under CLAUDE_PLUGIN_DATA, whatever the host.
const data = join(process.env.SOFTWARE_DESIGN_HOME ?? join(homedir(), ".software-design"), "pi");

/** Runs the launcher with `input` on stdin; resolves to its stdout, "" on any failure. */
function launch(args: string[], input: string, cwd: string): Promise<string> {
  return new Promise((resolve) => {
    const child = spawn("sh", [launcher, ...args], {
      cwd,
      env: { ...process.env, CLAUDE_PLUGIN_DATA: data },
      stdio: ["pipe", "pipe", "ignore"],
    });
    let out = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.on("error", () => resolve(""));
    child.on("close", () => resolve(out));
    child.stdin.end(input);
  });
}

/** One hook event, as Claude Code and Codex send it; the parsed reply or undefined. */
async function hook(event: string, input: Record<string, unknown>, cwd: string) {
  const out = (await launch(["hook", event], JSON.stringify({ cwd, ...input }), cwd)).trim();
  try {
    return out ? JSON.parse(out) : undefined;
  } catch {
    return undefined;
  }
}

export default function softwareDesign(pi: ExtensionAPI) {
  // Relative `cwd` is the session directory, so the tools analyse the project.
  pi.registerMcpServer("software-design", {
    command: "sh",
    args: [launcher, "mcp"],
    cwd: ".",
    exposure: "direct",
    description: "Design red flags (`check`) and public interfaces without bodies (`interface`).",
  });

  // Like Claude Code's `stop_hook_active`: one forced continuation per prompt.
  let continued = false;

  pi.on("session_start", async (_event, ctx) => {
    const reply = await hook("session-start", { session_id: ctx.sessionManager.getSessionId() }, ctx.cwd);
    if (reply?.systemMessage) ctx.ui.notify(reply.systemMessage, "info");
  });

  pi.on("before_agent_start", async (event, ctx) => {
    if (event.messageType !== "user") return;
    continued = false;
    const reply = await hook(
      "user-prompt",
      { session_id: ctx.sessionManager.getSessionId(), prompt: event.prompt },
      ctx.cwd,
    );
    const context = reply?.hookSpecificOutput?.additionalContext;
    if (context) return { message: { customType: "software-design", content: context, display: false } };
  });

  pi.on("tool_result", async (event, ctx) => {
    if ((event.toolName !== "edit" && event.toolName !== "write") || event.isError) return;
    const reply = await hook(
      "post-edit",
      { session_id: ctx.sessionManager.getSessionId(), tool_input: { file_path: event.input.path } },
      ctx.cwd,
    );
    const context = reply?.hookSpecificOutput?.additionalContext;
    if (context) {
      return {
        content: [...event.content, { type: "text" as const, text: context }],
        structuredContent: event.structuredContent,
      };
    }
  });

  pi.on("agent_before_settle", async (_event, ctx) => {
    const reply = await hook(
      "stop",
      { session_id: ctx.sessionManager.getSessionId(), stop_hook_active: continued },
      ctx.cwd,
    );
    if (reply?.decision !== "block") return;
    continued = true;
    return {
      entries: [{ type: "custom_message" as const, customType: "software-design", content: reply.reason, display: true }],
      continue: true,
    };
  });
}
