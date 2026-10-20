# Plan: ACP Red Star 4.0, a wider research scope

Status: **design only.** Nothing in this file is implemented. This is the living 4.0 plan, the way [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) was the living 3.0 plan. The short pointer in that file is §4.0. Schema notes for post source and the Haz stream corpus are §7.14.14 there. The embedded assistant is §7.14.12. Decentralization stays in 3.x.

Update 2026-10-19: opened from `main`. Post source and the Haz stream corpus come from draft PR #14. The embedded local assistant is §7.14.12 of [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) (merged from #10). This file points at that write-up and does not copy it. Items marked **PROPOSED** are suggestions for Michael to accept or cut. They are not commitments.

---

## Theme

Michael, 2026-10-19: "I want to make version 4 about spreading out the scope of research: expanding our sources and leveraging local models to help us wade through the data."

4.0 widens research. The 3.x client keeps a local encrypted copy of Red Star. 4.0 lets a research project draw on more than that forum, and it puts local models to work so a person can get through a large pile of posts and transcripts. The models run on the comrade's machine. They help read. They do not replace the archive, and they do not replace the person checking a transcript.

He is open to suggestions. Suggestions in this file are marked **PROPOSED** until he accepts or cuts them.

## Goals

- A research project can hold material from more than the Red Star forum, and every item still says where it came from.
- If a model is trained or adapted, the Haz streams are the corpus: every stream, transcripts that have been checked, and speaker turns that name Haz and the other person.
- A local assistant inside the client can be asked about the archive the comrade already has. The design of that assistant is §7.14.12 of the 3.0 plan. This plan does not restate it.
- Keyword search stays. Local models are an added way through the same data.
- The database stays the encrypted one at `~/.acp/ai/data/rs.database`. New columns are added in place.

## Non-goals

- Decentralization, relays, peers, and the network node. Those stay in 3.x, as already decided (3.23.0 and later in the 3.0 plan). 4.0 does not move them.
- A cloud model as the way the app thinks. A **PROPOSED** rule below is that the default is local and offline.
- Training a foundation model from scratch. §7.14.12 of the 3.0 plan already says a voice adaption, if it happens, is a small fine-tune, and the model is not chosen.
- Choosing the model in this document. An example in §7.14.12 is not a selection.
- Numbered 3.x stages. They stay where the 3.0 plan put them.
- Shipping code from this note. `CAPTURE_BLOCK` still stops a live publish.
- A membership pitch. Everyone reading this already uses Red Star.

## Workstreams

### Accepted

These three are in. The first is the prerequisite for the second.

1. **Post source / provenance.** Each post stores where it came from, next to the post. Examples: `redstar`, a YouTube transcript, and later other names. The row keeps a source identifier, a URL, and, for a transcript segment, a start and end offset. Existing rows default to Red Star. The sketch, the migration, the SQLCipher note, and the export note are in the 3.0 plan, §7.14.14. `post_versions.source` remains the edit-history column (`site`, `peer`, `local`, `draft`). It is a different field.
   The columns are additive. They can land in a 3.x minor before 4.0, the same way `body_json` landed early. That minor is not numbered here. Ingest and review stay in 4.0.

2. **Haz stream corpus.** If we make a model, we use the Haz streams: all the data from all the streams. Transcripts have to be correct. Speakers have to be defined, so a debate distinguishes Haz from the debater. Requirements:
   - Ingest every Haz stream, not a sample.
   - A review and correction workflow. An automatic transcript is the starting text. A person checks it.
   - Provenance for each correction: who changed it, when, and whether the previous text was a machine transcript or an earlier review.
   - The correction history is kept. The latest text is on the post. Older text stays in an append-only history (the `transcript_corrections` sketch in §7.14.14 of the 3.0 plan).
   - Diarize and label speakers. Store speaker turns: Haz, and each debater or guest, with the words and the time range.
   - Each turn uses post source, so it points at the stream URL and the offsets. This workstream waits on workstream 1.

3. **Embedded local assistant.** A local model inside the client, for questions about the local corpus and for actions the app already has, such as search, filter, and export. The write-up is §7.14.12 of [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md): the bottom-center chat strip, what the panel shows, and the note on voice. This plan does not copy that text. The model is not chosen. The assistant is not a 3.x requirement.

