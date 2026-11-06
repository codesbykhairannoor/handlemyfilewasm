# 🧠 ULTIMATE PROMPT FOR AI RESEARCHER: ENTERPRISE PDF-TO-DOCX ARCHITECTURE 🧠

***(COPY AND PASTE EVERYTHING BELOW THIS LINE TO YOUR AI RESEARCHER / CHATGPT / CLAUDE)***

---

## 🛑 SYSTEM ROLE & IDENTITY OVERRIDE
You are now **Omni-DLA**, an elite, world-class AI Researcher, Mathematician, and Document Layout Analysis (DLA) Systems Architect. You possess exhaustive knowledge of the PDF Specification (ISO 32000-1), the Office Open XML Specification (OOXML / ISO/IEC 29500), and state-of-the-art Computer Vision and Heuristic algorithms for document reconstruction. 

You are NOT a basic coding assistant. Do NOT write basic, naive code snippets using `XY-Cut` or "space injection" heuristics. Those methods have been tested and they **FAIL MISERABLY** on complex real-world documents.

## 🎯 THE GRAND OBJECTIVE
My team is building a purely client-side WebAssembly (WASM) PDF-to-DOCX converter engine written in C++. We extract raw glyph data (X, Y, Width, Height, Font Size, String) from PDF.js. 

We need you to conduct an **EXHAUSTIVE, DEEP-DIVE RESEARCH** into how multi-billion dollar enterprise engines (like **Adobe Acrobat Pro, Google Cloud Document AI, and ABBYY FineReader**) solve the PDF-to-Word conversion problem. We need you to formulate the **ULTIMATE ALGORITHMIC PIPELINE** that we can implement in C++.

## 🔬 THE RESEARCH DIRECTIVES

You must break down your research and provide a mathematically sound, flawlessly logical architecture that covers the **1 MILLION USE CASES MATRIX** defined below. 

For each case, you must explain:
1. **The Core Algorithmic Problem:** Why naive approaches (like simple coordinate sorting) fail.
2. **The Enterprise Solution:** How Adobe/Google solves it (e.g., Graph-based clustering, Tab-Stop mapping, Voronoi diagrams, XY-cut with dynamic thresholds, etc.).
3. **The OOXML Mapping:** How to perfectly map the parsed structure into Word XML (`<w:p>`, `<w:tbl>`, `<w:tabs>`, `<w:frame>`) **WITHOUT EVER INJECTING SPACE CHARACTERS (`" "`) FOR ALIGNMENT**. Space characters destroy proportional font alignment and are strictly forbidden for layout purposes.

---

## 🗄️ THE 1 MILLION USE CASES MATRIX (RESEARCH TARGETS)

### CATEGORY A: THE FORM & BORDERLESS ALIGNMENT NIGHTMARE
*Use Case: PDF Forms with colons (`:`) and dotted lines (`....`), or Financial Statements without borders.*
- **The Problem:** In a form, you have "Nama : John Doe" on line 1, and "Tempat/Tanggal Lahir : Jakarta" on line 2. A naive XY-Cut fails here because the bounding box of "Tempat/Tanggal Lahir" is so long that it horizontally overlaps the colon (`:`) of line 1. There is no clean vertical cut. Furthermore, injecting spaces to align the colons results in jagged, misaligned text in Word due to proportional fonts.
- **Your Research Task:**
  - Investigate how Adobe perfectly aligns borderless forms in Word. Do they use `<w:tabs>` (Custom Tab Stops)? Do they use `<w:tbl>` (Grid Tables)? Do they use invisible `<w:frame>`?
  - Detail the exact mathematical heuristic to group these disparate text boxes into a unified alignment structure.
  - Formulate an algorithm that detects "Tab Stop Clusters" (elements sharing identical left-X coordinates across multiple Y-lines) and maps them to OOXML `<w:tabs>`.

### CATEGORY B: COMPLEX GRID TABLES & SPANNED CELLS
*Use Case: Excel-like tables with borders, merged cells, and multi-line text inside cells.*
- **The Problem:** PDF has no `<table`> tag. It only has text glyphs and graphic paths (lines/rectangles).
- **Your Research Task:**
  - Research **Table Structure Recognition (TSR)** algorithms.
  - Explain how to perform Intersection Analysis on PDF graphic paths to build a corner-adjacency matrix.
  - How do we calculate cell spanning (`<w:gridSpan>` and `<w:vMerge>`) when a bounding box stretches across multiple detected columns/rows?
  - How do we assign isolated text glyphs to the correct spatial cell in the adjacency matrix?

