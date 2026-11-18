// Opening a review's page from outside the Reviews tab (ATM-575: the needs-you chip): in the app, in a tab of its own,
// as the Reviews tab opens one; in the browser, in place
import { reviewPageOf } from '../address';
import { inApp } from '../api/http';
import { currentAddress, navigate } from '../navigation';
import { tabActions } from '../tab-actions';

/** Open review `reference` of project `project` */
export function openReviewPage(project: string, reference: string): void {
  const address = reviewPageOf(currentAddress(), project, reference);
  if (inApp) tabActions().open(address);
  else navigate(address);
}
