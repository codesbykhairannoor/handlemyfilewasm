Arsitektur Rekonstruksi Tata Letak Presisi Tinggi untuk Mesin Konversi PDF ke DOCX Berbasis C++ dan WebAssemblyKonversi dokumen dari format Portable Document Format (PDF) ke format Microsoft Word (WordprocessingML atau DOCX) dengan mempertahankan fidelitas tata letak asli merupakan salah satu tantangan paling rumit dalam rekayasa perangkat lunak dokumen. Kegagalan mayoritas pustaka sumber terbuka di GitHub dalam menyelesaikan masalah ini disebabkan oleh perbedaan mendasar antara kedua format tersebut. PDF dirancang menggunakan paradigma "kertas digital" statis, di mana setiap elemen teks, gambar, dan vektor ditempatkan secara eksplisit menggunakan koordinat dua dimensi ($X, Y$) yang absolut. Sebaliknya, berkas DOCX menggunakan paradigma aliran dinamis (reflowable layout), di mana teks mengalir secara berkelanjutan di dalam paragraf, tabel, dan kolom.Sebagian besar pustaka konverter gratis menyederhanakan masalah ini dengan dua cara ekstrem yang tidak memuaskan: hanya mengekstrak teks mentah tanpa format sehingga merusak seluruh tata letak, atau membungkus setiap fragmen teks ke dalam kotak teks absolut (text boxes) di dalam dokumen Word. Pendekatan kedua memang menghasilkan tampilan visual yang sekilas mirip dengan PDF asli, namun dokumen tersebut menjadi hampir mustahil untuk diedit secara alami karena penambahan satu kata saja akan merusak seluruh keselarasan halaman.Laporan penelitian ini menyajikan cetak biru teknis bagi agen kecerdasan buatan (AI Agent) untuk merekayasa mesin konversi PDF ke Word berkinerja tinggi menggunakan bahasa C++. Mesin ini dirancang untuk dikompilasi menjadi modul WebAssembly (WASM) sehingga dapat diintegrasikan langsung ke dalam ekosistem JavaScript baik pada sisi klien (browser) maupun pada sisi peladen (Node.js) melalui pustaka NPM.Analisis Komparatif Kerangka Kerja dan Pustaka KonversiUntuk memahami batasan teknologi saat ini dan memformulasi solusi C++ yang superior, tim pengembang perlu menganalisis karakteristik teknis beberapa pustaka manipulasi dokumen yang ada di pasar. Evaluasi ini mencakup metode penataan letak, performa kecepatan, dan lingkungan target eksekusi.Kerangka Kerja / PustakaBasis Bahasa & EngineMetode Rekonstruksi LayoutKinerja & LatensiTarget Eksekusi UtamaBatasan Teknis Utamapdf2docx (Python)Python + PyMuPDF + python-docxBerbasis aturan heuristik dinamis (reflowable)Sedang (Terhambat overhead interpreter Python)Lingkungan Desktop / Peladen PythonTidak dapat dijalankan secara asli di browser tanpa runtime Python yang lambat.ComPDF SDK (Komersial)C++ + PDFium + AI Model proprietaryPendekatan hibrida dinamis-absolut berbasis AISangat Tinggi (Suku cadang native berkinerja tinggi)Multi-platform (Cloud, Desktop, Mobile, WASM)Memerlukan lisensi komersial dan model AI yang besar untuk layout kompleks.PDF.js (Mozilla)JavaScript MurniTidak ada (Hanya melakukan rendering visual ke elemen Canvas)Sedang (Terbatas pada thread tunggal JS)Browser WebTidak mendukung ekspor atau rekonstruksi struktur dokumen Word.Mesin WASM C++ (Arsitektur Usulan)C++17 + PDFium + miniz (OOXML Packer)Hibrida Proyeksi Profil Geometris & Cluster DocstrumSangat Tinggi (Kompilasi biner langsung ke WASM)Browser & Node.js (Universal NPM)Memerlukan manajemen memori linier manual yang sangat ketat.Mengapa Memilih PDFium sebagai Basis Parser C++?Ekstraksi konten PDF memerlukan parser tingkat rendah yang stabil dan teruji secara industri. PDFium, mesin parser yang dikembangkan oleh Google untuk peramban Chromium, menawarkan keunggulan mutlak dibandingkan alternatif seperti Poppler atau pdfmm.Pertama, PDFium dirancang dengan fokus pada keamanan kotak pasir (sandbox) dan performa memori yang sangat ketat, yang sangat penting untuk eksekusi WebAssembly di browser. Kedua, lisensi Apache 2.0 milik PDFium sangat permisif untuk integrasi komersial, berbeda dengan Poppler yang berlisensi GPL yang memaksa seluruh kode sumber aplikasi induk dibuka. Ketiga, PDFium mengekspos API yang sangat rinci untuk mengakses informasi batas koordinat karakter (character bounding boxes), gambar tersemat, dan data garis vektor yang menjadi bahan bakar utama bagi algoritma rekonstruksi tata letak.Metodologi Analisis Tata Letak Dokumen (Document Layout Analysis)Kunci untuk mencapai konversi yang "sama persis" tanpa mengorbankan kemampuan edit (editability) dokumen Word adalah dengan mengotomatisasi proses konversi koordinat fisik PDF menjadi struktur semantik Word. Proses ini disebut Document Layout Analysis (DLA). Mesin C++ yang dibangun harus menerapkan kombinasi dua algoritma segmentasi halaman: pendekatan atas-bawah (top-down) untuk membagi area besar, dan pendekatan bawah-atas (bottom-up) untuk merakit kata dan baris teks.                +-----------------------------------------+
                |          PDF Input (PDFium API)         |
                +-----------------------------------------+
                                     |
                                     v
                +-----------------------------------------+
                |      Ekstraksi Koordinat Karakter       |
                +-----------------------------------------+
                                     |
                                     v
                +-----------------------------------------+
                |    Deteksi & Penyaringan Header/Footer  |  <--- Kamus Jarak Levenshtein
                +-----------------------------------------+
                                     |
                                     v
                +-----------------------------------------+
                |     Algoritma Pemotong X-Y Rekursif     |  <--- Segmentasi Blok Besar (Kolom/Tabel)
                +-----------------------------------------+
                                     |
                                     v
                +-----------------------------------------+
                |       Algoritma Kluster Docstrum        |  <--- Penggabungan Karakter ke Baris/Paragraf
                +-----------------------------------------+
                                     |
                                     v
                +-----------------------------------------+
                |     Penyusunan Struktur Hierarki XML    |  <--- Pemetaan ke Struktur OOXML DOCX
                +-----------------------------------------+
