import shellCss from './shell.css?raw';
import { tokenCss } from './tokens';

export { shellCss };

/** Injects tokens and shell styles once. Sections add their own CSS with `?raw` or a CSS import. */
export function installStyles(doc: Document = document) {
  if (doc.getElementById('witness-shell-css')) return;
  const el = doc.createElement('style');
  el.id = 'witness-shell-css';
  el.textContent = tokenCss() + '\n' + shellCss;
  doc.head.appendChild(el);
}
