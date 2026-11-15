// Every chrome label the shell renders (D9). Checked by test/wording.test.ts.

export const slotTitles = {
  review: 'Review',
  proof: 'Behaviour changes',
  playground: 'Try it',
  map: 'Code map',
  decide: 'Decide',
  signoff: 'Sign off',
} as const;

export const labels = {
  close: 'Close',
  files: (n: number) => `${n} files`,
  old: 'Old',
  new: 'New',
  expandUp: 'Expand up',
  expandDown: 'Expand down',
  copyPath: 'Copy path',
  copied: 'Copied',
  notInOld: 'not in old',
  notInNew: 'not in new',
  palette: 'Commands',
  keys: 'Keyboard',
  requirement: 'Requirement',
  code: 'Code',
  loading: 'loading',
  noFile: 'no file text',
  bundles: 'Reviews',
  themeLight: 'Theme light',
  themeDark: 'Theme dark',
  themeSystem: 'Theme system',
  openFiles: 'Files',
  closeCode: 'Close code',
  stateRunning: 'running',
  stateNotRun: 'not run',
  stateUnavailable: 'unavailable',
  product: 'Witness',
  tests: 'Tests',
  change: 'Change',
  claims: 'Claims',
  notAsked: 'Not asked for',
  notFound: 'Not found',
  decisions: 'Decisions',
  compared: 'Lines compared',
  status: 'Status',
  noDescription: 'no description',
  evidence: 'Evidence',
  computed: 'computed, no AI',
  unified: 'Unified',
  split: 'Split',
  diffView: 'Diff view',
} as const;

export const banned = ['proved', 'safe', 'verified', 'trusted', 'reproduced', 'correct'];

/** All static label strings, for the wording test. */
export function allLabels(): string[] {
  const out: string[] = Object.values(slotTitles);
  for (const v of Object.values(labels)) {
    if (typeof v === 'function') out.push(v(4));
    else out.push(v);
  }
  return out;
}
