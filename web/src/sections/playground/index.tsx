import { replayCounts, type BundleData, type SectionModule } from '../../shell';
import { Playground } from './Playground';

/** What was replayed live, as plain values: `4 of 7 replayed · reported bug replayed`. */
export function headline(data: BundleData): string {
  const c = replayCounts(data);
  if (!c.total) return '';
  return `${c.replayed} of ${c.total} replayed · reported bug ${c.issue ? 'replayed' : 'not replayed'}`;
}

export const sections: SectionModule[] = [{ slot: 'playground', Component: Playground, headline }];
