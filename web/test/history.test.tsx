import { describe, expect, it } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';
import { fixtureSource, readRecord, renderPage } from './harness';

describe('files index folder history (D44)', () => {
  it('shows undeclared counts beside folders, nothing for zero or absent', async () => {
    const base = fixtureSource('2283');
    const files = readRecord('2283', 'files') as { files: { file: string; history?: unknown }[] };
    for (const f of files.files) {
      if (f.file.includes('CodeAnalysis/')) f.history = { folder: 'src/MessagePack.SourceGenerator/CodeAnalysis', undeclared: 3, reviews: 5 };
      if (f.file.startsWith('doc/')) f.history = { folder: 'doc/analyzers/', undeclared: 0, reviews: 2 };
    }
    const source = { ...base, record: async (k: string, n: Parameters<typeof base.record>[1]) => (n === 'files' ? files : base.record(k, n)) };
    await renderPage('2283', { sections: {}, source });
    fireEvent.click(screen.getByRole('button', { name: '4 files' }));
    const counts = [...document.querySelectorAll('[data-folder-count]')];
    expect(counts.map((c) => c.textContent)).toEqual(['3']);
    expect(counts[0].getAttribute('aria-label')).toBe('3 undeclared before');
    expect(counts[0].closest('.folder-row')!.textContent).toContain('CodeAnalysis/');
  });
});