Algoritma Pemotong X-Y Rekursif (Recursive X-Y Cut)Algoritma Recursive X-Y Cut (RXYC) digunakan untuk membagi halaman dokumen menjadi wilayah-wilayah persegi panjang besar (seperti kolom, area tabel, atau blok teks utama) dengan menganalisis celah kosong (whitespace) yang memanjang secara horizontal dan vertikal.Untuk mengimplementasikan RXYC pada data spasial vektor, halaman direpresentasikan sebagai kumpulan kotak pembatas (bounding boxes) elemen $E = \{e_1, e_2, \dots, e_n\}$, di mana setiap elemen memiliki batas koordinat $(x_{min}, y_{min}, x_{max}, y_{max})$. Algoritma kemudian memproyeksikan elemen-elemen ini ke sumbu horizontal dan vertikal untuk membuat profil proyeksi kerapatan. Secara matematis, profil proyeksi horizontal $P_H(y)$ dan profil proyeksi vertikal $P_V(x)$ dinyatakan sebagai berikut:$$P_H(y) = \sum_{i=1}^{n} \mathbb{I}(y_{min, i} \le y \le y_{max, i})$$$$P_V(x) = \sum_{i=1}^{n} \mathbb{I}(x_{min, i} \le x \le x_{max, i})$$Di mana $\mathbb{I}(C)$ adalah fungsi indikator bernilai $1$ jika kondisi $C$ terpenuhi, dan $0$ jika tidak. Langkah-langkah eksekusi algoritma oleh mesin C++ adalah:Analisis Profil Horizontal: Hitung $P_H(y)$ di sepanjang sumbu $Y$ dari halaman. Cari celah kosong kontinu (wilayah di mana $P_H(y) = 0$) yang tingginya melebihi ambang batas spasi baris $T_Y$. Jika ditemukan, bagi dokumen secara horizontal pada titik tengah celah tersebut menjadi beberapa node anak.Analisis Profil Vertikal: Untuk setiap node anak hasil pembagian horizontal, hitung $P_V(x)$ di sepanjang sumbu $X$. Cari celah vertikal kontinu (di mana $P_V(x) = 0$) yang lebarnya melebihi ambang batas kolom $T_X$. Jika ditemukan, potong wilayah tersebut secara vertikal untuk memisahkan kolom.Rekursi: Lakukan proses di atas secara berulang pada setiap sub-persegi panjang yang dihasilkan hingga tidak ada lagi celah kosong yang melebihi ambang batas $T_X$ dan $T_Y$. Hasil akhir dari rekursi ini adalah sebuah struktur pohon spasial (spatial tree) yang mengelompokkan elemen halaman ke dalam tata letak hierarkis yang logis.Algoritma Kluster Docstrum untuk Penanganan Layout KompleksPada dokumen yang memiliki sudut kemiringan (skew) atau tata letak non-standar, proyeksi lurus RXYC sering kali gagal memisahkan elemen dengan bersih. Di sinilah algoritma bawah-atas Docstrum diterapkan untuk mengelompokkan karakter individu menjadi kata, baris, dan paragraf berdasarkan hubungan ketetanggaan spasial terdekat.Tahapan implementasi algoritma Docstrum pada mesin C++ adalah sebagai berikut:Representasi Komponen: Setiap karakter hasil ekstraksi PDFium dianggap sebagai satu komponen terhubung (connected component) yang memiliki titik pusat koordinat $(x_c, y_c)$.Pencarian Tetangga Terdekat: Untuk setiap karakter $c_i$, hitung jarak Euclidean dan sudut orientasi terhadap $k$ tetangga terdekatnya (nilai $k = 5$ sangat direkomendasikan untuk menyeimbangkan kecepatan komputasi dan akurasi pengelompokan).Analisis Histogram Jarak: Plot seluruh jarak tetangga terdekat ke dalam histogram untuk mengidentifikasi puncak frekuensi secara otomatis. Puncak pertama menunjukkan jarak rata-rata antar-karakter dalam satu kata, puncak kedua menunjukkan jarak antar-kata (word spacing), dan puncak ketiga menunjukkan spasi antar-baris (line spacing).Pembentukan Baris dan Blok: Hubungkan dua karakter tetangga jika jaraknya berada dalam ambang batas jarak antar-karakter dan sudut orientasinya mendekati horizontal. Setelah baris teks terbentuk, kelompokkan baris-baris tersebut menjadi satu blok paragraf jika jarak vertikal antar-baris konsisten dengan nilai spasi baris dari histogram.Penyaringan Dinamis Header dan Footer Menggunakan Jarak LevenshteinAgar dokumen Word hasil konversi bersih dari elemen berulang yang merusak alur pembacaan, mesin harus mendeteksi dan menghapus tajuk atas (header) dan tajuk bawah (footer) secara otomatis sebelum melakukan konstruksi paragraf. Deteksi ini dilakukan dengan membandingkan baris teks pada posisi koordinat serupa di berbagai halaman menggunakan algoritma Levenshtein Distance.Dua baris string $S_i^0$ (pada halaman pertama) dan $S_i^k$ (pada halaman $k$ dengan koordinat vertikal $Y$ yang sama) diidentifikasi sebagai elemen berulang jika memenuhi kriteria berikut:$$\text{Levenshtein}(S_i^0, S_i^k) \le 5$$Batas toleransi kesalahan pengetikan atau perbedaan numerik sebesar $5$ diterapkan agar sistem tetap dapat mengenali tajuk atas yang memiliki sedikit variasi, seperti nomor halaman atau sub-judul bab yang dinamis (misalnya, membandingkan "Bab 1: Pendahuluan" dan "Bab 2: Pembahasan" yang memiliki kesamaan tata letak). Elemen yang terdeteksi sebagai tajuk ini akan dilepaskan dari struktur teks utama dan dipetakan ke dalam elemen header/footer asli pada berkas DOCX.Pengenalan Struktur Tabel dan Sel Tergabung (Merged Cells)Tabel di dalam PDF sering kali direpresentasikan sebagai garis-garis vektor terpisah tanpa penanda struktural. Untuk memulihkan tabel dengan presisi tinggi ke format Word, mesin C++ harus menjalankan algoritma pengenalan tabel berbasis aturan geometris berikut:Ekstraksi Garis Vektor: Gunakan API FPDFPage_GetAnnot atau fungsi ekstraksi gambar vektor page.get_drawings() untuk mendapatkan semua garis horizontal dan vertikal.Deteksi Grid Interseksi: Cari titik potong antara garis horizontal dan vertikal untuk membentuk matriks koordinat sel ($X_{grid}, Y_{grid}$).Klasifikasi Sel: Jika suatu area persegi di dalam grid dikelilingi oleh batas garis vektor, tandai area tersebut sebagai sel tabel.Deteksi Sel Tergabung (Merged Cells): Bandingkan ukuran sel geometris dengan koordinat grid standar. Jika sebuah sel menempati lebih dari satu kolom atau baris pada matriks grid, tandai sel tersebut dengan atribut penggabungan kolom (w:gridSpan) atau penggabungan baris (w:vMerge) pada kode XML WordprocessingML.Pemetaan Geometris PDF ke Struktur OOXML (DOCX)Setelah seluruh tata letak dokumen berhasil dianalisis dan direkonstruksi menjadi representasi logis, mesin C++ harus merakit dokumen Word sesuai dengan standar Office Open XML (OOXML).Kompresi data XML ini dilakukan secara dinamis langsung di dalam memori menggunakan pustaka miniz, sebuah implementasi zlib/deflate berbasis C yang sangat ringan dan tidak memiliki dependensi eksternal, sehingga sangat aman dan cepat ketika dijalankan di dalam WebAssembly.Pemetaan data spasial PDF ke elemen terstruktur WordprocessingML diatur berdasarkan spesifikasi pemetaan geometris berikut:Elemen PDFKarakteristik Spasial PDFElemen XML Target (DOCX)Atribut & Properti Kunci XMLTujuan Fungsional RekonstruksiParagraf[cite: 23]Jarak vertikal baris teks seragam, berada dalam satu batas wilayah kolom.<w:p>[cite: 23]<w:spacing w:before="..." w:after="..."/>, <w:ind w:left="..."/>[cite: 3]Mengalirkan teks secara alami; memberikan spasi paragraf dinamis alih-alih menggunakan enter kosong.Format Teks[cite: 27]Karakter dengan jenis font, warna, dan ukuran yang sama di dalam satu garis.<w:r>[cite: 23]<w:rPr>, <w:rFonts w:ascii="..." />, <w:sz w:val="..." />[cite: 23]Mengelompokkan fragmen kata yang seragam ke dalam satu run teks untuk menjaga efisiensi berkas.Gaya Huruf[cite: 27]Ketebalan goresan vektor atau metadata font yang menunjukkan ketebalan (bold) atau kemiringan (italic).<w:rPr>[cite: 23]<w:b/> (Tebal), <w:i/> (Miring), <w:color w:val="..." /> (Warna)Menerapkan dekorasi teks asli tanpa merusak pembacaan karakter.Tabel[cite: 24]Elemen teks yang terkurung di dalam garis-garis batas vektor horizontal dan vertikal.<w:tbl>[cite: 23]<w:tblGrid>, <w:tblPr>, <w:tblBorders>[cite: 26]Membangun tabel terstruktur di Word yang dapat disesuaikan ukurannya, bukan berupa gambar mati.Baris Tabel[cite: 24]Kumpulan sel yang sejajar secara horizontal pada sumbu $Y$ yang sama.<w:tr>[cite: 26]<w:trHeight w:val="..." />[cite: 26]Menentukan tinggi baris tabel dinamis berdasarkan isi sel.Sel Tabel[cite: 24]Kotak pembatas sel individu hasil perpotongan grid geometris.<w:tc>[cite: 26]<w:tcW w:w="..." />, <w:gridSpan w:val="..." />, <w:vMerge w:val="..." />[cite: 2, 26]Menampung teks di dalam koordinat batas sel; mendukung kolom/baris tergabung (colspan/rowspan).Gambar[cite: 27]Objek biner eksternal atau biner aliran gambar terkompresi (DCT/PNG) di PDF.<w:drawing>[cite: 26]<wp:anchor>, <wp:extent cx="..." cy="..." />[cite: 26]Menempatkan ilustrasi secara sejajar (inline) atau mengambang (floating) relatif terhadap paragraf terdekat.Panduan Porting C++ ke WebAssembly Menggunakan EmscriptenRantai alat (toolchain) Emscripten mengubah kode sumber C++ menjadi instruksi biner WebAssembly serta menyediakan berkas glue code JavaScript untuk berinteraksi dengan modul tersebut. Namun, kompilasi pustaka C++ berat seperti PDFium memerlukan beberapa modifikasi penting pada tingkat kode dan build sistem untuk memastikan kompatibilitas penuh di lingkungan web.Isolasi Ketergantungan Sistem OperasiEmscripten bertindak sebagai kompiler silang (cross-compiler) yang menargetkan lingkungan kotak pasir peramban web. Oleh karena itu, semua pemanggilan API yang bergantung pada fitur grafis atau font sistem operasi bawaan hos (seperti Win32 GDI/GDI+ pada Windows, Cocoa pada macOS, atau X11/Fontconfig pada Linux) harus diisolasi dan dinonaktifkan.Sistem grafis harus dikonfigurasi untuk menggunakan mesin rendering perangkat lunak murni (software-only rasterizer). Hal ini dapat dicapai dengan mengonfigurasi PDFium agar menggunakan pustaka grafis internalnya (seperti Skia atau AGG) tanpa menyentuh pustaka GUI hos. Semua makro deteksi sistem operasi pada build script harus dinonaktifkan secara eksplisit.Penanganan Font Metrics Melalui Virtual Filesystem (MEMFS)Tantangan utama dalam melakukan konversi PDF ke DOCX yang akurat di browser adalah ketersediaan berkas font untuk menghitung lebar karakter secara presisi (font metrics). Jika font standar (seperti Arial, Times New Roman, atau Calibri) tidak tersedia saat menghitung ukuran teks, penghitungan posisi pembatas paragraf akan meleset, menyebabkan baris teks terpotong secara acak di Word.Untuk mengatasi hal ini, gunakan teknologi sistem berkas virtual Emscripten (MEMFS). Pustaka TTF standar yang mewakili font-font utama harus dikemas ke dalam biner WebAssembly pada saat kompilasi menggunakan parameter --preload-file. Berkas-berkas font ini kemudian akan dimuat ke dalam memori RAM virtual saat modul diinisialisasi. PDFium harus diarahkan untuk membaca pustaka font dari direktori virtual tersebut (misalnya /fonts/Arial.ttf) menggunakan fungsi konfigurasi font kustom pada inisialisasi awal:C++// Mengonfigurasi PDFium untuk memuat font dari sistem berkas virtual WASM
FPDF_InitLibraryWithConfig(...);
Manajemen Memori Linier WASM dan Pencegahan Kebocoran MemoriWebAssembly mengeksekusi instruksi di dalam ruang memori linier yang terisolasi dari memori utama JavaScript. Karena runtime C++ menggunakan alokasi memori manual (malloc dan free), kegagalan pembebasan memori biner PDF atau dokumen Word hasil konversi akan langsung menyebabkan penumpukan konsumsi memori (memory leaks) yang dapat membuat tab browser mogok (crash).Aturan utama dalam pertukaran memori antara JS dan C++ adalah:Setiap kali JavaScript ingin mengirim berkas PDF ke modul C++, ia harus mengalokasikan buffer memori pada heap WASM terlebih dahulu, menyalin datanya ke buffer tersebut, dan mengirimkan alamat pointer-nya ke C++.Setelah C++ menyelesaikan proses konversi dan menghasilkan biner Word (DOCX), C++ harus mengembalikan alamat pointer biner DOCX beserta ukurannya ke JavaScript.JavaScript harus segera menyalin biner DOCX tersebut ke memori lokal JS (ArrayBuffer).Terakhir, JavaScript wajib memanggil fungsi pembebasan memori C++ (free) untuk menghapus buffer PDF input dan buffer DOCX output di dalam heap WASM.Implementasi Kode dan Rancangan Jembatan NPMBagian ini menyajikan arsitektur kode konkret bagi AI Agent untuk menulis berkas jembatan integrasi antara C++ dan JavaScript.Antarmuka Ekspor C++ (pdf_converter_bridge.cpp)Gunakan fungsi eksternal "C" dan makro EMSCRIPTEN_KEEPALIVE untuk mencegah kompiler menghapus fungsi-fungsi ini selama proses optimasi build (dead code elimination).C++#include <emscripten.h>
#include <cstdlib>
#include <cstring>
#include <vector>
#include "fpdfview.h"
#include "LayoutAnalyzer.h" // Kelas penganalisis geometris buatan pengembang
#include "DocxGenerator.h"  // Kelas pembuat XML OpenXML buatan pengembang

