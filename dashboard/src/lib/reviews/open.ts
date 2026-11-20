// Opening a review's page from outside the Reviews tab (ATM-575: the needs-you chip): in place, as the Reviews tab
// opens one, so its back button comes back here (ATM-686)
import { reviewPageOf } from '../address';
import { currentAddress, navigate } from '../navigation';

/** Open review `reference` of project `project` */
export function openReviewPage(project: string, reference: string): void {
  navigate(reviewPageOf(currentAddress(), project, reference));
}
