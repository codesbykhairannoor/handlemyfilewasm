# Rule: 1:1 PDF Page Boundary Preservation & Geometric Spacing

1. **1:1 Page Boundary Mapping**:
   - Every physical page in the source PDF represents an intentional page designed by the author.
   - Insert an explicit `<w:br w:type="page"/>` between consecutive PDF pages `P[i]` and `P[i+1]`, unless `P[i]` is a dense page ending mid-sentence that seamlessly continues on `P[i+1]` (lowercase start, no terminal punctuation, no header).

2. **Zero Synthetic Spacing Inflation**:
   - Never inject arbitrary hardcoded paragraph spacing constants (e.g. `+12pt`, `+6pt`, `+80 twips`).
   - Derive `space_before` strictly from the visual vertical delta ($\Delta y = y_{\min}(i) - y_{\max}(i-1)$) between bounding boxes in the PDF.
   - Keep `space_after = 0.0` and let `space_before` faithfully represent inter-block distance.

3. **Margin Fidelity**:
   - Derive section margins (`w:pgMar`) directly from the median bounding box boundaries of the PDF pages, preventing Word from inflating printable areas and causing premature line wraps or page overflows.

