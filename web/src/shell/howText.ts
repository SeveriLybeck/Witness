// The text of "How it works": what each kind of evidence is and what it does not show, for a C# developer.
// Prose, like the requirement text: sentences are allowed here and nowhere else in the chrome (D9).
export type HowTopic = 'status' | 'compare' | 'tests' | 'replay' | 'map' | 'ai' | 'signoff';
export interface HowItem { id: HowTopic; title: string; shows: string; limit: string; tools?: string }

export const howTitle = 'How it works';
export const howLead = 'Does this change do what it says, and what else did it do?';
export const howLimit = 'Does not show';
export const howTools = 'Tools';

/** How the status beside the review is set: fixed rules over the records, never the AI. */
export const howStatus = {
  title: 'Status',
  text: 'Fixed rules, not the AI. Red: lost data, a dropped diagnostic, a new exception, or under half the lines compared. Orange: unasked-for changes, claims without evidence, or open decisions. Green: neither.',
};

export const howItems: HowItem[] = [
  {
    id: 'compare',
    title: 'Behaviour changes, for every input',
    shows: 'Each changed method is compared on both commits for all inputs. A difference is where they end differently: an exception, a lost diagnostic, a lost value.',
    limit: 'Lines that could not be compared. They are counted, never guessed.',
    tools: 'Differences come from an SMT solver over a model of both versions; a second, independent check is a Lean proof of each difference.',
  },
  {
    id: 'tests',
    title: 'Tests, run on old and new',
    shows: 'The pull request’s tests run on old and new. Each changed block is then reverted alone to see which test notices.',
    limit: 'Whether a test checks the right thing. New tests passing on old code are hanging.',
  },
  {
    id: 'replay',
    title: 'Differences, replayed',
    shows: '“Try it” runs a triggering input on old and new, so you see the exception or lost value yourself.',
    limit: 'More than that one input. Without a working input it says not replayed.',
  },
  {
    id: 'map',
    title: 'Code map',
    shows: 'Changed code as blocks and who calls what. Fixed rules set importance. Identical edits fold into one entry.',
    limit: 'That an important block is wrong. Three bars mean read this.',
  },
  {
    id: 'ai',
    title: 'AI summary',
    shows: 'The box at the top: the AI’s reading of the evidence against the description. All else is computed without AI.',
    limit: 'The code: it reads none. It approves nothing and changes no question or gate.',
  },
  {
    id: 'signoff',
    title: 'Decide and sign off',
    shows: 'Rules raise questions; you answer. Approval waits for every answer, one random block and each important block opened.',
    limit: 'What you understood. Only what you opened and answered.',
  },
];
