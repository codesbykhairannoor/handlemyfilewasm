# Rule: 1:1 PDF Page Boundary Preservation & Geometric Spacing

1. **82% Density Threshold for Page Breaks**:
   - Insert explicit `<w:br w:type="page"/>` ONLY when:
     a. `prev_max_y < (page_height * 0.82)`: Intentional author page termination (Thesis Cover, Lembar Pengesahan, Kata Pengantar, Daftar Isi, or chapter endings).
     b. Next page begins with a major section/chapter title (`H1` at $y_{\min} \le 120\text{ pt}$).
     c. Orientation change (Portrait $\leftrightarrow$ Landscape).
   - On dense pages ($\ge 82\%$ full with continuing body/table content), do NOT insert artificial hard page breaks. Word's natural text layout flows them without creating single-line orphaned pages (which previously exploded 8-page documents into 15 pages).

2. **Native Right Tab Stop for Running Headers & Footers**:
   - Never use OOXML tables (`<w:tbl>`) in `<w:hdr>` or `<w:ftr>`. Tables in headers/footers have cell margins, borders, and row height constraints that inflate vertical footprint to $>40\text{ pt}$, pushing the footer up into body tables.
   - Use a single-line paragraph with a native Word right tab stop (`<w:tabs><w:tab w:val="right" w:leader="none" w:pos="{printable_width}"/></w:tabs>`):
     Left text (date/URL), tab character (`<w:tab/>`), right text / dynamic page number (`PAGE / NUMPAGES`).
   - Takes up exactly 1 line height ($\sim 11\text{ pt}$) with zero vertical overhead.

3. **Vertical Margin Clearance & Dynamic Clamping**:
   - Always enforce `w:header="360" w:footer="360"` ($18\text{ pt}$).
   - Body margins `w:top` and `w:bottom` must be dynamically derived from the document and clamped to safe typographic bounds: `clamp(28.0, 72.0)` points. Never force a minimum of $54\text{ pt}$ which artificially clips tight IEEE/academic margins ($\sim 36-44\text{ pt}$) and pushes content across page boundaries.

4. **Zero Synthetic Spacing Inflation**:
   - Never inject arbitrary hardcoded paragraph spacing constants (e.g. `+12pt`, `+6pt`, `+80 twips`).
   - Derive `space_before` strictly from the visual vertical delta ($\Delta y = y_{\min}(i) - y_{\max}(i-1) - 2.0$) between bounding boxes in the PDF.
   - Keep `space_after = 0.0` and let `space_before` faithfully represent inter-block distance.

5. **Zero Natural Language Keywords (Strict Language-Agnostic Engine)**:
   - NEVER hardcode human language keywords or strings (e.g. "DAFTAR ISI", "TABLE OF CONTENTS", "BAB", "CHAPTER", "KATA PENGANTAR", "HALAMAN", "PAGE", "LEMBAR PENGESAHAN", etc.) in any part of the engine (DLA, classifier, rxyc, table detector, or DOCX generator).
   - The engine must operate universally across all languages and scripts (Latin, Cyrillic, CJK, Arabic, Devanagari, etc.).
   - All structural decisions must be derived strictly from:
     a. **Typography**: Relative font size ratios ($font \ge 1.35 \times median\_body$), font weights (`is_bold`), Unicode casing properties (`is_all_caps` via `is_alphabetic` and `is_uppercase`).
     b. **Spatial Geometry**: Coordinates ($y_{\min} \le 160\text{ pt}$ for page-top titles), horizontal alignment (`alignment == "center"`), block length ($\le 2$ lines, $\le 80$ characters), and bounding box margins.
     c. **Structural Leader Patterns**: Universal dot leader patterns (`..` or trailing page number with tab gap $\ge 20\text{ pt}$ or $x \ge 350\text{ pt}$) to detect TOC rows and generate native Word dot-leader tabs, completely independent of natural language strings.

