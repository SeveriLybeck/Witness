// Group 18: the review (with the one list of decisions) and Sign off.
import reviewCss from './review.css?raw';
import type { SectionModule } from '../../shell';
import { AiReview, FullTextButton } from './Review';
import { SessionEffects, SignOff } from './SignOff';
import { ReviewStatus } from './StatusMark';

if (typeof document !== 'undefined' && !document.getElementById('witness-review-css')) {
  const el = document.createElement('style');
  el.id = 'witness-review-css';
  el.textContent = reviewCss;
  document.head.appendChild(el);
}

export const sections: SectionModule[] = [
  { slot: 'review', Component: AiReview, Tools: FullTextButton, Headline: ReviewStatus },
  { slot: 'signoff', Component: SignOff, Always: SessionEffects },
];
