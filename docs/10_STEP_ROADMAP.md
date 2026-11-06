# 🗺️ Master Roadmap: 10 Langkah Pembangunan Mesin Pure Rust PDF-to-DOCX

Dokumen pelacak progres rekayasa modular bertahap untuk membangun mesin konversi PDF ke DOCX berkinerja tinggi murni dalam bahasa **Rust**.

---

## DAFTAR CHECKLIST PROGRESS

- [x] **Step 1: Persiapan Lingkungan & Toolchain Rust**
  - [x] Instalasi Rustup & Toolchain (`rustc 1.99.1`, `cargo 1.99.1`).
  - [x] Pemasangan target WebAssembly `wasm32-unknown-unknown`.
  - [x] Pemasangan biner resmi `wasm-pack` (v0.13.1).
  - [x] Pemindahan cache `.rustup` dan `.cargo` ke drive `D:\` (menghemat ~2 GB ruang drive `C:`).
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 2: Inisialisasi Workspace & Core Crate**
  - [x] Desain arsitektur crate modular (`pdftodocx`).
  - [x] Konfigurasi `Cargo.toml` (dependensi `lopdf`, `quick-xml`, `zip`, `flate2`, `serde`, `wasm-bindgen`, `rstar`, `petgraph`).
  - [x] Definisi struktur data model inti (`src/models/mod.rs`): `Point`, `Rect`, `Matrix2D`, `Style`, `Glyph`, `Word`, `TextLine`, `ParagraphBlock`, `Cell`, `TableMatrix`, `VectorPath`, `DocumentPage`.
  - [x] Modul utilitas matematika satuan (`src/utils/mod.rs`): Twips, Half-Points, Eighths of a Point, EMUs, dan XML Escaping.
  - [x] Ekspor fungsi WebAssembly `#[wasm_bindgen] pub fn convert_pdf_to_docx(pdf_data: &[u8]) -> Result<Vec<u8>, JsValue>`.
  - [x] Kompilasi rilis WebAssembly via `wasm-pack build --target web --out-dir pkg` sukses 100% menghasilkan `pkg/pdftodocx_bg.wasm`.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 3: Low-Level PDF Binary Parser & Dekompresi Prediktor PNG Paeth**
  - [x] Pembacaan struktur biner PDF melalui `lopdf::Document::load_mem`.
  - [x] Penguraian pohon halaman (`/Pages` -> `/Page`), ekstraksi dimensi fisik `CropBox`/`MediaBox` ($W \times H$).
  - [x] Ekstraksi kamus sumber daya `/Resources` (`/Font` dan `/XObject` references).
  - [x] Penggabungan aliran konten `/Contents` multi-stream.
  - [x] Implementasi pembalik prediktor PNG (ISO 32000-1 §7.4.4.4) mencakup filter Sub, Up, Average, dan Paeth linear algorithm ($p = L + A - AL$).
  - [x] Pengujian ujung-ke-ujung WebAssembly dengan `sample.pdf` nyata via Node.js (`tests/test_wasm_step3.js`) berhasil 100%.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 4: Evaluator Matriks Afin 2D & Graphics State Machine**
  - [x] Tumpukan status grafis (*graphics state stack* `q` / `Q`).
  - [x] Implementasi perkalian matriks afin 2D linear $3 \times 3$ untuk `cm` (CTM), `Tm` (Text Matrix), `Td` / `TD` (displacement), dan `T*`.
  - [x] Perhitungan matriks perenderan efektif $\mathbf{T_{rm}} = \mathbf{T_{style}} \times \mathbf{T_m} \times \mathbf{CTM}$.
  - [x] Ekstraksi jalur garis vektor (`m`, `l`, `re`) untuk deteksi tabel masa depan.
  - [x] Evaluasi rekursif Form XObject (`Do`) dengan perbanyakan matriks $\mathbf{CTM_{new}} = \mathbf{FormMatrix} \times \mathbf{CTM}$.
  - [x] Pembalikan sumbu vertikal Kartesius (origin $Y$ kiri-bawah ke origin $Y$ kiri-atas standar Word).
  - [x] Pengujian nyata WebAssembly pada `sample.pdf` berhasil mengekstrak 14 glif dan 4 garis vektor di koordinat presisi.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 5: Dekoder Font, Metrik Glif, & `/ToUnicode` CMap**
  - [x] Pengurai kamus `/Font` (Type 1, TrueType, Type 0 CIDFont Identity-H dengan `/DescendantFonts`, `/DW`, dan `/W`).
  - [x] Penerjemah PostScript CMap `/ToUnicode` ke UTF-8 (menghilangkan 100% bug teks alien/kotak-kotak, mendukung ligatur seperti `fi`, `fl`).
  - [x] Penanganan array `/Differences` dan pemetaan nama glif Adobe standar (ISO 32000-1 §9.6.5).
  - [x] Pembersihan nama font subset acak (contoh: `"BAAAAA+Arial-BoldMT"` -> `"Arial"`).
  - [x] Ekstraksi atribut gaya tipografi (`is_bold`, `is_italic`, `hex_color` untuk grayscale, RGB, dan CMYK).
  - [x] Pengujian nyata WebAssembly pada `sample.pdf` via `tests/test_wasm_step5.js` sukses 100% mengekstrak `"Dummy PDF file"`.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 6: Ekstraktor Semantik Tagged PDF (Jalur Cepat 100% Presisi)**
  - [x] Pembacaan pohon struktur `/StructTreeRoot` dan pemetaan custom `/RoleMap` (contoh: `/Heading1` -> `H1`, `/CustomPara` -> `P`).
  - [x] Ekstraktor Daftar Isi/Bookmark `/Outlines` dengan dekode teks UTF-16BE (BOM 0xFEFF) dan pemetaan koordinat target halaman.
  - [x] Penelusuran rekursif elemen struktur `/StructElem` (`H1..H6`, `P`, `Table`, `TR`, `TH`, `TD`, `L`, `LI`, `Span`, `Artifact`).
  - [x] Pelacak penanda Marked Content di Content Stream (`BMC`, `BDC ... EMC`) yang menyematkan atribut `mcid` pada setiap glif teks.
  - [x] Rekonstruksi otomatis tabel semantik lengkap dengan atribut penggabungan sel horizontal (`col_span`) dan vertikal (`row_span`).
  - [x] Pengujian nyata WebAssembly via `tests/test_wasm_step6.js` sukses 100% pada `sample.pdf` dan berkas Tagged PDF sintetis (`tagged_sample.pdf`).
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 7: Mesin Heuristik DLA (Docstrum & Recursive X-Y Cut)**
  - [x] Algoritma Docstrum ($k$-NN via $R^*$-Tree spatial indexing) untuk mencari ambang adaptif $D_{char}, D_{word}, D_{line}$.
  - [x] Pengelompokan glif menjadi kata (`Word`) dengan pemisahan spasi eksplisit dan celah horizontal adaptif.
  - [x] Algoritma Recursive X-Y Cut (RXYC) berbasis profil proyeksi kerapatan 1D ($P_H(y)$ dan $P_V(x)$) untuk memotong tata letak multi-kolom dan menjamin urutan baca alami (*reading order DAG*).
  - [x] Pengklasifikasi Heading (`H1`, `H2`, `H3`) berdasarkan ukuran relatif terhadap median teks isi dan bobot font.
  - [x] Deteksi butir daftar berpeluru (*bullet points*) dan daftar bernomor (*numbered lists*).
  - [x] Konsolidasi kata menjadi `TextRun` kompak pada setiap `TextLine`.
  - [x] Pengujian nyata WebAssembly via `tests/test_wasm_step7.js` sukses 100% pada `sample.pdf` dan dokumen multi-kolom (`multicolumn_sample.pdf`).
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 8: Table Structure Recognition (TSR) & Custom Tab Stops**
  - [x] Deteksi garis horizontal & vertikal dari vektor path PDF, pembersihan noise, dan penggabungan segmen kolinear (toleransi 2.5 pt).
  - [x] Graf persimpangan garis (*planar graph rail intersection*, Anssi Nurminen 2013) untuk merekonstruksi matriks tabel.
  - [x] Perhitungan sel gabungan horizontal (`col_span`) dan vertikal (`row_span`), serta pemindahan paragraf ke dalam sel tabel.
  - [x] Filter pengecualian bingkai batas halaman (*page margin frame*) agar bingkai dekoratif penuh tidak disalahartikan sebagai tabel.
  - [x] Deteksi kolom dan formulir bertitik dua (`:`) otomatis diubah menjadi `<w:tabs>` dan `TextRun` tab stop.
  - [x] Deteksi pemandu titik (*dot leaders* `.....`) pada baris daftar isi yang otomatis diubah menjadi tab rata kanan dengan leader dot.
  - [x] Pengujian nyata WebAssembly via `tests/test_wasm_step8.js` sukses 100% pada `table_form_sample.pdf`.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 9: Generator OOXML (WordprocessingML) & Pengemas ZIP di RAM**
  - [x] Perakitan paket kontainer Open Packaging Conventions (OPC): `[Content_Types].xml`, `_rels/.rels`, `word/_rels/document.xml.rels`, `word/settings.xml`, `word/styles.xml`, `word/numbering.xml`, dan `word/document.xml`.
  - [x] Konversi matematis satuan fisik ISO/IEC 29500-1: Twips ($20\times$), Half-Points ($2\times$), Eighth-Points ($8\times$), EMUs ($12700\times$).
  - [x] Perenderan elemen paragraf `<w:p>`, gaya tipografi `<w:pStyle>`, jarak spasi `<w:spacing>`, dan identasi butir daftar `<w:numPr>`.
  - [x] Perenderan tab formulir & dot leader via `<w:tabs>` dan `<w:tab/>`.
  - [x] Perenderan tabel `<w:tbl>`, grid kolom `<w:tblGrid>`, baris `<w:tr>`, sel `<w:tc>`, dan penggabungan sel `<w:gridSpan>`.
  - [x] Pengemasan kontainer ZIP OPC langsung di RAM via `zip = "2.2"` dengan kompresi Deflate menghasilkan `Vec<u8>` berformat biner Word `.docx` asli (`PK\x03\x04`).
  - [x] Pengujian nyata WebAssembly via `tests/test_wasm_step9.js` sukses 100% pada `sample.pdf` dan `table_form_sample.pdf` menghasilkan `output_sample.docx` dan `output_table_form.docx`.
  - *Status:* **SELESAI & TERVERIFIKASI**.

- [x] **Step 10: Binding WASM, Antarmuka Web, & Pengujian Ujung-ke-Ujung**
  - [x] Ekspor fungsi WebAssembly utama `#[wasm_bindgen] pub fn convert_pdf_to_docx(pdf_data: &[u8]) -> Result<Vec<u8>, JsValue>`.
  - [x] Kompilasi biner rilis WebAssembly `wasm-pack build --target web --out-dir pkg` (100% LLVM `wasm-ld`, zero C++ MSVC linker dependency).
  - [x] Antarmuka web modern (`index.html` dan `test-app.js`) dengan fitur drag-and-drop, telemetri instan (kecepatan, waktu, ukuran RAM), tombol uji sampel 1-klik, dan pengunduh biner `.docx` langsung di browser.
  - [x] Benchmark performa & audit nol ketergantungan JavaScript (`tests/test_wasm_step10.js`): kecepatan rata-rata > 1 MB/s murni client-side di memori browser.
  - *Status:* **SELESAI & TERVERIFIKASI**.
