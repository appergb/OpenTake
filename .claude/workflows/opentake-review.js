// OpenTake 可复用「自审」workflow:对一个模块跑 审核→修复→复审 闭环,直到通过或用尽轮次。
// 用法:Workflow({ name: "opentake-review", args: {
//   module: "opentake-render",                  // 显示名
//   repo: "/abs/path/to/OpenTake",              // 仓库根(绝对路径)
//   targets: "crates/opentake-render/src",      // 审核的代码路径(逗号分隔)
//   upstreamRefs: ".../Preview/CompositionBuilder.swift", // 1:1 参考(可空)
//   specRefs: "docs/specs/render-SPEC.md",       // 规格/设计文档(可空)
//   verifyCmds: ["cargo test -p opentake-render", "cargo clippy -p opentake-render --all-targets -- -D warnings"],
//   rounds: 2                                     // 最多 审→修 轮数(默认 2)
// }})
// 返回:{ approved, rounds_run, final_review, history }

export const meta = {
  name: 'opentake-review',
  description: 'OpenTake 模块自审闭环:审核(子Agent)→修复(子Agent)→复审,直到通过或用尽轮次。可复用。',
  phases: [
    { title: '自审闭环', detail: '审核→(不通过则)修复→复审,逐轮收敛' },
  ],
}

const a = args || {}
const REPO = a.repo || '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake'
const MODULE = a.module || 'module'
const TARGETS = a.targets || 'crates'
const UPSTREAM = a.upstreamRefs || '(无,新增功能)'
const SPECS = a.specRefs || '(无)'
const VERIFY = Array.isArray(a.verifyCmds) && a.verifyCmds.length ? a.verifyCmds : ['cargo build --workspace', 'cargo test --workspace']
const ROUNDS = Number.isInteger(a.rounds) ? a.rounds : 2
const verifyText = VERIFY.map(c => '`' + c + '`').join(' 和 ')

const REVIEW_SCHEMA = {
  type: 'object', additionalProperties: false,
  required: ['verdict', 'build_passes', 'fidelity_ok', 'issues', 'summary'],
  properties: {
    verdict: { type: 'string', enum: ['approved', 'changes_requested'] },
    build_passes: { type: 'boolean', description: '亲自跑验证命令是否全部通过' },
    fidelity_ok: { type: 'boolean', description: '是否符合 1:1 复刻/规格(无对齐偏差)' },
    issues: { type: 'array', items: { type: 'object', additionalProperties: false,
      required: ['severity', 'file', 'problem', 'fix'],
      properties: { severity: { type: 'string', enum: ['critical', 'high', 'medium', 'low'] }, file: { type: 'string' }, problem: { type: 'string' }, fix: { type: 'string' } } } },
    summary: { type: 'string' },
  },
}

phase('自审闭环')

const reviewPrompt = (round) =>
  '你是严格的代码审核 Agent,审核模块「' + MODULE + '」(第 ' + round + ' 轮)。仓库根:' + REPO + '。\n' +
  '审核代码路径:' + TARGETS + '\n' +
  '1:1 参考(上游):' + UPSTREAM + '\n' +
  '规格/设计:' + SPECS + '\n' +
  '核查重点:① 若有上游参考,是否 1:1 忠实复刻(算法/常量/边界/单位/键名,逐字对齐,不允许"差不多");② 跨 crate/跨层函数调用签名是否匹配、类型是否正确;③ 正确性与惯用法;④ 是否真能构建——你必须亲自在 ' + REPO + ' 用 Bash 跑:' + verifyText + ',据实记录结果。\n' +
  '只报真实问题。verdict=approved 仅当验证命令全部通过且无 1:1/规格偏差(medium/low 的"已声明后续/脚手架"缺口不应阻断 approved,但要在 issues 里列出)。不要改代码、不执行 git。'

const fixPrompt = (review, round) => {
  const crit = (review.issues || []).filter(i => i.severity === 'critical' || i.severity === 'high')
    .map(i => '[' + i.severity + '] ' + i.file + ': ' + i.problem + ' → 建议:' + i.fix).join('\n')
  return '你是修复 Agent(模块「' + MODULE + '」,第 ' + round + ' 轮,仓库 ' + REPO + ')。修复以下审核发现的阻断级问题:\n' +
    (crit || review.summary) + '\n' +
    '外科手术式改动,不要无关重构。改完必须亲自跑通:' + verifyText + '。不执行 git。返回修复内容与验证结果。'
}

const history = []
let final = null
for (let round = 1; round <= ROUNDS; round++) {
  const review = await agent(reviewPrompt(round), { label: '审:' + MODULE + ' #' + round, phase: '自审闭环', schema: REVIEW_SCHEMA, effort: 'max' })
  history.push({ round, review })
  final = review
  if (!review) break
  const blocking = (review.issues || []).some(i => i.severity === 'critical' || i.severity === 'high')
  if (review.verdict === 'approved' || !blocking) break
  await agent(fixPrompt(review, round), { label: '修:' + MODULE + ' #' + round, phase: '自审闭环', effort: 'max' })
}

const approved = !!final && (final.verdict === 'approved' || !(final.issues || []).some(i => i.severity === 'critical' || i.severity === 'high'))
return { module: MODULE, approved, rounds_run: history.length, final_review: final, history }
