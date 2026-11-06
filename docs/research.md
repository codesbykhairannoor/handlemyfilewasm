Architectural Specification for Enterprise-Grade PDF-to-OOXML Layout Reconstruction EnginesIndustry Analysis of High-Fidelity Reflow EnginesHigh-fidelity document conversion from PDF (ISO 32000-1) to Office Open XML (OOXML / ISO/IEC 29500) is one of the most computationally complex problems in Document Layout Analysis (DLA). Naive conversion engines attempt to preserve visual layout by wrapping fragmented text lines in absolutely positioned text boxes using <w:frame> or drawing objects. While visually similar to the original document, this layout mechanism prevents downstream editing. When a user adds or deletes characters, text cannot flow between lines, resulting in text overlapping, paragraph truncation, and document layout desynchronization.To achieve professional-grade editability, enterprise-grade conversion engines—such as Adobe Acrobat Pro, Google Cloud Document AI, and ABBYY FineReader—reject coordinate-based positioning in favor of logical layout reconstruction. The physical coordinates of PDF elements are mapped to a logical hierarchy of sections, columns, paragraphs, and tables. This approach ensures that text flows seamlessly across lines, pages, and columns when edited.The physical layout constraints of the PDF specification are translated to the flowable paradigms of OOXML by mapping coordinate clusters to specific logical XML tags. The table below contrasts the parsing strategies of naive coordinate-sorting engines with those of enterprise-level layout-reconstruction engines:Document ElementNaive Coordinate-Sorting EngineEnterprise Layout-Reconstruction EnginePrimary OOXML TargetsBorderless Forms & Key-Value ListingsHorizontal space character (" ") padding and character positioning.Multi-level 1D coordinate density projection clustering.Custom Tab Stops (<w:tabs>) with tab characters (<w:tab/>).Complex Multi-Column SectionsSplit columns into separate, non-communicating vertical text frames.Recursive X-Y cuts corrected by Docstrum-based deskew calculations.Continuous Section Breaks (<w:sectPr>) with column counts (<w:cols>).Grid Tables & Spanned CellsLoose bounding box overlap detection of text blocks.Graph-theoretic cycle extraction on vector intersection matrices.Structured Tables (<w:tbl>) with w:gridSpan and w:vMerge.Continuous Paragraph FlowHard line breaks (<w:br/> or separate <w:p>) at every physical PDF line.Proximity clustering of margins and vertical line spacing offsets.Flowing Paragraph Blocks (<w:p>) containing contiguous runs (<w:r>).Background & Overlay ShapesDiscarded entirely or treated as floating inline elements.Z-index sorting paired with page-boundary coordinate translation.Anchor Drawing Canvas (<wp:anchor>) with specific wrap styles.Architectural Alignment StrategiesTo determine when to use specific OOXML structures, such as Tables, Custom Tab Stops, or Sections, enterprise engines evaluate structural density and geometric grouping.Custom Tab Stops (<w:tabs>) vs. Grid Tables (<w:tbl>)When parsing aligned structures, such as borderless tables, forms, or key-value listings, the engine must choose between custom tab stops and grid tables. Enterprise architectures utilize custom tab stops when the text components share a common, linear baseline but are horizontally segmented into predictable vertical alignment lines (gutters). If any element in the aligned group exhibits multi-line wrapping or contains independent block elements, the engine dynamically upgrades the region to a borderless grid table. This prevents wrapping text from overlapping horizontally with adjacent logical columns.Multi-Column Sections (<w:cols>)For academic, magazine, or newspaper layouts, the standard implementation relies on Section Columns. Section columns allow text to flow naturally from the bottom of the left column to the top of the right column. When a multi-column region is bounded vertically by single-column text (such as an abstract or a section divider), the engine inserts a continuous section break (<w:type w:val="continuous"/>) and configures <w:cols w:num="N"> inside the section properties (<w:sectPr>). If the multi-column elements represent floating text blocks (e.g., callout boxes, advertisements), the engine isolates them into floating anchors (<wp:anchor>) with associated wrap properties.The Multi-Phase Mathematical DLA PipelineTo extract raw glyph coordinates and reconstruct them into an editable OOXML document, the engine must execute a deterministic, 5-phase pipeline.+-------------------------------------------------------------+
|                Phase 1: Spatial Indexing                    |
|  - Load Glyphs (PDF.js / PDFium WASM)                       |
|  - 2D R*-Tree Construction                                  |
|  - Coordinate Normalization (PDF Points to Word Twips/EMUs) |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|         Phase 2: Connected Component Analysis               |
|  - Extract Vector Graphic Paths                             |
|  - Line/Path Intersection Matrix Construction               |
|  - Closed-Cycle Table Grid Topology Inference                |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|             Phase 3: Word & Line Formation                  |
|  - Calculate Docstrum Spacing Histograms (D_char, D_word)   |
|  - Dynamic Inter-glyph Proximity Grouping                   |
|  - Horizontal Text-Line Compilation via Linear Regression   |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|         Phase 4: Block Segmentation & Reading Order         |
|  - Recursive XY-Cut with Dynamic Separator Space Thresholds |
|  - Formulate Directed Acyclic Graph (DAG)                   |
|  - Topological Sort for Native Reading Order Determination  |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|          Phase 5: OOXML Serialization & Mapping             |
|  - CSS-to-WordprocessingML Transform                        |
|  - XML Generation: <w:p>, <w:tbl>, <w:tabs>, <w:sectPr>     |
|  - Compile DOCX Container Structure                         |
+-------------------------------------------------------------+
Phase 1: Spatial Indexing and Coordinate TransformationThe system extracts raw glyph data from PDF.js or PDFium WebAssembly builds, receiving each character as a tuple of coordinates, dimensions, font properties, and unicode strings. To ensure efficient spatial queries, the engine constructs a two-dimensional $R^*$-tree containing the bounding box of each glyph:$$B = [x_{\min}, y_{\min}, x_{\max}, y_{\max}]$$The $R^*$-tree structure allows for $O(\log N)$ range and nearest-neighbor queries, which are essential for grouping nearby characters into words.Coordinate NormalizationPDF coordinates represent a Cartesian plane with the origin $(0,0)$ at the bottom-left corner, measured in typographic points ($1/72$ of an inch). WordprocessingML structures assume an origin at the top-left corner, with dimensions specified in Twips (twentieth of a point, $1/1440$ of an inch) or English Metric Units (EMUs, $1/914400$ of an inch).Let $H_{\text{page}}$ be the total page height in points. The transformation function mapping a PDF point coordinate $(x_{\text{pdf}}, y_{\text{pdf}})$ to a Word Twip coordinate $(x_{\text{word}}, y_{\text{word}})$ is defined as:$$x_{\text{word}} = x_{\text{pdf}} \times 20$$$$y_{\text{word}} = (H_{\text{page}} - y_{\text{pdf}}) \times 20$$This normalization must occur prior to any structural clustering.Phase 2: Connected Component Analysis and Vector Path IntersectionPDF documents render tables using vector graphic drawing paths, typically represented by a sequence of line-to (l), move-to (m), and rectangle (re) operators. The engine intercepts these path commands and transforms them into a collection of horizontal and vertical line segments:$$H = \{h_1, h_2, \dots, h_m\}, \quad V = \{v_1, v_2, \dots, v_k\}$$An intersection point $P_{ij}$ is formed where a horizontal segment $h_i$ crosses a vertical segment $v_j$:$$P_{ij} = h_i \cap v_j$$The engine constructs an intersection adjacency matrix to map these crossings. Vertices are mapped to a planar graph $G = (P, E)$, where edges $E$ represent physical vector paths connecting the intersection points. Minimally enclosed cycles within this graph represent the boundaries of individual table cells.Phase 3: Word and Line FormationTo convert raw glyphs into continuous text lines, the engine constructs a nearest-neighbor angle and distance histogram, a methodology derived from the Docstrum algorithm. For each glyph $g_i$, the $k$-nearest glyphs are identified ($k \ge 5$). The centroid distance $d$ and angle $\theta$ between the centroids are computed.The resulting distance histogram exhibits distinct peaks:Peak 1 ($D_{\text{char}}$): Represents character spacing within words.Peak 2 ($D_{\text{word}}$): Represents word spacing within lines.Peak 3 ($D_{\text{line}}$): Represents line-to-line vertical spacing.Character Grouping HeuristicTwo glyphs $g_a$ and $g_b$ with horizontal bounding box boundaries $[x_{a,\min}, x_{a,\max}]$ and $[x_{b,\min}, x_{b,\max}]$ are merged into a single word if they satisfy the horizontal proximity constraint:$$x_{b,\min} - x_{a,\max} \le D_{\text{char}} \times (1.0 + \epsilon)$$And the vertical baseline overlap constraint:$$\max(y_{a,\min}, y_{b,\min}) - \min(y_{a,\max}, y_{b,\max}) \ge 0.5 \times \min(H_a, H_b)$$Where $H_i$ is the font size (height) of the glyph. Once words are compiled, they are linked into horizontal text lines using the $D_{\text{word}}$ threshold.Phase 4: Block Segmentation and Reading Order ReconstructionTo partition the page into column blocks and paragraphs, the engine applies the recursive X-Y cut (RXYC) algorithm. RXYC is a top-down algorithm that projects the bounding boxes of all text lines onto the horizontal ($X$) and vertical ($Y$) axes to construct projection profiles:$$P_x(x) = \sum_{i} \mathbb{I}(x \in [x_{i,\min}, x_{i,\max}])$$$$P_y(y) = \sum_{i} \mathbb{I}(y \in [y_{i,\min}, y_{i,\max}])$$Where $\mathbb{I}$ is an indicator function evaluating to 1 if the coordinate falls within the line's span.Valleys in $P_x(x)$ indicate vertical white spaces (gutters). If a valley width exceeds the dynamic threshold $T_x = D_{\text{word}} \times 4.0$, the node is split vertically. Valleys in $P_y(y)$ exceeding $T_y = D_{\text{line}} \times 1.5$ indicate paragraph or block separations, triggering horizontal cuts. This process is executed recursively, generating a hierarchical tree where leaves represent individual layout blocks (paragraphs, tables, or floating figures).Reading Order Directed Acyclic Graph (DAG)Because raw PDF Z-index sorting may not match logical reading flow, the engine constructs a Directed Acyclic Graph $G_{\text{read}} = (V, E)$, where $V$ represents the segmented leaf blocks from the RXYC step. A directed edge $e_{ab} = (v_a, v_b)$ is created if block $v_a$ topologically precedes block $v_b$ based on two spatial constraints:Vertical Dominance: $y_{a,\max} < y_{b,\min}$ and the horizontal projections of $v_a$ and $v_b$ overlap.Horizontal Precedence: $x_{a,\max} < x_{b,\min}$ and both blocks share overlapping vertical coordinates, within a multi-column region.The engine executes a topological sort on $G_{\text{read}}$ to generate the final reading order for sequential serialization.Phase 5: OOXML Serialization and Style TransformationThe final phase converts the logical structure tree into the physical zip container format required by the Open Packaging Conventions (OPC) specification. The text lines are converted into runs (<w:r>) inside paragraphs (<w:p>) and tables (<w:tbl>).A standard translation matrix maps CSS-like text attributes to OOXML attributes:Source Property (PDF / CSS)Target OOXML Element / AttributeTwips / Units ConversionFont Family<w:rFonts w:ascii="..." w:hAnsi="..."/>Direct string assignment.Font Size (Points)<w:sz w:val="value"/>$\text{value} = \text{Points} \times 2$.Bold Weight<w:b/>Boolean toggle.Italic Style<w:i/>Boolean toggle.Line Spacing (Pt)<w:spacing w:line="..." w:lineRule="auto"/>$\text{twips} = \text{Points} \times 20$.Left Margin / Indent<w:ind w:left="value"/>$\text{value} = \text{Points} \times 20$.Paragraph Before Spacing<w:spacing w:before="value"/>$\text{value} = \text{Points} \times 20$.Paragraph After Spacing<w:spacing w:after="value"/>$\text{value} = \text{Points} \times 20$.The serialized text streams are written directly into word/document.xml within the output ZIP container.Detailed Evaluation of the Edge Case MatrixCategory A: The Form & Borderless Alignment NightmareThe Core Algorithmic ProblemIn a standard form layout containing key-value pairs or alignment symbols (such as colons or dotted lines), horizontal text strings frequently overlap when projected onto a single axis. For example:Line 1: Nama : John Doe
Line 2: Tempat/Tanggal Lahir : Jakarta
The bounding box of "Tempat/Tanggal Lahir" is horizontally wider than "Nama". If the engine projects the coordinate bounds onto the vertical $Y$-axis, it detects two distinct, non-overlapping horizontal baselines. However, a horizontal projection onto the $X$-axis reveals no clear vertical cut or gutter between the keys and the values because "Tempat/Tanggal Lahir" overlaps horizontally with the colon and the start of "John Doe" on the line above.A naive RXYC algorithm fails because the vertical split threshold is never met, causing the entire block to merge into a single, scrambled text block. If the engine attempts to resolve this by inserting space characters (" ") to align the colons, proportional font rendering causes layout drifting when opened in Word processing software.The Enterprise SolutionTo resolve this without using absolute coordinates, enterprise engines apply density-based spatial clustering to the starting and ending positions of the text segments.First, the algorithm extracts all text fragments on adjacent baselines and projects only their starting $X$-coordinates ($x_{\min}$) onto a horizontal density function $f(x)$:$$f(x) = \sum_{i=1}^{N} \exp\left(-\frac{(x - x_{\min, i})^2}{2\sigma^2}\right)$$Where $\sigma$ represents a smoothing factor scaled to the average character width $D_{\text{char}}$. The local maxima (peaks) of $f(x)$ identify the vertical alignment axes of the form. If multiple lines share coordinates near these peaks, the engine classifies the region as an aligned form structure.Furthermore, if a specific character—such as a colon (:) or a period (.)—recurs at a shared alignment axis across multiple lines, the engine isolates it as a structural anchor.               Peak 1                   Peak 2              Peak 3
                 |                        |                   |
