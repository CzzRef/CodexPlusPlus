# Knowledge Rules

Tool: tool-neutral (codex, claude, grok, and any CodeNote-routed agent)

## Routing

- Canonical product/design docs: `docs/`
- Error memories: `vibe/knowledge/error-memory/`
- ADRs: `vibe/knowledge/adr/`
- New process docs: `vibe/specs/`
- Reusable failure capture: [error-memory-capture](../../../../CzzProj/CodeNote/AiRef/VibePractice/Skills/global/error-memory-capture/SKILL.md)

## Initialization Notes

- No AI-DB workspace was created.
- `docs/` remains the product/design authority; `vibe/knowledge/` indexes reusable facts instead of copying bodies.

## Write Policy

- Search existing knowledge before adding new records.
- Store only reusable, verified, safe knowledge.
- Mark evidence as code, test, user-confirmed, official-doc, or inference.
- Never store credentials, install paths with secrets, or private receipts.
- Link old and new docs when business behavior changes.

## Document Governance Map

- Knowledge index: [../knowledge/README.md](../knowledge/README.md)
- Specs index: [../specs/README.md](../specs/README.md)
- DB workspace: not configured for this project
- Use `--all-markdown` only for deep historical document hygiene; default audit covers active AI rule surfaces.
