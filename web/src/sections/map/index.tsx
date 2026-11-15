import type { BundleData, SectionModule } from '../../shell';
import { MapSection } from './Map';
import { mapLabels } from './labels';
import { linksOf } from './layout';

/** `6 blocks · 1 outside`, plus the mechanical edits folded away: `· 402 identical edits`. */
export function headline(d: BundleData): string {
  const out = linksOf(d).filter((l) => l.placement === 'outside').length;
  const items = [mapLabels.blocks(d.blocks.length)];
  if (out) items.push(`${out} ${mapLabels.outside}`);
  const groups = ((d.records.groups as { groups?: { rule: string; count?: number; parts?: string[] }[] } | null)?.groups ?? []);
  // The largest group of each rule leads; smaller ones are counted beside it.
  const byRule = new Map<string, number[]>();
  for (const g of groups) byRule.set(g.rule, [...(byRule.get(g.rule) ?? []), g.count ?? g.parts?.length ?? 0]);
  for (const [rule, counts] of byRule) {
    const label = (mapLabels.rule as Record<string, (n: number) => string>)[rule];
    const [top, ...rest] = counts.sort((a, b) => b - a);
    const more = rest.reduce((a, b) => a + b, 0);
    items.push((label ? label(top) : `${top} ${rule.replace(/-/g, ' ')}`) + (more ? ` +${more}` : ''));
  }
  return items.join(' · ');
}

export const sections: SectionModule[] = [{ slot: 'map', Component: MapSection, headline }];
