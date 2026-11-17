// A review's page is a window of its own (ATM-574): its title, which the page sets once the review is loaded, for the
// app's title bar (the browser's tab title is set with it).
export const reviewWindow = $state({ title: '' });