extern "C" {

// Alokasi memori heap WASM yang dipanggil dari JavaScript
EMSCRIPTEN_KEEPALIVE
void* allocate_wasm_buffer(size_t size) {
    return std::malloc(size);
}

// Pembebasan memori heap WASM yang dipanggil dari JavaScript
EMSCRIPTEN_KEEPALIVE
void free_wasm_buffer(void* ptr) {
    std::free(ptr);
}

// Menyimpan ukuran biner DOCX yang dihasilkan
size_t global_output_docx_size = 0;

EMSCRIPTEN_KEEPALIVE
size_t get_converted_docx_size() {
    return global_output_docx_size;
}

// Fungsi utama konversi dokumen
EMSCRIPTEN_KEEPALIVE
uint8_t* convert_pdf_to_docx(uint8_t* pdf_buffer, size_t pdf_size) {
    // 1. Inisialisasi pustaka PDFium
    FPDF_InitLibrary();
    
    // 2. Muat dokumen PDF dari pointer memori linear WASM
    FPDF_DOCUMENT pdf_doc = FPDF_LoadMemDocument(pdf_buffer, static_cast<int>(pdf_size), nullptr);
    if (!pdf_doc) {
        global_output_docx_size = 0;
        return nullptr;
    }
    
    // 3. Eksekusi Analisis Geometris & Tata Letak Dokumen (DLA)
    LayoutAnalyzer analyzer;
    analyzer.AnalyzePages(pdf_doc);
    
    // 4. Konstruksi berkas DOCX terkompresi ZIP di dalam memori buffer C++
    DocxGenerator generator;
    std::vector<uint8_t> docx_zip_bytes = generator.BuildZipArchive(analyzer.GetExtractedLayout());
    
    global_output_docx_size = docx_zip_bytes.size();
    if (global_output_docx_size == 0) {
        FPDF_CloseDocument(pdf_doc);
        FPDF_DestroyLibrary();
        return nullptr;
    }
    
    // 5. Alokasikan buffer keluaran di heap WASM untuk dikonsumsi JavaScript
    uint8_t* out_buffer = static_cast<uint8_t*>(std::malloc(global_output_docx_size));
    std::memcpy(out_buffer, docx_zip_bytes.data(), global_output_docx_size);
    
    // 6. Bersihkan memori dokumen PDFium
    FPDF_CloseDocument(pdf_doc);
    FPDF_DestroyLibrary();
    
    return out_buffer;
}

}
Wrapper JavaScript / NPM (index.js)Wrapper JavaScript ini membungkus interaksi tingkat rendah dengan modul WebAssembly ke dalam sebuah API berbasis Promise yang bersih dan modern.JavaScriptimport ModuleInitializer from './pdf_converter_wasm.js';