Line 1:  [Nama]  |                        [ : ]               [John Doe]
Line 2:  [Tempat/Tanggal Lahir]           [ : ]               [Jakarta]
                 |                        |                   |
                 v                        v                   v
            X = 54.0 pt              X = 180.0 pt        X = 198.0 pt
To reconstruct this layout while preserving editing flow, the engine maps these peaks to custom tab stops (<w:tabs>) in Word processing XML. The horizontal distance from the left page margin to each peak is calculated in points and converted to Twips:$$\text{Position (Twips)} = X_{\text{peak}} \times 20$$If a text block contains multi-line wrapping (where a value wraps onto a second line without overlapping the next key), the engine dynamically upgrades the layout to a borderless table (<w:tbl>). This isolates each field into a cell, preventing wrapped text from flowing into adjacent columns.The OOXML MappingThe following OOXML structure maps these alignment points to a single paragraph utilizing custom tab stops, eliminating the need for alignment spaces:XML<w:p>
  <w:pPr>
    <w:tabs>
      <w:tab w:val="left" w:leader="none" w:pos="3600"/>
      <w:tab w:val="left" w:leader="none" w:pos="3960"/>
    </w:tabs>
  </w:pPr>
  <w:r>
    <w:t>Tempat/Tanggal Lahir</w:t>
  </w:r>
  <w:r>
    <w:tab/>
  </w:r>
  <w:r>
    <w:t>:</w:t>
  </w:r>
  <w:r>
    <w:tab/>
  </w:r>
  <w:r>
    <w:t>Jakarta</w:t>
  </w:r>
