---
description: "Zero-Heuristic Spatial Geometry Invariants: Strict Prohibition of Discrete If-Else Branching on 2D Space"
globs: "src/**/*.rs"
---

# Zero-Heuristic Spatial Geometry Standard

## Core Invariant
Continuous 2D spatial coordinates $(x, y, w, h)$ must NEVER be partitioned using discrete boolean `if-else` branching with arbitrary magic constants (e.g. `x <= mid + 15.0`, `left_count >= 1 && right_count >= 1`, `gap >= 7.0`).

## Enforced Design Principles
1. **Topological 1D Interval Merging for Lines**:
   - Two bounding intervals $[y_{\min}, y_{\max}]$ and $[y'_{\min}, y'_{\max}]$ belong to the same physical text line if their vertical projection intervals overlap:
     $$\max(y_{\min}, y'_{\min}) < \min(y_{\max}, y'_{\max})$$
   - Horizontal gaps on the same vertical line represent **Tab Stops** (`<w:tab/>`) or paragraph indentations, NOT separate columns.
   - Resumes, invoices, receipts, legal signatures, and exam headers with left-aligned descriptions and right-aligned dates/amounts MUST remain on the same line.

2. **Continuous Projection Gutter Analysis for Columns**:
   - A multi-column layout requires a continuous, unobstructed vertical whitespace gutter spanning across multiple paragraphs.
   - Column classification must never be evaluated as an arbitrary page-wide boolean flag.

3. **No Arbitrary Magic Numbers**:
   - All spatial thresholds must be derived dynamically from intrinsic document metrics (median font size, inter-character docstrum spacing, page printable width).
   - Never inject hardcoded coordinates like `15.0`, `35.0`, `75.0`, or `120`.

4. **Preserve Embedded Graphics & Masks**:
   - Image masks (`/ImageMask true`) and inverted decode tables (`/Decode [1 0]`) must be respected to prevent transparent signatures and stamps from rendering as solid black boxes.