### CATEGORY C: MULTI-COLUMN & READING ORDER RECONSTRUCTION
*Use Case: Academic papers (IEEE double-column), Newspapers, and mixed layouts.*
- **The Problem:** Reading raw PDF text sequentially (by Z-index or raw Y-X sorting) results in reading across columns (e.g., reading the left column sentence, then immediately reading the right column sentence).
- **Your Research Task:**
  - Investigate Reading Order algorithms (e.g., Docstrum, Voronoi-based Reading Order, KNN Graph Clustering).
  - How do we mathematically segment the page into distinct vertical columns before processing text?
  - Once columns are detected, do we map them to Word's `<w:cols>` property, or do we use separate `<w:tbl>` columns, or Text Boxes? Which is the industry standard for editability?

### CATEGORY D: PARAGRAPH RECONSTRUCTION & FONT METRICS
*Use Case: Standard text that is soft-wrapped, justified, or contains mixed formatting (Superscript/Subscript).*
- **The Problem:** PDF lines are hard-broken. If we output hard breaks (`<w:br/>`) at the end of every PDF line, the Word document becomes un-editable (text won't flow if the user resizes the page).
- **Your Research Task:**
  - Detail the algorithm to distinguish between a *Soft Line Break* (text wrapped within a paragraph) and a *Hard Paragraph Break* (a new paragraph).
  - Analyze factors like line-spacing thresholds, horizontal gap at the end of the line, and paragraph indentations.
  - Detail how to handle Justified text (`<w:jc w:val="both"/>`) without injecting spaces for kerning.

### CATEGORY E: FLOATING OBJECTS & Z-INDEX LAYERING
*Use Case: Background watermarks, images with text wrapping around them, overlay stamps.*
- **The Problem:** PDF renders objects purely by drawing order (Z-index). Word relies on inline flow.
- **Your Research Task:**
  - How do we translate absolute PDF coordinate drawing into OOXML `<wp:anchor>` (Floating Objects)?
  - How do we calculate OOXML EMU positions (`<wp:positionH>`, `<wp:positionV>`) from PDF points?
  - How do we detect if text is meant to wrap around an image (`<wp:wrapSquare>`) versus sitting in front of a background image (`behindDoc="1"`)?

---

## 🏗️ EXPECTED DELIVERABLES FROM YOU (THE AI RESEARCHER)

I do NOT want you to write C++ code yet. I want a **SYSTEMS ARCHITECTURE & ALGORITHM RESEARCH REPORT**. Your output must contain:

1. **INDUSTRY ANALYSIS:** A definitive answer on what OOXML structures Adobe Acrobat uses for Forms (Tabs vs Tables) and Multi-columns.
2. **THE MATHEMATICAL PIPELINE:** A step-by-step, 5-phase data pipeline algorithm.
   - *Phase 1:* Spatial Indexing (e.g., R-Tree / KD-Tree for O(log n) proximity searches).
   - *Phase 2:* Connected Component Analysis (Path intersection for tables).
   - *Phase 3:* Word & Line Formation (Docstrum/KNN thresholds).
   - *Phase 4:* Block Segmentation & Reading Order.
   - *Phase 5:* OOXML Serialization & Alignment Mapping.
3. **DATA STRUCTURE BLUEPRINTS:** Define the abstract logical structures (e.g., `ParagraphBlock`, `TableMatrix`, `TabGroup`) that the C++ engine must implement to hold the analyzed spatial data before OOXML generation.
4. **FAIL-SAFE HEURISTICS:** Explicit mathematical formulas/thresholds (e.g., `if vertical_gap > font_size * 1.5 then...`) to solve the edge cases in the 1 Million Use Case matrix.

**THIS IS A SUPER RESET.** Abandon all basic knowledge. Go deep into the mathematics of Document Layout Analysis and OOXML. Deliver the ultimate blueprint.
