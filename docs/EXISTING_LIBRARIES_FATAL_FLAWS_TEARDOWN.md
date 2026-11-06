# 💥 Teardown: Analisis Kritis & Cacat Fatal Arsitektur Pustaka PDF-to-DOCX (Python, NPM, dan Native C++)

Dokumen ini merupakan bedah rekayasa perangkat lunak (*software engineering teardown*) terhadap pustaka-pustaka populer di ekosistem **Python**, **NPM (Node.js)**, dan **Native C++** yang mengklaim mampu melakukan konversi PDF ke DOCX. 

Tujuan dokumen ini adalah mengungkap **kelemahan arsitektur fatal (*fatal architectural flaws*)** mereka, sehingga mesin **Pure Rust** yang kita bangun tidak mengulangi kesalahan desain yang sama.

---

## 1. EKOSISTEM PYTHON: `pdf2docx` (Artifex / PyMuPDF + python-docx)

`pdf2docx` adalah pustaka sumber terbuka paling populer di Python untuk tugas ini. Namun, di balik popularitasnya, arsitekturnya memiliki kelemahan struktural mendasar:

### Cacat Fatal 1: Heuristik Tabel yang Rapuh (*Lattice vs Stream Fragility*)
* **Mekanisme**: `pdf2docx` membagi deteksi tabel menjadi dua: *Lattice* (tabel dengan garis batas vektor) dan *Stream* (tabel tanpa garis batas berdasarkan perataan teks).
* **Cacat Fatal**:
  * Algoritma *Lattice* sering mengalami *False Positive*. Daftar isi, kode program, atau teks dua kolom yang memiliki garis pemisah tipis sering keliru diidentifikasi sebagai tabel.
  * Ketika terjadi kesalahan identifikasi, mesin sering melempar *crash* fatal (`making page error`, `ZeroDivisionError`, atau kalkulasi sel yang rusak).
  * Pada tabel bersarang (*nested tables*), kolom bayangan (*phantom columns*) sering tercipta dan teks di dalam sel terduplikasi.
* **Bukti Nyata**: Pengembang sering kali terpaksa menambahkan kode darurat:
  ```python
  cv.convert("output.docx", parse_lattice_table=False) # Matikan deteksi tabel jika crash!
  ```

### Cacat Fatal 2: Kerusakan Urutan Baca pada Elemen Mengambang (*Floating Callouts*)
* **Mekanisme**: PDF sering meletakkan kotak catatan samping (*sidebars*), kotak kutipan (*callouts*), atau stempel pada koordinat bebas.
* **Cacat Fatal**: `pdf2docx` tidak memetakan elemen ini ke `<wp:anchor>` (gambar/bingkai mengambang Word). Sebagai gantinya, mereka dipaksa masuk ke dalam aliran teks reguler (`<w:p>`). Akibatnya, kalimat di kotak catatan samping sering kali **disuntikkan tiba-tiba di tengah-tengah kalimat paragraf utama**.

### Cacat Fatal 3: Masalah "Kata Terikat" (*Glued Words Bug*)
* **Mekanisme**: Pada PDF yang dihasilkan oleh mesin peramban modern (Google Chrome Skia PDF Printer) atau LaTeX, karakter spasi sering kali tidak dituliskan sebagai spasi ASCII (`0x20`), melainkan menggunakan spasi lebar nol (*Zero-Width Space* `U+200B`) atau pergeseran posisi matriks `TJ`.
* **Cacat Fatal**: `pdf2docx` gagal menghitung jarak metrik font ini secara akurat, sehingga seluruh kata dalam satu kalimat menyatu tanpa spasi: `IniAdalahContohTeksYangMenyatu`.

### Cacat Fatal 4: *Memory Bloat* & Hambatan Python GIL
* **Mekanisme**: Dibangun di atas runtime Python dinamis yang memuat representasi pohon objek halaman secara penuh ke dalam RAM.
* **Cacat Fatal**: Mengonversi dokumen 100 halaman membutuhkan RAM 300–800 MB dan memakan waktu 30–90 detik karena terhambat *Global Interpreter Lock* (GIL) dan *garbage collector*. Tidak mungkin dikompilasi menjadi biner WebAssembly yang ringan untuk browser (Pyodide membutuhkan runtime 35+ MB).

