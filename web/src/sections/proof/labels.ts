// Copy deck for the proof section (D2, D9). Checked by proof.test.tsx against `banned`.
// Wording is a C# developer's: no tool names in the chrome.

export const kindLabels: Record<string, string> = {
  'crash-fixed': 'crash fixed',
  crash: 'new exception',
  'silent-failure': 'silent failure',
  'null-access': 'null access',
  'output-changed': 'output change',
  'new-dependency': 'new dependency',
  'public-api': 'public API',
  'data-loss': 'data loss',
};

export const checkLabels = {
  checked: 're-checked by a second checker',
  'model-checked': 're-checked, with a model-written argument',
  'not-checked': 'not re-checked',
  checking: 'checking',
  'file-comparison': 'file comparison',
  'api-comparison': 'public API comparison',
} as const;

export const categoryLabels: Record<string, string> = {
  declared: 'Declared',
  'new-api': 'New API',
  hanging: 'Hanging',
  withdrawn: 'Withdrawn',
  'not-run': 'Not run',
};
export const categoryOrder = ['declared', 'new-api', 'hanging', 'withdrawn', 'not-run'];

export const proofLabels = {
  differences: (n: number) => `${n} ${n === 1 ? 'difference' : 'differences'}`,
  groups: (n: number) => `${n} ${n === 1 ? 'behaviour' : 'behaviours'}`,
  notChecked: (n: number) => `${n} not re-checked`,
  unchecked: 'not re-checked',
  lost: (v: string | number) => `value lost: ${v}`,
  modelled: (m: number, c: number) => `${m} of ${c} lines compared`,
  notReplayed: 'not replayed',
  replayed: 'replayed',
  replay: 'Replay',
  assumptions: (n: number) => `${n} ${n === 1 ? 'assumption' : 'assumptions'}`,
  notModelled: 'not compared',
  notModelledN: (n: number) => `${n} ranges not compared`,
  cover: 'cases cover the named conditions',
  identical: 'identical in the model',
  perIteration: 'per iteration',
  anyLength: 'any length',
  facts: (n: number) => `${n} ${n === 1 ? 'fact' : 'facts'}`,
  noTest: 'no test',
  testsN: (n: number) => `${n} ${n === 1 ? 'test' : 'tests'}`,
  tests: 'Tests',
  before: 'Before',
  after: 'After',
  checks: 'Checks',
  details: 'Details',
  output: 'Output',
  difference: (n: number) => `Difference ${n}`,
  when: 'When',
  old: 'Old',
  new: 'New',
  kind: 'Kind',
  location: 'Location',
  block: 'Block',
  check: 'Check',
  more: (n: number) => `+${n}`,
  state: { running: 'running', 'not-run': 'not run', unavailable: 'unavailable', ready: '' } as Record<string, string>,
};

export function allProofLabels(): string[] {
  const out = [...Object.values(kindLabels), ...Object.values(checkLabels), ...Object.values(categoryLabels)];
  for (const v of Object.values(proofLabels)) {
    if (typeof v === 'function') out.push((v as (a: number, b: number) => string)(2, 3));
    else if (typeof v === 'string') out.push(v);
  }
  return out;
}
