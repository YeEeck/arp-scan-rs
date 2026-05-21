# Network Interface Selector Hover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight animated hover background to selectable rows in the network interface popup list without changing selection behavior.

**Architecture:** Keep the change entirely in the Slint view layer by deriving each row background from four existing states: selected, hovered, enabled, and default. Preserve the stronger selected styling, keep disabled rows inert, and add a short background animation for smoother transitions.

**Tech Stack:** Rust, Slint 1.14.1, cargo test

---

### Task 1: Add Hover Styling To The Selector Rows

**Files:**
- Modify: `ui/main.slint`
- Verify: `cargo check`

- [ ] **Step 1: Inspect the existing selector row rendering**

Read the `NetworkInterfaceSelector` row loop in `ui/main.slint` and confirm the current row background logic only distinguishes selected and disabled rows.

- [ ] **Step 2: Write the minimal Slint styling change**

Update the popup row so the `TouchArea` hover state participates in the row background:

```slint
background: index == root.current-index
    ? #e7edf5
    : touch-area.has-hover && item.enabled && root.enabled
        ? #f2f6fb
        : (item.enabled ? #ffffff : #f3f5f8);

animate background { duration: 150ms; }
```

Keep the existing disabled opacity and click handling unchanged.

- [ ] **Step 3: Run build verification**

Run: `cargo check`
Expected: exit code `0`

- [ ] **Step 4: Commit**

```bash
git add ui/main.slint
git commit -m "fix: 为网卡下拉列表补充悬停动效"
```

### Task 2: Run Regression Verification

**Files:**
- Verify: repository test suite

- [ ] **Step 1: Run the existing automated regression suite**

Run: `cargo test -v`
Expected: existing tests pass with exit code `0`

- [ ] **Step 2: Record the visual verification boundary**

Confirm no new automated hover-specific assertion was introduced because the
current repository test setup does not provide a low-risk hover-visual
assertion path for this purely presentational change.

- [ ] **Step 3: Verify requirements against the design**

Check that the implementation meets all of the following:

```text
- only selectable rows react on hover
- hover is lighter than selected state
- disabled rows remain inert
- selection behavior is unchanged
```

- [ ] **Step 4: Commit if additional changes were needed**

```bash
git status --short
```

If verification required no further edits, do not create an extra commit.
