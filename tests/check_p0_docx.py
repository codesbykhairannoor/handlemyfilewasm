import zipfile, re

with zipfile.ZipFile('tests/output_verified.docx') as z:
    xml = z.read('word/document.xml').decode('utf-8')
    paras = re.findall(r'<w:p[ >].*?</w:p>', xml, re.DOTALL)
    print(f'Total paragraphs in docx: {len(paras)}')
