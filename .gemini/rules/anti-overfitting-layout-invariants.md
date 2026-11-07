# Rule: Anti-Overfitting & Continuous Geometric Layout Invariants

## 1. Absolute Prohibition on Single-Document Hyper-Analysis
- NEVER inspect or micro-tune code against the coordinate idiosyncrasies of a single document in isolation.
- Do NOT write temporary scripts to inspect single-document bounding boxes and then hardcode thresholds tailored to that document.
- Single-document tailoring is an anti-pattern that guarantees regressions on other document types. All algorithms must be derived from universal mathematical and typographic principles.

## 2. Prohibition of Discrete If-Else Thresholds on Continuous 2D Space
- Document layout is a continuous 2D spatial domain ($x, y, w, h$, font metrics, density). 
- NEVER use discrete integer count thresholds (e.g., `left_count >= 1 && right_count >= 1`) to make global layout decisions (such as Single-Column vs Multi-Column).
- Structural layout classification must be governed by continuous mathematical models:
  1. **Text Mass Symmetry Ratio**:
     $$\mathcal{S}_{\text{mass}} = \frac{\min(M_L, M_R)}{\max(M_L, M_R)} \ge 0.40$$
     Where $M_L$ and $M_R$ are the aggregated text masses (character counts or total glyph area) of non-wide blocks on each side of the page center. Isolated marginal metadata (e.g. right-aligned resume dates, line numbers, page numbers) represent $< 15\%$ of total text mass and must NEVER trigger multi-column mode.
  2. **Baseline Pairing for Tab Stops**:
     When two blocks share the same vertical baseline ($|\Delta y| \le 2.0\text{ pt}$), they belong to the same logical line and must be modeled as Tab Stops (`<w:tab/>`), not parallel column partitions.
  3. **Scale-Invariant Font Proportions**:
     All whitespace decision boundaries (Docstrum word clustering, gutter thresholds, line leading) must be strictly proportional to local font em-size ($0.18 - 0.32\text{ em}$), never fixed point constants (`16.0 pt`, `54.0 pt`).

## 3. Mandatory Multi-Class Corpus Verification
- Any modification to layout analysis, clustering, or DOCX generation must be validated across a diverse multi-class corpus:
  1. Single-page table-dense documents (e.g. exam papers, forms).
  2. Single-column ATS resumes/CVs with right-aligned dates.
  3. Multi-column academic papers (IEEE/ACM) with spanning titles and figures.
  4. Standard narrative documents / reports.
- If an algorithm works for Document A but breaks Document B, the mathematical abstraction is incomplete and must be redesigned, never patched with another `if-else`.

