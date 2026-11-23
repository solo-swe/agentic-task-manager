## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-19T07:45:54Z · c1
  what are the risks with this approach? We are changing the compmd part to help with this based on your earlier feedback. Are we going to need to change again when compmd changes land?
  - **claude:0f56a916** · 2026-11-19T07:47:49Z · c2
    No rework when CompMD lands. Per part:
    - **The seam** (`compmd-runtime.ts`): built for it. When CompMD takes a shared `stylesheet` and a `scheme`, they are passed in `sandboxOf()` (or on `open`): one place, `CompMd.svelte` untouched.
    - **Jump hygiene** (no closes while a jump settles) and **no page for a blank run**: they are just fewer opens and closes, cheaper with or without CompMD sharing. They can stay; nothing in them depends on how CompMD styles a page.
    - **The placeholder containment**: independent of CompMD.
    - **The test bounds**: will need tightening, not changing code. Today they allow CompMD 0.7.1s own reads (4 a page); with the fix, reads should be ~0 and sheet sets ~1, so the bounds drop to what CompMD then promises, which makes the test guard the fix.

    Risks I see now:
    1. **The blank-run fallback** shows the plain text as written instead of a CompMD page, relying on it being exactly as tall as CompMDs drawing (it is the same fallback a run shows while it waits its turn, and the Split row tests pass). If CompMD changes how a code block is drawn (CMD-250 draws each line as an element), the two could differ in height and a blank side would drift from the other. Mitigation: the row-alignment tests would catch it; once CompMD opens are cheap, this special case can simply be removed.
    2. **The shared sandbox**: CompMD 0.7.1 keeps each pages state in `open()`, not the sandbox, so sharing is safe today. If CompMD ever put per-page state on the sandbox, pages would share it. The CompMD tasks per-document context makes this the intended use, so it should only get safer.
    3. **Deferred give-back** keeps a card made a moment longer after a jump (only while the jump holds, then given back as before; the memory test passes every run).
    4. **Not verified in the app yet**: the counts are from Chromium and Playwrights WebKit; your WKWebView should see the same fewer triggers, but each remaining CompMD open still costs a full restyle until CompMDs change.