class HighPrecisionPdfConverter {
    constructor() {
        this.wasmModule = null;
    }

    async initialize() {
        if (this.wasmModule) return;
        // Inisialisasi modul WebAssembly
        this.wasmModule = await ModuleInitializer();
    }

    async convertPdfToDocx(pdfUint8Array) {
        await this.initialize();

        const pdfSize = pdfUint8Array.length;
        let pdfPointer = null;
        let docxPointer = null;

        try {
            // 1. Alokasikan buffer pada heap WASM untuk berkas PDF input
            pdfPointer = this.wasmModule._allocate_wasm_buffer(pdfSize);
            if (!pdfPointer) {
                throw new Error("Gagal mengalokasikan memori linier WASM untuk PDF input.");
            }

            // 2. Salin data biner PDF dari JS ke dalam heap WASM
            this.wasmModule.HEAPU8.set(pdfUint8Array, pdfPointer);

            // 3. Jalankan fungsi konversi C++
            docxPointer = this.wasmModule._convert_pdf_to_docx(pdfPointer, pdfSize);
            
            const docxSize = this.wasmModule._get_converted_docx_size();
            if (!docxPointer || docxSize === 0) {
                throw new Error("Proses analisis dan rekonstruksi dokumen gagal di sisi WebAssembly.");
            }

            // 4. Salin biner DOCX hasil konversi ke dalam ArrayBuffer JS lokal
            const docxResult = new Uint8Array(docxSize);
            docxResult.set(
                this.wasmModule.HEAPU8.subarray(docxPointer, docxPointer + docxSize)
            );

            return docxResult.buffer;

        } catch (error) {
            console.error("Kesalahan Konversi PDF-ke-Word:", error.message);
            throw error;
        } finally {
            // 5. Bebaskan kembali memori heap WASM secara manual untuk menghindari kebocoran memori
            if (pdfPointer) {
                this.wasmModule._free_wasm_buffer(pdfPointer);
            }
            if (docxPointer) {
                this.wasmModule._free_wasm_buffer(docxPointer);
            }
        }
    }
}

