import { createRoot } from 'react-dom/client';
import { App } from './shell/App';
import { httpSource } from './shell/data';
import { installStyles } from './shell/styles';

installStyles();
createRoot(document.getElementById('root')!).render(<App source={httpSource()} />);
