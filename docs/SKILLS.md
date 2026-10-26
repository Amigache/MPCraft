# MPCraft — Rubric for the project's own skills

A skill is a rule that somebody can act on, written so that the next person does not have to remember it
from a previous session. Working from memory is how a skill and the code drift apart: the skill says one
thing, the tooling does another, and both look fine until someone follows the skill.

Each skill is read against these eight points. A skill that fails one of them is corrected in the same
change that notices it, rather than left as a note.

| # | Point | What it has to be | How it is checked |
| --- | --- | --- | --- |
| 1 | Trigger | The description names the **condition** that calls the skill, not the topic it is about: "use when writing or running integration tests against a live vehicle", not "testing". | A reader decides from the description alone whether the skill applies to a given task. |
| 2 | One rule | The body states a rule with its reason — `X must Y because Z` — rather than a preference or a list of habits. | The rule can be violated. A paragraph that nothing can disagree with is not a rule. |
| 3 | Runnable steps | Each step produces an answer, and the skill says what that answer means: what a green proves and what it does not. | Run the steps on a clean tree and read what they say. |
| 4 | Known-bad case | The skill shows the rule firing on something wrong, so its green is trusted before it is relied on — the same way a gate is shown failing on a known-bad case. | The case is written down and can be run again. |
| 5 | No divergence | The skill matches what the tooling actually does on this machine. When they disagree, one of them is changed in the same commit and the disagreement is written down rather than passed over. | Read the skill against the script it describes, not against the intention. |
| 6 | Boundaries | What cannot be done here is said: hardware, services, a device the browser run has no backend for, a port these binaries do not bind. | The skill names what it cannot verify rather than implying it can. |
| 7 | No duplicated rules | Rules that live in `docs/ARCHITECTURE.md` or `docs/PARITY.md` are pointed at, not copied. Two copies are how drift starts. | The skill contains pointers; the rule itself has one home. |
| 8 | Language | Skills are written in English, because that is the language the code is written in. Skill prose is not the interface, so it does not go through the catalogue; what the interface writes does. | The catalogue gate reads the interface, not this document. |

## What this caught once, which is why point 5 is on the list

`sitl-testing` described only the CI container on UDP 14550, while the runs on this machine used Mission
Planner's local binaries on the port they reserve per instance — a build that does not take a UDP device
string, does not bind the RC input port, and exits when its client disconnects. Both were true; the skill
covered one of them, so anyone following it would have tried Docker and lost the afternoon. The skill now says
the two, and which assertions each one can answer.

The same kind of drift is what the gates guard against: a green that rests on nothing is read as a pass, so
`scripts/run-gates.js` names what did not run (`NO-APLICA`, `HERRAMIENTA-AUSENTE`, `SUSTITUIDA`, `BLOQUEADO`)
and shows each state firing in `scripts/run-gates-controls.js`. A skill is held to the same standard by points
3 and 4.
