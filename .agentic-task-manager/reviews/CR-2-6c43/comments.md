## docs/code-review-design.md:165 · round 1 · t1
> | `changes_requested` | sent back for fixes; it still takes comments, replies and a reviewer's further words (changes asked for again keeps it here, with what they wrote; ATM-597) | a reviewer, when submitting |

- [x] **Hakan Serce** · 2026-11-18T05:40:47Z · c1
  if it is already sent back for fixes, sending again back for fixes is confusing and mentally wrong. Why not just make it publish comments or something like that? But this on the other hand, implies that comments are not sent directly and they are draft until sent. I am not sure if we have that concept here, ie draft vs published. It could be good to have that separation actually. Maybe when writing comments, we can have two options, or three actually: Save Draft, Send Directly, Cancel. Then a Publish Comments/Send back for correction/Approve button can be up there. If it is already Sent back for correction, maybe the button name shows as Publish Comments.
  - **claude:0f56a916** · 2026-11-18T05:41:43Z · c2
    Agreed: "request changes" on a review already sent back is the wrong word for what you meant, which was "here are more comments". Here is a proposal in two steps; tell me which parts you want.

    **1. Now, in this review: no second "send back"**
    - Sent back, the decision panel shows **Publish comments** (with an optional summary) where *Request changes* was; Approve stays disabled until the author's next round (CR-6).
    - The rule goes back to refusing a second request for changes, so the CLI and the page agree. Your words are kept either way, as a comment.

    **2. A follow-up task: draft comments, sent together** (what GitHub calls "start a review")
    - **Writing a comment** offers **Add to my review** (a draft only you see, kept until you send it), **Comment now** (sent at once, as today) and **Cancel**.
    - **The top button** counts your drafts and sends them, with the summary:
      - an open review: *Comment*, *Request changes* or *Approve*;
      - a review sent back: *Publish comments*.
    - **Agents** (the watch, `wait`) hear nothing until the drafts are sent, so an agent never acts on half a review.
    - **Where drafts live** (my recommendation): in your own atm state, not in the review's folder. The folder is committed to git, so a draft there would be visible to anyone with the repository before you sent it. Drafts are anchored like any comment and move with the code across rounds.
    - **One question:** should replies be drafts too, or only new comments? GitHub drafts both; I would start with both, for one model.

    If that's right, I'll do step 1 in this review and add step 2 as a task in this sprint.
  - **claude:0f56a916** · 2026-11-18T06:37:59Z · c3
    Done in round 2, as step 1: a review sent back shows **Publish comments** and is never sent back again (the rule refuses it too). Replies are drafts too, as I suggested. Step 2, drafts sent together, is **ATM-612** in this sprint.
