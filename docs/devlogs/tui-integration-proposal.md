# Proposal: Progressive TUI Integration

This document proposes revising the development plan to integrate TUI development progressively alongside CLI features, rather than deferring it entirely to Phase 9.

## Motivation

The original plan defers all TUI work to Phase 9, after the CLI is feature-complete. This approach has drawbacks:

1. **Big bang integration risk** - Building the entire TUI at once means discovering UX issues late
2. **API drift** - Core APIs may not be TUI-friendly if the TUI isn't exercising them during development
3. **Delayed feedback** - Users (and the developer) can't validate the interactive experience until very late
4. **Motivation gap** - A long stretch of CLI-only work before any visual payoff

## Proposal: Phased TUI Integration

Start TUI integration after the core engine stabilizes (post-Phase 4), and grow it incrementally with each subsequent phase.

### Revised Phase Structure

#### Phases 0-4: CLI-Only (unchanged)

These phases establish the foundational systems. TUI work would be premature here because:
- Config, discovery, and template engine APIs are still forming
- Markdown AST insertion (Phase 4) is a core capability that should stabilize first

| Phase | Focus |
|-------|-------|
| 0 | Workspace, CI, doctor stub |
| 1 | Configuration system |
| 2 | Template discovery |
| 3 | Template engine MVP |
| 4 | Markdown AST insertions |

#### Phase 5: File Planner + TUI Foundation

**Original**: File planner, atomic writes, undo log
**Revised**: Add TUI foundation alongside file operations

TUI additions:
- Basic Ratatui app scaffold
- Template list view (palette)
- Read-only preview pane
- No execution yet - just browsing

Rationale: The Coordinator facade is introduced here, providing a clean API for the TUI to consume.

```
TUI Scope (Phase 5):
- [ ] App shell with event loop
- [ ] Template palette (fuzzy list)
- [ ] Preview pane (rendered template, read-only)
- [ ] Keybindings: navigate, quit
```

#### Phase 6: CLI Wiring + TUI Execution

**Original**: Expose CLI commands (template, capture, macro, list, preview, doctor, undo)
**Revised**: TUI gains execution capability alongside CLI

TUI additions:
- Execute `new` from TUI (create file)
- Variable prompts (interactive input)
- Success/error feedback

```
TUI Scope (Phase 6):
- [ ] Execute template from palette
- [ ] Variable input prompts
- [ ] Status bar with feedback
- [ ] Confirmation before write
```

#### Phase 7: Macro Runner + TUI Multi-Step

**Original**: Macro runner, security gates
**Revised**: TUI supports macro execution and trust prompts

TUI additions:
- Macro selection and preview
- Step-by-step execution view
- Trust confirmation dialogs

```
TUI Scope (Phase 7):
- [ ] Macro palette
- [ ] Step progress indicator
- [ ] Trust/security prompts
- [ ] Error recovery options
```

#### Phase 8: Lua Hooks + TUI Scripting View (Optional)

**Original**: Lua scripting
**Revised**: TUI can display Lua macro previews

TUI additions:
- Lua script preview
- Sandbox status indicator

#### Phase 9: TUI Polish (Reduced Scope)

**Original**: Full TUI implementation
**Revised**: Polish and refinement only

Remaining work:
- Theming support
- Accessibility improvements
- Performance optimization
- Edge case handling

```
TUI Scope (Phase 9):
- [ ] Theme configuration
- [ ] Responsive layouts
- [ ] Help overlay
- [ ] Final UX polish
```

#### Phase 10: Documentation & Release (unchanged)

### Visual Timeline

```
Phase:    0   1   2   3   4   5   6   7   8   9   10
          |---|---|---|---|---|---|---|---|---|---|
CLI:      ============================>
TUI:                          ================>
                              ^
                              TUI starts here
                              (after core stabilizes)
```

### Benefits of This Approach

1. **Incremental validation** - TUI UX is tested as features land
2. **Shared abstractions** - Core APIs naturally become TUI-friendly
3. **Smaller phases** - Each phase has a bounded TUI scope
4. **Earlier user testing** - Interactive prototype available sooner
5. **Reduced Phase 9 risk** - No longer a monolithic TUI phase

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| TUI slows core development | Keep TUI scope minimal per phase; defer polish to Phase 9 |
| API churn breaks TUI | TUI only consumes Coordinator facade, not internals |
| Ratatui learning curve | Start with simple list+preview; complexity grows gradually |

### Implementation Notes

1. **Coordinator as boundary**: The TUI should only depend on `Coordinator` (introduced Phase 5-6), not on internal modules like `TemplateRepository` or `MarkdownEdit`.

2. **Feature flags**: Consider gating TUI builds behind a cargo feature if compile times become an issue:
   ```toml
   [features]
   default = ["cli"]
   tui = ["ratatui", "crossterm"]
   ```

3. **Shared types**: Output types (success/error reports) should be defined in `core` so both CLI and TUI can consume them identically.

4. **Testing strategy**: TUI snapshot tests (terminal output) can use `insta` with terminal rendering, but prioritize core logic tests.

## Decision

This proposal is open for consideration. The key decision points are:

1. **When to start TUI?** Proposal: Phase 5 (after AST insertion stabilizes)
2. **How much TUI per phase?** Proposal: Bounded scope, roughly 20-30% of phase effort
3. **Separate binary or integrated?** Current: separate `markadd-tui` binary (keep this)

## Next Steps

If accepted:
1. Update `docs/01_development_plan.md` with revised phase descriptions
2. Add TUI deliverables to each phase's checklist
3. Begin TUI scaffold in Phase 5

---

*Proposed: 2025-12-13*
*Status: Draft*
