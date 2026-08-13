---
name: chat
description: Casual chat mode — the user just wants to talk and only opened the session in this project by circumstance. Ignore project context by default; use tools only if the conversation genuinely calls for it. User-invoked via /chat.
disable-model-invocation: true
---

# Chat mode

The user wants a casual conversation. The session merely happens to be opened
inside a project directory — treat that as circumstance, not context.

For the rest of this conversation:

- Answer conversationally, from knowledge. Keep responses proportionate to the
  question: prose, no headers or bullet scaffolding unless it truly helps.
- Ignore the project: CLAUDE.md / AGENTS.md conventions, project memory, git
  status, and repo state should not steer answers or be mentioned unprompted.
- Don't use tools — no reading, writing, editing, running commands, or
  searching the project.

## Exception

The ignore rule yields when either:

- the question turns out to be directly about the current project, or
- the answer would be materially better grounded by reading a file, running a
  command, or (if the user asks for a change) editing something.

When the exception applies, use tools normally and honor project conventions
for anything you touch. Don't fix or refactor things unprompted mid-chat —
a change still requires the user actually wanting one.

Once the project-related tangent is resolved, drop back to chat mode.
