# Lessons log

Tieto's challenge "Earned Trust in AI Code" asks each team to "name the failure modes you saw, and keep a short log of decisions", and lists a lessons log as a deliverable. This is that log for Witness: what we decided and why, what went wrong while building it during Junction 2026 (13–15 November), and what we would tell another team.

## Decisions

| When | Decision | Why |
|---|---|---|
| Fri | The old version of the code is the specification. We compare old against new and do not try to prove the new code "correct". | Nobody has a formal spec for a pull request. Everybody has the previous commit. |
| Fri | A solver finds the differences and a proof checker confirms them. Two different tools. | The path enumeration around the solver is our own code and can drop a case. Lean's coverage proof fails if it does. |
| Fri | No verdict comes from a language model. The AI compares proven findings with the ticket, and every statement must cite a record or it is dropped. | "The AI said it was fine" is the thing the challenge rules out. |
| Sat | What needs a person is decided by fixed rules in one config file. | A reviewer has to be able to ask why something was flagged and get the same answer every time. |
| Sat | Lines the comparison cannot cover are counted and shown on the page. | A proof over 28 of 29 lines is useful. A proof that hides the 29th is not. |
| Sat | The pull request's own tests are run on old and new code to tell declared changes from undeclared ones. | A change with a test that fails on the old code was meant. A change with no such test may not have been. |
| Sun | Findings are worded by what they do to you ("turns off three safety checks"), from fixed templates over the records. | Our earlier wording used the library's error codes, and people outside the codebase could not tell what was at stake. |
| Sun | An example written by an AI can illustrate a finding. It cannot change a finding's label or the status of the change. | See the first failure mode below. |

## Failure modes we saw

- **An AI-written example changed a proven finding's label.** The solver and Lean had established "the old code reports an error, the new code reports nothing". A model then wrote an example program, the pipeline ran it, saw a value come back as 0 and relabelled the finding "data loss". The run was real, but the choice of input was the model's. We had built our demo around that number before we noticed. Now only examples from the ticket or from the pull request's tests can relabel anything.
- **The page applied that rule more loosely than the pipeline.** The pipeline allowed only named sources. The page merely excluded one. An independent review found three inputs that slipped through.
- **Reading the code predicted the wrong behaviour, twice.** Before running anything we wrote down what the old code would do on the ticket's example. We were wrong about it twice: the old build did not just crash, it also reported the right error. Since then nothing goes on the page that was not run or proven.
- **Most of the largest change was not compared.** On the 28-file feature, 83 of 450 changed lines were compared, because new methods with loops could not be modelled. The page says so, and the change is marked for a full read.
- **One change had no proof at all.** The 237-file change makes its only real edit in a template file we have no parser for. It is shown as "compared as text", and 402 identical edits are folded into one item.
- **One error code stood for six different errors.** We first picked a description by name order, which was a guess. The compiler knows which one each line raises, so now we carry that through.
- **A fix can look right on the ticket and still be unnecessary.** One sample change tidies code for a bug that does not occur on the commit it was written against. The ticket's own example behaves the same before and after.
- **Words drift faster than code.** "Switches off three build errors" could be read as fixing errors. Two cards rendered identical text for different findings. A proof sentence said "seven conditions" above five. Each was caught by a person reading the rendered page, not by a test, so we added tests that read every string.

## What we would tell another team

1. Decide what is allowed to change a label before you build the demo. Evidence is only as trustworthy as whatever gets attached to it afterwards.
2. Run it. Predictions made by reading a diff were our own first failure, and they are exactly what an AI reviewer does.
3. Show what was not checked. It costs one line on the page and it is what makes the rest believable.
4. Keep the finder and the checker separate, and let a third party review the result. Our own review of our own page missed what an independent one found in an hour.
5. Spend time on the words. A proof nobody can read does not earn any trust.

## Limits that remain

- The step from C# to the logical model is not itself proved.
- A proof covers the changed code and the helpers it calls. Unchanged code around it is treated as a black box with stated assumptions.
- Each language needs its own translator. C# works; Rust and TypeScript are first steps.
- All of this has run on four sample changes of one repository.
