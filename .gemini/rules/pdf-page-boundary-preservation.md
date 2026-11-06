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

3. **Vertical Margin Clearance**:
   - Always enforce `w:header="360" w:footer="360"` ($18\text{ pt}$) and body margins `w:top` and `w:bottom` $\ge 54\text{ pt}$ ($1080\text{ twips}$).
   - This guarantees a minimum $25\text{ pt}$ of clean white space clearance between running headers/footers and document text/tables, eliminating overlaps completely.

4. **Zero Synthetic Spacing Inflation**:
   - Never inject arbitrary hardcoded paragraph spacing constants (e.g. `+12pt`, `+6pt`, `+80 twips`).
   - Derive `space_before` strictly from the visual vertical delta ($\Delta y = y_{\min}(i) - y_{\max}(i-1) - 2.0$) between bounding boxes in the PDF.
   - Keep `space_after = 0.0` and let `space_before` faithfully represent inter-block distance.

