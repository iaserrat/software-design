export const meta = {
  name: 'deep-audit',
  description: 'Audit a codebase for design red flags, ranked by churn and verified',
  whenToUse: 'The user asked for a design or complexity audit of a repository or a large directory',
  phases: [
    { title: 'Scan', detail: 'run the check tool at tier B and rank findings by churn' },
    { title: 'Verify', detail: 'one flag-verifier per heavy tier B candidate' },
    { title: 'Read', detail: 'a cold reader per most-changed module' },
  ],
}

// args: { paths?: string[], verify?: number, modules?: number }
const paths = (args && args.paths) || []
const VERIFY = (args && args.verify) || 12
const MODULES = (args && args.modules) || 3

const FINDINGS = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          rule: { type: 'string' },
          flags: { type: 'string' },
          cite: { type: 'string' },
          tier: { type: 'string' },
          path: { type: 'string' },
          line: { type: 'number' },
          symbol: { type: 'string' },
          message: { type: 'string' },
          weight: { type: 'number' },
        },
        required: ['rule', 'cite', 'tier', 'path', 'line', 'message', 'weight'],
      },
    },
    total: { type: 'number' },
  },
  required: ['findings', 'total'],
}

const VERDICT = {
  type: 'object',
  properties: {
    verdict: { type: 'string', enum: ['CONFIRMED', 'REJECTED'] },
    evidence: { type: 'string' },
    exemption: { type: 'string' },
    fix: { type: 'string' },
  },
  required: ['verdict', 'evidence', 'fix'],
}

const READING = {
  type: 'object',
  properties: {
    usable_without_code: { type: 'boolean' },
    blocking_questions: { type: 'array', items: { type: 'string' } },
    misleading_names: { type: 'array', items: { type: 'string' } },
  },
  required: ['usable_without_code', 'blocking_questions'],
}

phase('Scan')
const scan = await agent(
  `Call the MCP tool mcp__plugin_software-design_software-design__check (load it with ToolSearch first) with ` +
    `tier "B" and paths ${JSON.stringify(paths)}. Return every finding exactly as the tool reports it, in the ` +
    `tool's order (highest weight first), and the total count.`,
  { label: 'check', schema: FINDINGS, effort: 'low' },
)
const findings = (scan && scan.findings) || []
log(`${findings.length} findings from the check tool`)

const tierA = findings.filter((f) => f.tier === 'A')
const candidates = findings.filter((f) => f.tier === 'B')
const toVerify = candidates.slice(0, VERIFY)
if (candidates.length > toVerify.length) {
  log(`verifying the ${toVerify.length} heaviest of ${candidates.length} tier B candidates; the rest stay unverified`)
}

// Hotspot modules: the directories holding the heaviest findings.
const dirs = []
for (const f of findings) {
  const dir = f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '.'
  if (!dirs.includes(dir)) dirs.push(dir)
  if (dirs.length === MODULES) break
}

const [verified, readings] = await Promise.all([
  pipeline(toVerify, (f) =>
    agent(
      `Verify this candidate red flag. Finding: ${JSON.stringify(f)}. Read its detector entry (id "${f.rule}") ` +
        `in rules.json at the plugin root, then follow your instructions.`,
      { agentType: 'software-design:flag-verifier', phase: 'Verify', label: `${f.rule} ${f.path}:${f.line}`, schema: VERDICT },
    ).then((v) => (v ? { ...f, ...v } : null)),
  ),
  pipeline(dirs, (dir) =>
    agent(`Read the public interface of the module at ${dir} and report as instructed.`, {
      agentType: 'software-design:cold-reader',
      phase: 'Read',
      label: `cold read ${dir}`,
      schema: READING,
    }).then((r) => (r ? { module: dir, ...r } : null)),
  ),
])

const confirmed = verified.filter(Boolean).filter((v) => v.verdict === 'CONFIRMED')
log(`${confirmed.length} of ${toVerify.length} verified candidates confirmed`)

return {
  total_findings: findings.length,
  tier_a: tierA,
  confirmed,
  rejected: verified.filter(Boolean).filter((v) => v.verdict === 'REJECTED'),
  unverified: candidates.slice(VERIFY),
  cold_reads: readings.filter(Boolean),
}
