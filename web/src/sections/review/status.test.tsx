// The status rule on the four real bundles and on a synthetic quiet change.
import { describe, expect, it } from 'vitest';
import { realData, type Change } from '../../../test/harness';
import { factsOf, statusOf } from './status';
import { diagnosticSeverities, plainLine, questionGroups } from './cites';
import { reviewBanned } from './labels';

const expected: Record<Change, { level: string; label: string; reasons: string[]; asked: string; unasked: string }> = {
  '2283': {
    level: 'red', label: 'Needs human review',
    reasons: ['1 data loss', '2 diagnostics dropped', '2 not asked for', '4 to decide'],
    asked: '2 of 2 claims have evidence', unasked: '2 changes, 1 loses data: 42 → 0',
  },
  '2234': {
    level: 'orange', label: 'Possible problems',
    reasons: ['2 not asked for', '2 to decide'],
    asked: '1 of 1 claim has evidence', unasked: '2 changes',
  },
  '2297': {
    level: 'red', label: 'Needs human review',
    reasons: ['0 of 29 lines compared', '1 not asked for', '1 claim without evidence', '2 to decide'],
    asked: '1 of 2 claims have evidence', unasked: '1 change',
  },
  '2129': {
    level: 'red', label: 'Needs human review',
    reasons: ['1 new exception', '83 of 450 lines compared', '4 not asked for', '15 to decide'],
    asked: '2 of 2 claims have evidence', unasked: '4 changes, 1 new exception',
  },
};

describe('status, by fixed rules over the records', () => {
  for (const c of Object.keys(expected) as Change[]) {
    it(`${c}: ${expected[c].label}`, async () => {
      const data = await realData(c);
      const s = statusOf(data);
      const e = expected[c];
      expect({ level: s.level, label: s.label, reasons: s.reasons }).toEqual({ level: e.level, label: e.label, reasons: e.reasons });
      expect(factsOf(data)).toEqual({ asked: e.asked, unasked: e.unasked });
      for (const t of [s.label, ...s.reasons, e.asked, e.unasked]) for (const w of reviewBanned) expect(t.toLowerCase()).not.toContain(w);
      expect(s.label.split(' ').length).toBeLessThanOrEqual(3);
    });
  }

  it('green: every claim has evidence, nothing beyond the description, nothing to decide', async () => {
    const real = await realData('2234');
    const rec = real.records['ai-review'] as { statements: { heading: string }[] };
    const data = {
      ...real,
      records: {
        ...real.records,
        questions: { questions: [] },
        'ai-review': { ...rec, statements: rec.statements.filter((s) => s.heading === 'matches') },
      },
    };
    expect(statusOf(data)).toEqual({ level: 'green', label: 'All checks passed', reasons: [] });
    expect(factsOf(data)).toEqual({ asked: '1 of 1 claim has evidence', unasked: 'nothing' });
  });

  it('diagnostic lines say what the user sees, then the code: severity from a replay when one states it', async () => {
    const real = await realData('2283');
    const lines = (d: typeof real) => questionGroups(d).map((g) => plainLine(d, g).line);
    expect(lines(real)).toContain('Analyzer diagnostic MsgPack016 no longer shown (reportDiagnostic not called)');
    const rp = real.records.replays as { replays: object[] };
    const withSev = { ...real, records: { ...real.records, replays: { ...rp, replays: [...rp.replays, { difference: null, old: 'errors MsgPack016, MsgPack005; warnings CS8785', new: '0 warnings' }] } } };
    expect(diagnosticSeverities(withSev).get('MsgPack016')).toBe('error');
    expect(lines(withSev)).toContain('Analyzer error MsgPack016 no longer shown (reportDiagnostic not called)');
    // several in one decision are counted and listed
    const g = questionGroups(withSev).find((x) => x.differences.includes('d1'))!;
    const d1 = withSev.differences.find((d) => d.id === 'd1')!;
    const many = { ...g, behaviour: { ...g.behaviour!, members: [{ ...d1, old: 'reports MsgPack004, MsgPack005, MsgPack016' }] } };
    expect(plainLine(withSev, many).line).toBe('3 analyzer errors no longer shown: MsgPack004, MsgPack005, MsgPack016');
  });

  it('the AI alone cannot make a change red, nor green while computed evidence is red', async () => {
    const real = await realData('2283');
    const quiet = { ...real, records: { ...real.records, 'ai-review': { recording: 'k', statements: [{ heading: 'matches', text: 'All fine', cites: ['d1'] }], dropped: [] }, questions: { questions: [] } } };
    expect(statusOf(quiet).level).toBe('red');
    const twoClean = await realData('2234');
    const noisy = { ...twoClean, records: { ...twoClean.records, questions: { questions: [] } } };
    expect(statusOf(noisy).level).toBe('orange');
  });
});