</w:p>
Category B: Complex Grid Tables & Spanned CellsThe Core Algorithmic ProblemPDF files contain no logical structural markup for tables, such as row or column container tags. A PDF renders a table by drawing raw text strings and vector paths (lines and rectangles) onto a coordinate canvas. Naive sorting algorithms attempt to reconstruct tables by grouping text blocks that fall within a bounding box.This approach fails on complex grid tables with spanned cells, multi-line cell content, or missing borders. Because the engine cannot determine which cells are merged, it often splits spanned text into multiple adjacent cells or groups unrelated vertical headers into a single cell, rendering the table un-editable.The Enterprise SolutionEnterprise engines reconstruct table geometry using Table Structure Recognition (TSR) algorithms. The layout engine executes an intersection analysis on the extracted horizontal ($H$) and vertical ($V$) vector segments to build a corner-adjacency matrix $M$:$$M_{ab} = \begin{cases}  1 & \text{if vertex } P_a \text{ connects to } P_b \text{ via a continuous vector path} \\ 0 & \text{otherwise} \end{cases}$$The algorithm parses this matrix to construct a planar graph $G = (P, E)$. It then extracts the minimally enclosed cycles (faces) of the graph to identify the boundaries of individual cells.  P0 -------- P1 -------- P2
  |            |            |
  |   Cell A   |   Cell B   |
  |            |            |
  P3 -------- P4 -------- P5
  |                         |
  |         Cell C          |
  |                         |
  P6 -------------------- P7
