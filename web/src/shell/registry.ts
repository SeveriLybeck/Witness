import type { ComponentType } from 'react';
import type { BundleData, SectionState } from './data';
import type { Slot } from './context';

export interface SectionProps {
  data: BundleData;
  /** State of the bundle section feeding this slot (`bundle.sections`); `not-run` when absent. */
  state: SectionState;
}

/** What `src/sections/<name>/index.tsx` exports as `sections`. */
export interface SectionModule {
  slot: Slot;
  Component: ComponentType<SectionProps>;
  /** Values shown beside the title, e.g. `3 differences`. */
  headline?: (data: BundleData) => string;
  /** A headline that needs more than bundle data (e.g. the session's open count); wins over `headline`. */
  Headline?: ComponentType<SectionProps>;
  /** Controls at the right end of the open slot's title row (e.g. the review's `Full text`). */
  Tools?: ComponentType<SectionProps>;
  /** Rendered whether or not the slot is folded; for effects that must not stop when it folds. Draws nothing. */
  Always?: ComponentType<SectionProps>;
}

/** Bundle section that feeds each page slot. */
export const slotState: Record<Slot, string> = {
  review: 'review', proof: 'proof', playground: 'playground', map: 'map', decide: 'rules', signoff: 'rules',
};

type Mod = { sections?: SectionModule[] };

export function collect(mods: Record<string, Mod>): Partial<Record<Slot, SectionModule>> {
  const out: Partial<Record<Slot, SectionModule>> = {};
  for (const path of Object.keys(mods).sort()) {
    for (const s of mods[path].sections ?? []) {
      if (out[s.slot]) throw new Error(`slot ${s.slot} registered twice (${path})`);
      out[s.slot] = s;
    }
  }
  return out;
}

export const registered = collect(import.meta.glob<Mod>('../sections/*/index.tsx', { eager: true }));
