# Result List Hover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight full-row hover background to the scan result list while preserving zebra striping, layout, and scrolling behavior.

**Architecture:** Keep the change entirely inside `ui/main.slint` by deriving each rendered row background from hover state plus the existing zebra fallback. Use a `TouchArea` per visible row, animate only the background transition, and leave row contents, separators, and Rust-side bindings unchanged.

**Tech Stack:** Rust, Slint 1.14.1, cargo check, cargo test

---

### Task 1: Add Hover Styling To Result Rows

**Files:**
- Modify: `ui/main.slint`
- Verify: `cargo check`

- [ ] **Step 1: Confirm the current row backgrounds are zebra-only**

Read the `ResultListPanel` row loop in `ui/main.slint` and confirm both the
outer row rectangle and its inner content rectangle currently derive their
background only from `Math.mod(index, 2)`.

- [ ] **Step 2: Write the minimal Slint styling change**

Update the rendered row to introduce a row touch area and derive the background
from hover state before falling back to zebra colors:

```slint
background: row-touch-area.has-hover
    ? #f2f6fb
    : (Math.mod(index, 2) == 0 ? #fcfdff : #f7f9fc);
animate background { duration: 150ms; }
```

Apply the same background expression to both the outer row rectangle and the
inner content rectangle so the full visible row reacts consistently.

Add the touch area as an overlay without click behavior:

```slint
row-touch-area := TouchArea { }
```

- [ ] **Step 3: Run build verification**

Run: `cargo check`
Expected: exit code `0`

- [ ] **Step 4: Commit**

```bash
git add ui/main.slint
git commit -m "fix: 为结果列表增加轻量悬停反馈"
```

### Task 2: Run Regression Verification

**Files:**
- Verify: repository test suite

- [ ] **Step 1: Run the existing automated regression suite**

Run: `cargo test -v`
Expected: existing tests pass with exit code `0`

- [ ] **Step 2: Record the hover-testing boundary**

Confirm no new hover-specific automated assertion was added because the current
repository test harness does not provide a low-risk way to verify this purely
visual hover transition without exposing extra implementation detail.

- [ ] **Step 3: Verify requirements against the design**

Check that the implementation meets all of the following:

```text
- full-row hover only
- hover remains lighter than the header band
- zebra striping remains the idle fallback
- no changes to scrolling, clipping, or column layout
```

- [ ] **Step 4: Commit if additional changes were needed**

Run:

```bash
git status --short
```

If verification required no further edits, do not create an extra commit.