To calculate cell spans across rows or columns, the engine projects all cell coordinates onto a reference grid defined by the unique, sorted coordinates of the table boundaries:$$X_{\text{grid}} = \{X_0, X_1, \dots, X_C\}, \quad Y_{\text{grid}} = \{Y_0, Y_1, \dots, Y_R\}$$Let a physical cell bounding box be $B_{\text{cell}} = [x_{\min}, y_{\min}, x_{\max}, y_{\max}]$. The logical grid coordinates are mapped to grid indexes using an approximation threshold $\delta = 3.0 \text{ pt}$:$$c_1 = \arg\min_k \vert{}x_{\min} - X_{\text{grid}}[k]\vert{}, \quad c_2 = \arg\min_k \vert{}x_{\max} - X_{\text{grid}}[k]\vert{}$$$$r_1 = \arg\min_k \vert{}y_{\min} - Y_{\text{grid}}[k]\vert{}, \quad r_2 = \arg\min_k \vert{}y_{\max} - Y_{\text{grid}}[k]\vert{}$$The column span and row span are defined as:$$\text{ColSpan} = c_2 - c_1, \quad \text{RowSpan} = r_2 - r_1$$If $\text{ColSpan} > 1$, the engine generates <w:gridSpan w:val="ColSpan"/> in the cell properties (<w:tcPr>). If $\text{RowSpan} > 1$, it applies the OOXML vertical merge property.To assign raw text glyphs to the correct spatial cells, the engine maps the centroid of each word $W$ to the cell boundary polygons. A word is assigned to a cell if its centroid coordinate $(x_c, y_c)$ falls within the cell's bounding box:$$x_{\min} \le x_c \le x_{\max} \quad \text{and} \quad y_{\min} \le y_c \le y_{\max}$$If a word overlaps a cell boundary, the engine calculates the intersection area and assigns the text to the cell with the largest overlap.The OOXML MappingThe following OOXML structure represents a table with a column span of 2 in the first row and a vertical row merge in the first column:XML<w:tbl>
  <w:tblPr>
    <w:tblW w:w="5000" w:type="dxa"/>
  </w:tblPr>
  <w:tblGrid>
    <w:gridCol w:w="2500"/>
    <w:gridCol w:w="2500"/>
  </w:tblGrid>
  <w:tr>
    <w:tc>
      <w:tcPr>
        <w:gridSpan w:val="2"/>
      </w:tcPr>
      <w:p>
        <w:r><w:t>Header Span (ColSpan 2)</w:t></w:r>
      </w:p>
    </w:tc>
  </w:tr>
  <w:tr>
    <w:tc>
      <w:tcPr>
        <w:vMerge w:val="restart"/>
      </w:tcPr>
      <w:p>
        <w:r><w:t>Merged Row Start</w:t></w:r>
      </w:p>
    </w:tc>
    <w:tc>
      <w:p>
        <w:r><w:t>Cell 2</w:t></w:r>
      </w:p>
    </w:tc>
  </w:tr>
  <w:tr>
    <w:tc>
      <w:tcPr>
        <w:vMerge w:val="continue"/>
      </w:tcPr>
      <w:p/>
    </w:tc>
    <w:tc>
      <w:p>
        <w:r><w:t>Cell 4</w:t></w:r>
      </w:p>
    </w:tc>
  </w:tr>
