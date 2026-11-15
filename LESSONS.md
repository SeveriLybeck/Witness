# Lessons log

Tieto's challenge asked us to name the failure modes we saw and keep a short log of decisions. This is that log. It covers what we decided while building Witness at Junction 2026, what went wrong, and what we still can't do.

## What we decided

We compare the new code against the old code instead of trying to prove the new code is correct. Nobody has a formal spec for a pull request, but everyone has the previous commit, so that is what we treat as the spec.

We use two tools for the proof, not one. Z3 finds the differences and Lean checks them. The reason is that the code around the solver is ours and it can miss a case. If it does, Lean's check that the list covers everything fails.

The AI never gives a verdict. It only compares the proven list of changes to the ticket, and if a statement it makes doesn't point to a real record we throw the statement away.

What needs a human is decided by fixed rules in a config file. We wanted the reviewer to be able to ask why something was flagged and get the same answer every time.

We show the lines we could not compare. On our main example that is 1 line out of 29. Hiding it would have made the rest worth less.

We word findings by what they do to the developer. "Turns off three safety checks" instead of error codes like MsgPack004, because nobody outside that codebase knows what those mean.

## What went wrong

The big one: we let an AI-written example change the label of a proven finding. Lean had proven that the old code reports an error and the new code reports nothing. Then a model wrote an example program, we ran it, a value came back as 0, and the pipeline called the finding "data loss". The run was real but the input was picked by an AI, so the label wasn't deterministic at all. We had already built our demo around that number when we noticed. Now only examples from the ticket or from the PR's own tests can change a label.

After fixing that in the pipeline we found the web page was still checking it more loosely. A separate review agent found three inputs that got through.

Early on we tried to predict what the old code does just by reading it, and we were wrong twice. The old build didn't only crash, it also reported the correct error. After that we stopped putting anything on the page that wasn't run or proven.

On the biggest sample change we could only compare 83 of 450 lines, because our translator can't handle new methods with loops yet. On the 237-file change we got no proof at all, because the only real edit is in a template file we have no parser for. Both are shown as such on the page.

One error code turned out to cover six different errors, and for a while we showed the description of the wrong one.

The wording kept going wrong. "Switches off three build errors" sounded like it fixes errors. Two different findings showed the exact same text. One proof said "seven conditions" above a list of five. We only caught these by reading the actual page, so now there are tests that read every string on it.

## What we still can't do

- The translation from C# to the logical model is not proven itself.
- The proof follows the changed code and the helpers it calls. Unchanged code around it is a black box.
- Every language needs its own translator. C# works, Rust and TypeScript are only started.
- We have run this on four sample changes from one repository.
