<!-- file: .github/copilot-instructions.md -->
<!-- version: 2.5.0 -->
<!-- guid: 4d5e6f7a-8b9c-0d1e-2f3a-4b5c6d7e8f9a -->

# Copilot/AI Agent Coding Instructions System

This repository uses a centralized, modular system for Copilot/AI agent coding, documentation, and
workflow instructions, following the latest VS Code Copilot customization best practices.

---

# 🚨 CRITICAL: COMMIT MESSAGE FORMAT 🚨

**EVERY SINGLE COMMIT MESSAGE MUST USE CONVENTIONAL COMMITS FORMAT. NO EXCEPTIONS.**

**ALL commit messages MUST follow Conventional Commits format:**

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Required format:**

- `type`: feat, fix, docs, style, refactor, test, chore, ci, perf, build
- `scope`: optional but recommended (e.g., `ci`, `docs`, `core`, `api`)
- `subject`: short imperative description (50 chars max)
- `body`: detailed explanation (optional but recommended for non-trivial changes)
- `footer`: breaking changes, issue references (optional)

**✅ CORRECT Examples:**

- `feat(api): add user authentication endpoint`
- `fix(core): resolve memory leak in image processor`
- `docs(readme): update installation instructions`
- `ci(lint): fix super-linter markdown configuration`
- `refactor(utils): simplify date parsing logic`
- `perf(db): optimize query performance for large datasets`
- `style: reformat code with Prettier`
- `test(auth): add unit tests for login functionality`

**❌ WRONG - These will be REJECTED:**

- `Add unit tests for DiskManager, IsoManager, PostProcessor, and QemuUtils` ❌ NO TYPE!
- `Update tests` ❌ TOO VAGUE, NO TYPE!
- `Fixed bugs in the code` ❌ NO TYPE, TOO VAGUE!
- `Improvements to the system` ❌ NO TYPE, NO DETAIL!

## Multiple Changes in One Commit

**When a commit contains MULTIPLE DIFFERENT types of changes, use MULTIPLE conventional commit lines
in the subject.**

This allows automated tools to parse and count each type of change separately for analytics,
changelogs, and release notes.

**Format for multiple changes:**

```
<type1>(<scope1>): <change1>; <type2>(<scope2>): <change2>; <type3>(<scope3>): <change3>

<detailed body with bullet points for each change>
```

**✅ CORRECT Example - Multiple Changes:**

```
test(disk): add DiskManager unit tests; test(iso): add IsoManager tests; test(qemu): add QemuUtils tests

Changes Made:

test(disk): add unit tests for DiskManager
- Add tests for disk creation and path retrieval
- Verify error handling for invalid paths

test(iso): add unit tests for IsoManager
- Add tests for URL generation validation
- Test different Ubuntu versions (22.04, 24.04)

test(qemu): add unit tests for QemuUtils
- Enhance tests for image info retrieval
- Add conversion error handling tests
- Handle expected errors in CI environments without tools
```

**Alternative - Use separate commits when changes are unrelated:**

- Commit 1: `test(disk): add DiskManager unit tests`
- Commit 2: `test(iso): add IsoManager unit tests`
- Commit 3: `test(qemu): add QemuUtils unit tests`

**See `.github/instructions/commit-messages.instructions.md` for complete guidelines.**

---

## Documentation Updates

Documentation files in `.github/instructions/` can be edited directly in this repository. Keep
changes concise and consistent with the language-specific guidance. If conflicts arise during
merges, resolve them directly in the affected files.

## Git operations policy

- Use VS Code tasks first for Git operations when available.
- Otherwise, use the Rust utility `copilot-agent-utilr` (or `copilot-agent-util`) for consistent
  logging and args-file support.
- Do not use raw `git` commands in automation unless explicitly required.

Examples:

- Commit using the utility:
  - `copilot-agent-utilr git commit -m "feat: add feature"`
  - Or via task: “Git Commit” (wired to the utility)
- Status/add/push:
  - `copilot-agent-utilr git status`
  - `copilot-agent-utilr git add -A`
  - `copilot-agent-utilr git push`

## System Overview

- **General rules**: `.github/instructions/general-coding.instructions.md` (applies to all files)
- **Language/task-specific rules**: `.github/instructions/*.instructions.md` (with `applyTo`
  frontmatter)
- **Prompt files**: `.github/prompts/` (for Copilot/AI prompt customization)
- **Agent-specific docs**: `.github/AGENTS.md`, `.github/CLAUDE.md`, etc. (pointers to this system)
- **VS Code integration**: `.vscode/copilot/` contains symlinks to canonical `.github/instructions/`
  files for VS Code Copilot features

## How It Works

- **General instructions** are always included for all files and languages.
- **Language/task-specific instructions** extend the general rules and use the `applyTo` field to
  target file globs (e.g., `**/*.go`).
- **All code style, documentation, and workflow rules are now found exclusively in
  `.github/instructions/*.instructions.md` files.**
- **Prompt files** are stored in `.github/prompts/` and can reference instructions as needed.
- **Agent docs** (e.g., AGENTS.md) point to `.github/` as the canonical source for all rules.
- **VS Code** uses symlinks in `.vscode/copilot/` to include these instructions for Copilot
  customization.

## For Contributors

- **Edit or add rules** in `.github/instructions/` only. Do not use or reference any
  `code-style-*.md` files; these are obsolete.
- **Add new prompts** in `.github/prompts/`.
- **Update agent docs** to reference this system.
- **Do not duplicate rules**; always reference the general instructions from specific ones.
- **See `.github/README.md`** for a human-friendly summary and contributor guide.

For full details, see the [general coding instructions](instructions/general-coding.instructions.md)
and language-specific files in `.github/instructions/`.