</w:tbl>
Category C: Multi-Column & Reading Order ReconstructionThe Core Algorithmic ProblemPDF files render text objects based on drawing order (Z-index) rather than logical reading flow. If an engine parses PDF characters sequentially based on Z-index or raw coordinate sorting, it often reads across columns. For example, in a two-column layout, a naive line-merging engine reads the first line of the left column, immediately followed by the first line of the right column, rendering the resulting text unreadable.The Enterprise SolutionTo reconstruct the correct reading order, enterprise engines first partition the page layout into distinct columns before grouping lines into paragraphs.The engine projects the bounding boxes of all text lines onto the horizontal $X$-axis to generate a projection profile $P_x(x)$. A vertical gutter is identified where the projection profile drops to zero ($P_x(x) = 0$) across a continuous vertical span:$$\text{Gutter Width} = x_{\text{right}} - x_{\text{left}} \ge W_{\text{page}} \times 0.04$$If the gutter width exceeds $4\%$ of the total page width, the engine splits the page vertically at the midpoint of the gutter.               Column 1               Gutter             Column 2
        +--------------------+                     +--------------------+
        |                    |      Px(x) = 0      |                    |
        |                    |       Vertical      |                    |
        |                    |         Cut         |                    |
        |                    |          |          |                    |
        +--------------------+          v          +--------------------+
                             |<---------->|
                              Gutter Width
To resolve minor page rotations (skew) that skew the vertical projection, the engine calculates the page's orientation using a Docstrum-based nearest-neighbor angle histogram. The skew angle $\theta$ is determined by the dominant peak in the angle histogram. All extracted coordinates are rotated by $-\theta$ around the page center before applying the projection cuts:$$x' = x \cos(-\theta) - y \sin(-\theta)$$$$y' = x \sin(-\theta) + y \cos(-\theta)$$Once columns are isolated, the engine constructs a Directed Acyclic Graph (DAG) $G_{\text{read}} = (V, E)$ to model the reading flow. Vertices $V$ represent the segmented layout blocks. Directed edges $E$ are defined based on geometric boundaries:Vertical Precedence: A directed edge $e_{ab}$ connects block $A$ to block $B$ if $A$ is vertically above $B$ within the same column boundary.Horizontal Column Precedence: In multi-column sections, a directed edge connects the bottom block of column $k$ to the top block of column $k+1$.Applying a topological sort to this graph defines the logical reading order.To preserve page flow and editability, enterprise engines map continuous multi-column structures to native Word columns. This implementation allows text to flow naturally between columns. The engine inserts continuous section breaks (<w:sectPr>) around multi-column text blocks to isolate them from single-column elements.        +-------------------------------------------------+
        |                 Single Column Header            |
        +-------------------------------------------------+
        ================== Section Break ==================  <- w:type w:val="continuous"
        +--------------------+       +--------------------+
        |                    |       |                    |
        |      Column 1      |       |      Column 2      |  <- w:cols w:num="2"
        |                    |       |                    |
        +--------------------+       +--------------------+
        ================== Section Break ==================  <- w:type w:val="continuous"
        +-------------------------------------------------+
        |                 Single Column Footer            |
        +-------------------------------------------------+
Using nested tables or absolute text boxes to isolate columns is restricted to fixed sidebars or callout boxes, as text cannot flow dynamically between these elements.The OOXML MappingThe following OOXML mapping configures a continuous document section with a two-column layout:XML<w:p>
  <w:r><w:t>This paragraph resides in a single-column layout above the section break.</w:t></w:r>
</w:p>
<w:p>
  <w:pPr>
    <w:sectPr>
      <w:type w:val="continuous"/>
      <w:cols w:num="2" w:space="720"/>
    </w:sectPr>
  </w:pPr>
