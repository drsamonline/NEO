# Constitution

The non-negotiables. Everything else in this repository is negotiable; these are
not, because each one exists to prevent a specific failure that has already happened
here at least once.

## 1. Never invent completed functionality

A stub is honest. A plausible implementation that does not work is not. Four
documents in this repository once described a project that did not exist, and the
cost was a reader who could not trust any of them.

## 2. A milestone is done when its acceptance check runs and passes

Not when the code exists. Not when it builds locally. The check is named in
`DEV_PLAN.md`, and a green tick that came from a different check is not a green tick.

## 3. A backend that cannot express something rejects it

It never emits a placeholder that compiles and misbehaves. That is the one failure a
user cannot see, and it is worse than a clear error.

## 4. Numbers come from a command

`wc -l`, a CI log, a run id. Never from memory and never estimated. The README was
once wrong about the compiler's size by 2.5×, and every other number on the page
became worthless with it.

## 5. Say what you did not verify

Every pull request here has a "not verified" section. A change that hides what it
could not run is a change nobody can review.

## 6. A flaky check is worse than no check

It teaches people to re-run CI instead of reading it, and from then on the suite
tells you nothing. This is why the wall-clock benchmark gate is not on pull
requests, and why the coverage floor has a written rule for when it may move.

## 7. Documentation is checked, not asserted

A document that claims something the code does not do is a bug in the document. The
grammar, the error codes, the editor grammar and this documentation index are each
compared against the code by a script in CI.

## 8. Correct a mistake out loud

`DEV_PLAN.md` records a claim that was wrong — the WebAssembly `testsuite` submodule
— and says so rather than deleting the line. A plan that quietly drops an item is
worse than one that admits the error.
