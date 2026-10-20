# Coming soon: ACP Red Star 4.0, a wider research library

Comrades,

**ACP Red Star** is headed toward version 4. This is a preview of where that version is going. It has not shipped. Nothing in this post is in your hands yet.

Version 3 is the client: a local, encrypted archive of our forum, and the work of reading, writing, and syncing that archive. Version 4 is about research. Michael put it this way: he wants version 4 to spread out the scope of research, expanding our sources and using local models to help us wade through the data.

## What 4.0 is for

Red Star will still be the home archive. 4.0 is how a research project grows past a single forum thread without losing track of where each sentence came from, and how a model on your own machine helps you read a pile that is too big to hold in your head.

Decentralized sync between comrades' nodes stays a 3.x job. 4.0 does not take that work over, and it does not delay it.

## What we have actually decided

Three things are in the plan. They are planned, not built.

- **Every post will say where it came from.** A Red Star post, a transcript segment, and anything we add later will carry a source name, an identifier, and a URL. A transcript segment will also carry its start and end time. Rows you already have will count as Red Star. Those columns may land in a 3.x update, before the rest of 4.0, so the database is ready. The Haz work and the assistant wait for 4.0.
- **The Haz stream corpus.** If we make a model, we use the Haz streams. All of them. All of the data from all of the streams. @hazaldin, that means your streams are the corpus, not a highlight reel. Transcripts will be checked and corrected, and every correction will be kept, so we can see what the machine said and what a person fixed. Speakers will be labeled. In a debate, Haz is Haz and the debater is the debater. Each turn will point back at the stream and the timestamp.
- **A local assistant inside the app.** A model on your machine, for questions about the archive you already have, and for actions the app already knows, such as search, filter, and export. The design of that assistant is written up on its own. The model is not chosen. This post does not pretend we have picked one.

## What we are only considering

Michael is open to suggestions. The following are **possibilities**. He may accept them, cut them, or leave them for later. They are not promises.

- More sources behind that same source record: YouTube transcripts first, then other video and podcast platforms, RSS, public posts, PDFs and documents, and ACP site pages. Each one would keep its provenance.
- Local transcription for a stream that has no good captions, and local diarization to propose who is speaking. A person would still check the words and the names.
- Local semantic search beside the keyword search you already have.
- Summaries and questions over one research project, where every answer cites the exact posts and the transcript timestamps it used. No citation, no claim.
- Models that run locally and offline unless you decide otherwise, with the model files managed in the app and the hardware needs written down.
- Research projects that mix Red Star posts and other sources, and exports that carry that provenance in a new export format. Older exports would still import.
- A timeline of what was said, where, and when.
- A rights, permission, and credit note on third-party footage and transcripts.

## Why it is worth doing

Years of education and debate sit on Red Star. Years of streams sit beside it, harder to search and easier to misquote. A wider library is only useful if we can still say who said a line, where, and when, and if the tool that helps us read it runs on our own machines.

ACP Red Star 4.0 is being planned for the people who already use Red Star. The plan is [architecture/PLAN-4.0.md](../../../architecture/PLAN-4.0.md). The repo is still https://github.com/m3talsmith/acp-rs-scraper.

When 4.0 has actually shipped, the launch post will replace this one. Until then, this is the coming-soon draft.

For the working class,
~ @m3talsmith