</w:p>
<w:p>
  <w:r><w:t>This text flows dynamically inside the first column of the multi-column section.</w:t></w:r>
</w:p>
Category D: Paragraph Reconstruction & Font MetricsThe Core Algorithmic ProblemPDF files represent text as absolutely positioned single-line fragments or isolated character runs. The physical line breaks in a PDF are hard-coded. If a conversion engine outputs a hard break (<w:br/>) or initializes a new paragraph (<w:p>) at every physical line, the converted document becomes un-editable. When a user modifies the text, line wraps cannot adjust dynamically, resulting in layout drifting.The Enterprise SolutionTo reconstruct continuous, reflowable paragraphs, the engine analyzes line spacing, alignment, and right margin alignment offsets to differentiate soft wraps from hard paragraph breaks.Let $L_i$ and $L_{i+1}$ be two vertically adjacent text lines. Let $H_i$ represent the primary font size of line $L_i$, and $X_{\text{margin\_right}}$ represent the calculated right margin boundary of the column block.                  Left Margin                                Right Margin
                       |                                           |
Line i:                | This is a line of text that flows up to   | <- Ends near margin (Soft wrap)
Line i+1:              | the right edge.                           | <- Ends early (Hard break)
                       |                                           |
                       |<----------------------------------------->|
                                       Block Width
The engine evaluates three primary metrics to classify the line transition:Vertical Distance Offset ($G_y$): The gap between the baselines of $L_i$ and $L_{i+1}$ is compared to the line-height threshold.Right Margin Deficit ($D_x$): The distance from the rightmost edge of $L_i$ to the column's right boundary ($X_{\text{margin\_right}} - x_{i,\max}$).Left Indent Shift ($\Delta X_{\text{left}}$): The horizontal offset between the starting positions of the two lines ($x_{i+1,\min} - x_{i,\min}$).The classification rules are defined as:$$\text{Transition}(L_i, L_{i+1}) = \begin{cases} \text{Soft Break} & \text{if } G_y \le H_i \times 1.5 \text{ and } D_x \le H_i \times 3.0 \text{ and } \Delta X_{\text{left}} \approx 0 \\ \text{Hard Break} & \text{otherwise} \end{cases}$$Soft Break Case: The lines are grouped into a single paragraph block (<w:p>). The physical line break is discarded, allowing the text to flow naturally.Hard Break Case: The engine inserts a paragraph break, creating a new <w:p> node.Handling Justified TextJustified text in PDF files is rendered by adding extra horizontal kerning or spacing between individual words and characters. Naive engines attempt to replicate this alignment by inserting physical spaces (" ") or empty character runs between words. This breaks text editing and causes alignment distortions in Word processing software.Enterprise engines extract only the raw text, discard the artificial character-spacing gaps, and apply native justification formatting to the paragraph container:XML<w:pPr>
  <w:jc w:val="both"/>
</w:pPr>
Handling Superscript and SubscriptPDF files render superscript and subscript characters (such as exponents or footnote references) by reducing the font size and shifting the vertical coordinate baseline.The engine detects these structures by comparing the run's baseline position $y_{\text{run}}$ and font height $H_{\text{run}}$ to the primary baseline $Y_{\text{baseline}}$ and font size $H_{\text{line}}$ of the text line:$$\Delta y = y_{\text{run}} - Y_{\text{baseline}}$$$$\text{ScaleFactor} = \frac{H_{\text{run}}}{H_{\text{line}}}$$Superscript: Detected if $\Delta y \ge H_{\text{line}} \times 0.2$ and $\text{ScaleFactor} \le 0.8$. It is mapped to <w:vertAlign w:val="superscript"/>.Subscript: Detected if $\Delta y \le -H_{\text{line}} \times 0.1$ and $\text{ScaleFactor} \le 0.8$. It is mapped to <w:vertAlign w:val="subscript"/>.The OOXML MappingThe following OOXML markup represents a single, justified paragraph containing inline superscript and subscript runs:XML<w:p>
  <w:pPr>
    <w:jc w:val="both"/>
  </w:pPr>
  <w:r>
    <w:t>The math equation is defined as X</w:t>
  </w:r>
  <w:r>
    <w:rPr>
      <w:vertAlign w:val="superscript"/>
    </w:rPr>
    <w:t>2</w:t>
  </w:r>
  <w:r>
    <w:t> and the variable is indexed as Y</w:t>
  </w:r>
  <w:r>
    <w:rPr>
      <w:vertAlign w:val="subscript"/>
    </w:rPr>
    <w:t>k</w:t>
  </w:r>
