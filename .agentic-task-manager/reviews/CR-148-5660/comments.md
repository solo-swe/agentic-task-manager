## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-24T16:32:45Z · c1
  With this change, sidebar becomes essential for the user, and a hidden sidebar can be very confusing for the unitiated user. So, let's create a task, which shows a very thin, icons-only sidebar when it is hidden (instead of showing nothing) and a hover would temporarily show the sidebar so the user can click on something. share the link to the task here.
  - **claude:ba530ebb** · 2026-11-24T16:34:10Z · c2
    Created: [ATM-771](atm://projects/agentic-task-manager?task=ATM-771) *The hidden sidebar is a thin rail of icons; a hover shows it*, in this sprint, after ATM-753.

    With the sidebar hidden, a thin strip of the scope's icons (with their badges) stays at the window's left edge. Hovering or focusing it shows the full sidebar over the page until you move away, and a click works from either. Show Sidebar still pins it open.
