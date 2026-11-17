// A review's page is a window of its own (ATM-574): its title, which the page sets once the review is loaded, for the
// app's title bar (the browser's tab title is set with it); and, in the app, each review's name by its project and the
// reference its tab's address names (`<project>:CR-7` → "CR-7: Trim the names"), for the tab's label once shown.
export const reviewWindow = $state<{ title: string; names: Record<string, string> }>({ title: '', names: {} });