</w:p>
Category E: Floating Objects & Z-Index LayeringThe Core Algorithmic ProblemPDF files use a flat coordinate space where images, text, and vector graphics are rendered sequentially based on drawing order (Z-index). Word processing engines, however, rely on inline document flow. If a conversion engine maps PDF background watermarks or floating layout images directly inline, it breaks paragraph flow, pushing text onto adjacent pages or scrambling the logical reading layout.The Enterprise SolutionTo preserve layout flow, the engine maps absolute PDF coordinates to Word's drawing canvas elements using <wp:anchor> wrapping.Let a PDF image bounding box be $B_{\text{img}} = [x_{\min}, y_{\min}, x_{\max}, y_{\max}]$. The engine converts these coordinates from points to English Metric Units (EMUs):$$W_{\text{emu}} = (x_{\max} - x_{\min}) \times 12700$$$$H_{\text{emu}} = (y_{\max} - y_{\min}) \times 12700$$To calculate the image's top-left offset relative to the page margins, the engine applies the coordinate transformation:$$X_{\text{emu\_offset}} = (x_{\min} - X_{\text{margin\_left}}) \times 12700$$$$Y_{\text{emu\_offset}} = (Y_{\text{margin\_top}} - y_{\max}) \times 12700$$These values are written to <wp:positionH> and <wp:positionV> within the anchoring element.Wrap Style and Z-Index ClassificationTo determine how text flows around an image, the engine analyzes spatial overlaps between the image and nearby text blocks:Background Watermarks: If the image dimensions closely match the page boundary ($W_{\text{img}} \ge W_{\text{page}} \times 0.9$ and $H_{\text{img}} \ge H_{\text{page}} \times 0.9$), and other text elements overlap its area, the engine classifies it as a background image. It is serialized using behindDoc="1".Tight Wrap Style (<wp:wrapSquare>): If a floating image overlaps a segmented text block, and the intersection area is less than $50\%$ of the text block area, the engine configures text wrapping around the image boundary using <wp:wrapSquare wrapText="bothSides"/>.Inline Image: If the image does not overlap any text blocks and resides on its own horizontal baseline, it is classified as an inline image and mapped using <wp:inline>.The OOXML MappingThe following OOXML structure anchors a floating image relative to the top-left page margins, enabling square text wrapping on both sides:XML<w:p>
  <w:r>
    <w:drawing>
      <wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251658240" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">
        <wp:simplePos x="0" y="0"/>
        <wp:positionH relativeFrom="margin">
          <wp:posOffset>1143000</wp:posOffset>
        </wp:positionH>
        <wp:positionV relativeFrom="margin">
          <wp:posOffset>1905000</wp:posOffset>
        </wp:positionV>
        <wp:extent cx="2336800" cy="1752600"/>
        <wp:effectExtent l="0" t="0" r="0" b="0"/>
        <wp:wrapSquare wrapText="bothSides"/>
        <docPr id="1" name="Picture 1"/>
        <cNvGraphicFramePr>
          <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
        </cNvGraphicFramePr>
        <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:graphicData xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
            <!-- Binary reference nodes for image rendering -->
          </a:graphicData>
        </a:graphic>
      </wp:anchor>
    </w:drawing>
  </w:r>
</w:p>
Logical C++ Data StructuresThe following modern, compile-ready C++17 library declarations define the logical data models used by the client-side DLA engine to hold spatial and structural metadata before serialization:C++#pragma once

#include <string>
#include <vector>
#include <memory>
#include <map>
#include <cmath>
#include <algorithm>

namespace DLA {

// Represents a 2D bounding box in absolute coordinate space
struct BoundingBox {
    double xMin = 0.0;
    double yMin = 0.0;
    double xMax = 0.0;
    double yMax = 0.0;

    double width() const { return xMax - xMin; }
    double height() const { return yMax - yMin; }

    bool intersects(const BoundingBox& other) const {
        return (xMin <= other.xMax && xMax >= other.xMin &&
                yMin <= other.yMax && yMax >= other.yMin);
    }

    BoundingBox getIntersection(const BoundingBox& other) const {
        return BoundingBox{
            std::max(xMin, other.xMin),
            std::max(yMin, other.yMin),
            std::min(xMax, other.xMax),
            std::min(yMax, other.yMax)
        };
    }

    double area() const {
        double w = width();
        double h = height();
        return (w > 0.0 && h > 0.0) ? (w * h) : 0.0;
    }
};

enum class AlignType {
    Left,
    Center,
    Right,
    Justified
};

struct Style {
    std::string fontFamily = "Calibri";
    double fontSize = 11.0; // In points
    bool isBold = false;
    bool isItalic = false;
    std::string hexColor = "000000";
};

struct Glyph {
    std::wstring unicodeChar;
    BoundingBox bbox;
    Style style;
};

struct Word {
    std::wstring text;
    BoundingBox bbox;
    Style style;
    std::vector<Glyph> glyphs;
};

struct TextLine {
    BoundingBox bbox;
    std::vector<Word> words;
    AlignType alignment = AlignType::Left;
};

struct TabStop {
    double xPosPoints = 0.0;
    std::string type = "left"; // Options: left, right, center, decimal
};

struct TabGroup {
    std::vector<TabStop> tabStops;
    double alignmentTolerancePoints = 5.0;
};

struct ParagraphBlock {
    BoundingBox bbox;
    std::vector<TextLine> lines;
    TabGroup localTabGroup;
    bool isDoubleColumn = false;
    double lineSpacingPoints = 12.0;
    double spaceBefore = 0.0;
    double spaceAfter = 6.0;
};

struct Cell {
    BoundingBox bbox;
    size_t startRow = 0;
    size_t endRow = 0;
    size_t startCol = 0;
    size_t endCol = 0;
    std::vector<ParagraphBlock> contentBlocks;

