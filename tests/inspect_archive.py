import zipfile
import re

with zipfile.ZipFile('tests/inspect_latest.docx', 'r') as z:
    for name in z.namelist():
        print(name)
    print('--- header1.xml ---')
    if 'word/header1.xml' in z.namelist():
        print(z.read('word/header1.xml').decode('utf-8'))
    print('--- footer1.xml ---')
    if 'word/footer1.xml' in z.namelist():
        print(z.read('word/footer1.xml').decode('utf-8'))
    
    xml = z.read('word/document.xml').decode('utf-8')
    br_count = xml.count('w:type="page"')
    print(f'Total hard page breaks (<w:br w:type="page"/>): {br_count}')
    
    # Check sectPr in document.xml
    sects = re.findall(r'<w:sectPr.*?</w:sectPr>', xml, re.DOTALL)
    print(f'Total sectPr elements: {len(sects)}')
    for i, s in enumerate(sects):
        print(f'sectPr {i}:')
        print(s)