export { HighPrecisionPdfConverter };
Cetak Biru Integrasi AI Agent & Alur Kerja ImplementasiUntuk merekayasa mesin konversi bertenaga tinggi ini secara efisien, agen kecerdasan buatan (AI Agent) harus mengikuti alur kerja terstruktur yang dibagi menjadi beberapa fase pengodean dan validasi berikut:Fase 1: Setup Proyek dan Pustaka Pihak KetigaInisialisasi Repositori: Buat struktur proyek standar C++ dengan memisahkan folder /src (untuk kode pengolah layout), /third_party (untuk menampung biner statis PDFium dan header miniz), dan /wasm (untuk berkas pembungkus WASM).Integrasi PDFium: Unduh versi pra-kompilasi biner statis PDFium yang kompatibel dengan WebAssembly/Emscripten untuk menghindari kerumitan kompilasi silang PDFium dari awal. Letakkan berkas pustaka (libpdfium.a) di bawah direktori /third_party/pdfium/lib.Integrasi miniz: Salin berkas tunggal miniz.c dan miniz.h langsung ke dalam folder dependensi eksternal proyek untuk menangani operasi pengemasan berkas ZIP biner secara efisien.Fase 2: Pengembangan Algoritma DLA C++Kelas LayoutAnalyzer: Bangun logika pembacaan halaman PDF menggunakan antarmuka C++ PDFium.Kompilasi Fungsi Heuristik: Tulis implementasi fungsi matematika untuk algoritma Recursive X-Y Cut menggunakan array dinamis C++ std::vector untuk menampung kotak pembatas elemen.Penyusunan Logika Hubungan Spasial: Implementasikan perhitungan jarak Euclidean antarkarakter untuk melengkapi algoritma Docstrum sebagai filter sekunder untuk menyatukan fragmen teks menjadi satu untaian kalimat.Fase 3: Rekayasa Generator OpenXML (DOCX)Penyusun Struktur XML: Implementasikan modul pembangun XML dinamis menggunakan kelas std::stringstream untuk memproduksi teks XML yang valid tanpa dependensi pustaka berat di luar pustaka standar C++.Integrasi Struktur ZIP: Gunakan modul miniz untuk membuat arsip kompresi dalam memori. Struktur folder dalam memori harus sepenuhnya mematuhi standar Open Packaging Conventions (OPC) untuk berkas Word (DOCX).Fase 4: Skrip Kompilasi EmscriptenTulis konfigurasi berkas CMakeLists.txt untuk mengotomatisasi kompilasi menggunakan rantai alat Emscripten. Pastikan opsi optimasi diatur ke mode rilis maksimum (-O3) untuk performa tinggi, mengaktifkan fitur ekspor memori runtime, dan menambahkan berkas font TrueType ke dalam sistem berkas virtual WASM.Fase 5: Validasi Pengujian dan Penerbitan NPMPengujian Unit Otomatis: Buat skrip pengujian berbasis Node.js yang memuat modul WebAssembly hasil kompilasi dan memproses berkas PDF uji coba.Pengecekan Struktur Hasil: Gunakan skrip pengujian untuk memverifikasi apakah berkas .docx yang dihasilkan dapat diekstrak secara struktural dan tidak mengandung kesalahan XML di Microsoft Word.Distribusi NPM: Setelah lolos uji stabilitas memori, bungkus modul WASM dan kelas JS pembungkus ke dalam paket NPM untuk dipublikasikan secara luas sehingga siap digunakan oleh pengembang JavaScript di seluruh dunia.