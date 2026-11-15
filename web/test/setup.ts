import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => cleanup());

/** jsdom has no matchMedia; tests set `window.innerWidth` and queries follow it. */
if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = (query: string) => {
    const max = /max-width:\s*(\d+)px/.exec(query);
    const reduce = /prefers-reduced-motion:\s*reduce/.test(query);
    const min = /min-width:\s*(\d+)px/.exec(query);
    const matches = max ? window.innerWidth <= Number(max[1]) : min ? window.innerWidth >= Number(min[1]) : reduce ? false : false;
    return {
      matches, media: query, onchange: null,
      addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false,
    } as MediaQueryList;
  };
}
if (typeof Element !== 'undefined' && !Element.prototype.scrollIntoView) Element.prototype.scrollIntoView = () => {};