### PROPOSED

Michael can accept or cut each of these. None of them is a promise. The pre-launch post treats them as possibilities.

1. **PROPOSED. More sources, one interface.** YouTube transcripts first, then other video and podcast platforms, RSS, public posts on X, PDFs and documents, and ACP site pages. Each source uses the same post-source fields: name, identifier, URL, and offsets or page locations where those exist. A new source is a new value of `source`, not a new database.

2. **PROPOSED. Local transcription and diarization.** A Whisper-class model on the machine, for a stream that has no usable captions. Local diarization proposes speaker turns. A person still reviews the text and the labels before the turn is treated as corrected. The machine pass is the first row of the correction history, with actor `machine`.

3. **PROPOSED. Local embeddings and semantic search.** Vectors for every stored source, computed on the machine, beside the keyword search that already exists. Keyword search stays the default path. Semantic search is a second way to ask.

4. **PROPOSED. Summaries and questions over a research project.** A local model may summarize a project or answer a question about it. Every answer cites the exact posts, and the transcript timestamps, that it used. An answer with no citation is not shown. The model does not get to invent a source.

5. **PROPOSED. Local and offline by default.** No cloud model call unless a person turns that on later, and that switch is not part of this proposal. Model files are downloaded and stored by the app, in a documented directory under `~/.acp`, and the docs name the disk and memory a given model needs. A machine that cannot run a model still browses, searches, and exports.

6. **PROPOSED. Research projects that mix sources.** A project can hold Red Star posts and transcript turns together. An export of that project carries the provenance fields. That is a new export format version. Format version 2 stays valid: an older file imports as Red Star. This note does not bump the version constant.

7. **PROPOSED. A cross-source timeline.** One view of what was said, where, and when: a Red Star post and a transcript turn on the same line of time, each still labeled with its source.

8. **PROPOSED. Rights, permission, and credit.** Third-party footage and transcripts need a note of what we may store, who is credited, and what must not be republished. The note is stored with the source. It is not a substitute for permission the project does not have.

## Prerequisites

- **Post source before the Haz corpus.** A turn without a source id, a URL, and offsets cannot be tied to the stream.
- **Post source before any PROPOSED extra source.** Same columns, new `source` values.
- **The encrypted database and the current research projects.** 4.0 adds columns and rows. It does not invent a second database.
- **Corrected text before a model treats a transcript as quotable.** The review workflow is part of the corpus, not a later cleanup.
- **The assistant design in §7.14.12 of the 3.0 plan**, if that work is what 4.0 builds. If that section changes, this plan updates the pointer. It does not fork the design.
- **3.x keeps shipping.** 4.0 does not block attachments, interactions, or the network node.

## Open questions

- Which 3.x minor, if any, should carry the source columns? The plan allows an early landing and does not assign the number.
- Which channels count as "every Haz stream," and who holds the list?
- Who reviews a transcript, and does a correction record a nick, a role, or both?
- Does diarization need a human to confirm every speaker label before the turn is stored, or may a machine label sit in the history until someone confirms it?
- Which local model, if any, and what disk and memory are the minimum we will document?
- Which **PROPOSED** items are in 4.0, which wait, and which are cut?
- What may we store from a third-party video: our own transcript, the platform's captions, the audio, or only a link?
- Should a mixed-source export be format version 3, and what does an older client do when it sees one?

## Risks

- A wrong transcript or a swapped speaker becomes part of a model or a citation. The correction history has to be real, and the latest text has to be the reviewed one.
- Scope. Extra sources are easy to list and slow to ingest. The accepted core is provenance, the Haz corpus, and the assistant already designed in §7.14.12 of the 3.0 plan. The **PROPOSED** list is allowed to shrink.
- Two copies of the assistant design. This file points at §7.14.12 of the 3.0 plan. If both texts grow their own chrome and voice notes, they will drift. That section stays the design; this file stays the pointer.
- Hardware. A local model that only runs on a large GPU leaves other comrades out. Browsing and keyword search have to work with no model installed.
- Rights. Storing someone else's footage or captions without a credit and permission note will have to be undone later.
- Export compatibility. A new format version that old importers reject will strand a shared research project. Version 2 files have to keep importing.
