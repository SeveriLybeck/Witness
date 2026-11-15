// Public interface for sections: import from '../../shell'.
export { useShell, useCommands, slots } from './context';
export type { Shell, Selection, CodeTarget, Command, Slot } from './context';
export type { SectionModule, SectionProps } from './registry';
export { Loc, LevelGlyph, DiffNumber, Sheet } from './parts';
export { basename, blocksInFile, changedLines, firstChange, levelOf, ruleName, ruleNames, repoPaths, lostValue, replayCounts, claimOf, claimHasTitle } from './data';
export { Prose } from './prose';
export { groupDifferences, groupOf, behaviourKey, behaviourLabel, behaviourPattern } from './behaviour';
export type { DiffGroup } from './behaviour';
export type {
  BundleData, Bundle, Records, RecordName, DataSource, SectionState, StateName, Location, FileEntry, Part, Block,
  Difference, Level, FileText,
} from './data';
export { fontSizes, space, colors, motionMs } from './tokens';
export { banned } from './labels';