    size_t colSpan() const { return endCol - startCol; }
    size_t rowSpan() const { return endRow - startRow; }
};

struct TableMatrix {
    BoundingBox bbox;
    std::vector<double> gridCols; // Horizontal coordinate grid steps
    std::vector<double> gridRows; // Vertical coordinate grid steps
    std::vector<std::vector<Cell>> cells; // Merged table grid matrix
};

enum class ImageWrapStyle {
    Inline,
    Square,
    Tight,
    BehindText,
    InFrontOfText
};

struct GraphicObject {
    BoundingBox bbox;
    std::vector<uint8_t> binaryData;
    std::string mimeType = "image/png";
    ImageWrapStyle wrapStyle = ImageWrapStyle::Inline;
    size_t zIndex = 0;
};

struct DocumentPage {
    size_t pageNumber = 0;
    double widthPoints = 612.0;  // Letter size default width
    double heightPoints = 792.0; // Letter size default height
    
    // Ordered sequence parsed by topological DAG sort
    std::vector<std::shared_ptr<ParagraphBlock>> paragraphs;
    std::vector<std::shared_ptr<TableMatrix>> tables;
    std::vector<std::shared_ptr<GraphicObject>> floatingGraphics;
};

} // namespace DLA
Fail-Safe Heuristics & Mathematical ThresholdsThe table below defines the decision-making rules used by the DLA engine to classify layout components and handle complex edge cases during coordinate-to-OOXML transformation:Scenario / Decision PathChecked ConditionApplied Threshold / EquationAssigned OOXML ActionCharacter AssociationHorizontal gap between contiguous glyphs ($G_x$).$G_x \le D_{\text{char}} \times 1.15$Merge characters into a single Word node.Word SegmentationHorizontal gap between adjacent words ($G_w$).$G_w > D_{\text{char}} \times 1.15$ and $G_w \le D_{\text{word}} \times 1.4$Group words into a shared TextLine block.Form Column DetectionDensity projection peaks ($f(x)$) for forms.$f(X_{\text{peak}}) \ge 0.5 \times N_{\text{lines\_in\_block}}$Register custom tab stops (<w:tabs>) at the peak coordinates.Form to Table UpgradeWrapping within form elements.$\text{LineCount}(V_{\text{value\_field}}) > 1$Convert the form region to a borderless table (<w:tbl>).Continuous Column SplitVertical projection profile valley width ($W_v$).$W_v \ge W_{\text{page}} \times 0.04$Split the layout into separate reading columns.Soft Break DetectionLine wrapping within a paragraph block.$G_y \le H_{\text{font}} \times 1.5$ and $D_{\text{right\_margin}} \le H_{\text{font}} \times 3.0$.Keep text lines in the current flowable paragraph (<w:p>).Hard Break DetectionNew paragraph instantiation.$G_y > H_{\text{font}} \times 1.5$ or $D_{\text{right\_margin}} > H_{\text{font}} \times 3.0$.Complete current paragraph and insert new <w:p> node.Table Cell AssignmentAssigning isolated text elements to a cell.$\frac{\text{Area}(B_L \cap C_{\text{cell}})}{\text{Area}(B_L)} \ge 0.70$.Serialize the text run inside the target table cell (<w:tc>).Background ImagePage boundary overlap analysis.$W_{\text{img}} \ge W_{\text{page}} \times 0.9$ and $H_{\text{img}} \ge H_{\text{page}} \times 0.9$Set anchor background property (behindDoc="1").Inline Image AssignmentText run overlap analysis.$\text{Area}(B_{\text{img}} \cap B_{\text{text}}) = 0$Set inline image layout XML node (<wp:inline>).Superscript ClusteringCharacter baseline deviation ($\Delta y$).$\Delta y \ge H_{\text{font}} \times 0.2$ and $\text{ScaleFactor} \le 0.8$Map the run to <w:vertAlign w:val="superscript"/>.Subscript ClusteringCharacter baseline deviation ($\Delta y$).$\Delta y \le -H_{\text{font}} \times 0.1$ and $\text{ScaleFactor} \le 0.8$Map the run to <w:vertAlign w:val="subscript"/>.