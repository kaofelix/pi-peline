# Agent Instructions

Instructions for AI agents (like Pi) working on this project.

---

## General Behavior

### Tracking Improvements

**Rule:** When you're asked to note something to fix later, or when you identify technical debt/bugs during your work, **always add it to `IMPROVEMENTS.md`**.

**Format:**
```markdown
### [Brief Title]

**Issue:** Clear description of the problem

**Current Behavior:** How it works now (include examples if applicable)

**Proposed Fix:** What should be done

**Files to Modify:** List of files that need changes (if applicable)

**Priority:** Low/Medium/High (with rationale)
```

**Why:** This keeps track of technical debt without derailing from the main task.

---

## Project Context

This is `pi-peline`, a tool for orchestrating multi-step AI agent workflows. Key directories:

- `src/` - Core Rust code
- `examples/` - Example pipeline definitions
- `pipelines/` - Working pipelines (dogfooding, experiments)
- `docs/` - Project documentation
- `IMPROVEMENTS.md` - Technical debt and future improvements
- `OBSERVABILITY_PLAN.md` - Plan for adding real-time observability

---

## Code Conventions

- Use descriptive variable and function names
- Add documentation comments for public APIs (`///`)
- Keep functions focused and small
- Run `cargo test` before committing changes
- Use `clippy` (`cargo clippy`) for lint checks

---

## Testing

Before making changes:
1. Read relevant test files in `tests/` directory
2. Run existing tests with `cargo test`
3. Add tests for new functionality
4. Verify all tests pass