---

## 2. EKOSISTEM NPM / NODE.JS: "Skandal" Pembungkus LibreOffice

Sebagian besar paket di NPM yang mengklaim dapat mengonversi PDF ke DOCX (seperti `file-converter-nodejs`, `pdf-to-docx`, dll.) ternyata menyembunyikan arsitektur palsu:

### Cacat Fatal 1: Menjalankan Sub-Proses LibreOffice Headless (`soffice --headless`)
* **Mekanisme**: Paket NPM tersebut sebenarnya tidak memiliki algoritma konversi sama sekali! Mereka hanya memanggil sub-proses sistem operasi:
  ```javascript
  child_process.spawn('soffice', ['--headless', '--convert-to', 'docx', 'input.pdf']);
  ```
* **Cacat Fatal**:
  1. **Dependensi Raksasa**: Peladen (*server*) wajib memasang instalasi LibreOffice sebesar 500 MB – 1 GB.
  2. **Kerentanan Konkurensi**: Menjalankan LibreOffice dalam mode *headless* di server Node.js sering menyebabkan *process hang*, *memory leak*, dan *deadlock* saat ribuan *request* masuk bersamaan.
  3. **Latensi Tinggi**: Waktu booting proses LibreOffice memakan waktu 3–8 detik per dokumen.
  4. **Nol Kompatibilitas Browser**: Mustahil dijalankan di sisi klien (*browser client-side*).

### Cacat Fatal 2: "Jebakan Kotak Teks" LibreOffice Draw (*The Text Frame Disaster*)
Ini adalah kelemahan paling mengerikan dari LibreOffice:
* Ketika LibreOffice membuka PDF, modul yang digunakan sebenarnya adalah **LibreOffice Draw** (`writer_pdf_import`), bukan LibreOffice Writer!
* Karena Draw tidak memahami teks mengalir, **setiap baris teks dibungkus ke dalam kotak teks absolut (*text box / frame*) terpisah**:
  ```xml
  <w:p>
    <w:pPr><w:framePr w:x="1440" w:y="2880" w:w="4000" w:h="300"/></w:pPr>
    <w:r><w:t>Baris 1 dari PDF</w:t></w:r>
  </w:p>
  ```
* **Dampaknya pada Pengguna**:
  Secara visual dokumen terlihat mirip, tetapi **DOKUMEN TERSEBUT TIDAK BISA DIEDIT**.
  Jika pengguna menambahkan 1 kata saja di baris 1, teks tidak akan mengalir ke baris 2, melainkan meluap keluar kotak atau menabrak kotak teks di bawahnya!

### Cacat Fatal 3: Paket Pure JS Naif (`pdfjs-dist` + `docx`)
Paket-paket eksperimen yang mencoba menulis konverter murni di JS biasanya hanya melakukan:
```javascript
for (const item of textContent.items) {
  doc.addParagraph(new Paragraph(item.str)); // 1 baris PDF = 1 paragraf Word!
}
```
* **Dampaknya**: Seluruh paragraf hancur dengan *hard-break* di setiap akhir baris fisik, tidak ada tabel, tidak ada format kolom, dan tata letak berantakan total.

---

## 3. PUSTAKA NATIVE C++ KOMERSIAL: Adobe, Apryse, ABBYY, Foxit

Pustaka enterprise seperti Adobe PDF Services, Apryse (PDFTron), dan ABBYY FineReader adalah tolok ukur industri dengan akurasi sangat tinggi. Namun, arsitektur mereka memiliki batasan komersial dan distribusi:

| Pustaka / SDK | Kelebihan Arsitektur | Kelemahan Arsitektur Fatal bagi Pengembang Independen |
| :--- | :--- | :--- |
| **Adobe PDF Services API** | Akurasi rekonstruksi terbaik di dunia karena pencipta format PDF. Menggunakan analisis layout berbasis model AI proprietary. | **Cloud-Only & Sangat Mahal**. Biaya per halaman ($0.05 per transaksi). Data PDF pengguna wajib dikirim ke server Adobe (masalah privasi). |
| **Apryse (PDFTron SDK)** | Mesin C++ native sangat cepat, mendukung *Gutter-Based multi-alignment projection*. | **Ukuran Biner Raksasa & Lisensi Tertutup**. Biner SDK berukuran 30–80 MB, biaya lisensi puluhan ribu dolar per tahun, tidak open-source. |
| **ABBYY FineReader Engine** | Integrasi OCR dan DLA paling matang di dunia akademis. | Memerlukan instalasi runtime OS yang berat dan dongle lisensi perangkat keras / cloud token yang kaku. |

---

## 4. TABEL KOMPARATIF CACAT ARSITEKTUR

| Fitur / Karakteristik | `pdf2docx` (Python) | NPM Wrapper (LibreOffice) | Paket Pure JS Naif | Mesin Pure Rust (Rancangan Kita) |
| :--- | :--- | :--- | :--- | :--- |
| **Metode Rekonstruksi** | Heuristik Garis & Spasi | Kotak Teks Statis (*Draw Frames*) | Ekstraksi Teks Mentah | **Reflowable DLA + Tagged Tree Direct Mapping** |
| **Kemampuan Edit Word** | Sedang (Tabel sering rusak) | **NOL (Hancur total jika diedit)** | Buruk (*Hard break* di tiap baris) | **Tinggi (Paragraf & Tabel mengalir alami)** |
| **Konsumsi Memori** | Sangat Tinggi (300–800 MB) | Raksasa (500 MB–1 GB) | Sedang | **Sangat Ringan (~15–30 MB)** |
| **Kecepatan Konversi** | Lambat (GIL Python) | Sangat Lambat (Spawn Process) | Cepat tapi rusak | **Ultra Cepat (Multi-threaded & Zero-Copy)** |
| **Dukungan Tagged PDF** | ❌ Diabaikan | ❌ Diabaikan | ❌ Diabaikan | **✅ Jalur Utama (Akurasi 100%)** |
| **Tab Stop vs Spasi** | ❌ Pakai spasi buatan | ❌ Pakai koordinat frame | ❌ Teks acak | **✅ `<w:tabs>` matematis presisi** |
| **Target Eksekusi** | Hanya Desktop/Server Python | Hanya Server Linux/Windows | Browser & Node | **Universal: CLI, Server Docker, & Browser WASM** |

---

## 5. KESIMPULAN REKAYASA UNTUK MESIN RUST KITA

Dari kegagalan dan kelemahan fatal pustaka-pustaka di atas, kita menetapkan **7 Prinsip Emas (*The 7 Golden Rules*)** untuk arsitektur Rust kita:

1. **Anti Text Box**: Dilarang membungkus teks biasa ke dalam `<w:framePr>` atau text box absolut. Seluruh teks harus mengalir di `<w:p>`.
2. **Anti Space Padding**: Dilarang meratakan kolom atau formulir dengan spasi buatan (`"   "`). Wajib menggunakan `<w:tabs>`.
3. **Tagged-First Priority**: Jika PDF memiliki `/StructTreeRoot`, gunakan pohon semantik itu langsung. Jangan buang waktu menebak dengan heuristik visual jika tag semantiknya sudah ada!
4. **Graph-Theoretic TSR**: Tabel dibangun dari graf persimpangan segmen vektor (*planar graph*), bukan tebakan *bounding box* kasar.
5. **Soft-Break vs Hard-Break Separation**: Baris teks di dalam satu paragraf harus digabung menjadi satu aliran teks mengalir, bukan dipecah dengan tag `<w:br/>` di setiap baris fisik PDF.
6. **Zero External Runtime**: Mesin harus mandiri (*self-contained*), tanpa dependensi LibreOffice, tanpa Python, dan tanpa browser canvas mockup.
7. **Zero-Copy & Memory Safety**: Memproses byte biner PDF langsung di memori dengan kepemilikan ketat Rust tanpa risiko *memory leak* atau *segmentation fault*.

