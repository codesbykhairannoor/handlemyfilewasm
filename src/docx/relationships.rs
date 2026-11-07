pub struct DocxRelationships;

impl DocxRelationships {
    /// Generates [Content_Types].xml conditionally based on header/footer existence
    pub fn content_types(has_header: bool, has_footer: bool) -> String {
        let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
  <Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
"#);
        if has_header {
            xml.push_str(r#"  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
"#);
        }
        if has_footer {
            xml.push_str(r#"  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
"#);
        }
        xml.push_str("</Types>");
        xml
    }

    /// Generates _rels/.rels
    pub fn package_relationships() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#.to_string()
    }

    /// Generates word/_rels/document.xml.rels with image, header, and footer relationships
    pub fn document_relationships(image_rels: &[(usize, &str)], has_header: bool, has_footer: bool) -> String {
        let mut rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>
"#);
        if has_header {
            rels.push_str(r#"  <Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
"#);
        }
        if has_footer {
            rels.push_str(r#"  <Relationship Id="rIdFooter1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
"#);
        }
        for &(img_id, ext) in image_rels {
            rels.push_str(&format!(
                r#"  <Relationship Id="rIdImg{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image{}.{}"/>
"#,
                img_id, img_id, ext
            ));
        }
        rels.push_str("</Relationships>");
        rels
    }

    /// Generates word/header1.xml preserving document running header
    pub fn header(header_p: Option<&crate::models::ParagraphBlock>, printable_width_twips: u32) -> String {
        let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
"#);
        if let Some(p) = header_p {
            let mut left_text = String::new();
            let mut right_text = String::new();
            let mut saw_tab = false;

            for line in &p.lines {
                for run in &line.runs {
                    if run.is_tab {
                        saw_tab = true;
                    } else if !run.text.is_empty() {
                        if saw_tab {
                            if !right_text.is_empty() {
                                right_text.push(' ');
                            }
                            right_text.push_str(&run.text);
                        } else {
                            if !left_text.is_empty() {
                                left_text.push(' ');
                            }
                            left_text.push_str(&run.text);
                        }
                    }
                }
            }

            if saw_tab && !right_text.is_empty() {
                // Dual header (left date + right title) via native Word right tab stop
                xml.push_str(&format!(
                    r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Header"/>
      <w:tabs>
        <w:tab w:val="right" w:leader="none" w:pos="{}"/>
      </w:tabs>
      <w:jc w:val="left"/>
    </w:pPr>
    <w:r>
      <w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr>
      <w:t xml:space="preserve">{}</w:t>
    </w:r>
    <w:r><w:tab/></w:r>
    <w:r>
      <w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr>
      <w:t xml:space="preserve">{}</w:t>
    </w:r>
  </w:p>
"#,
                    printable_width_twips,
                    crate::utils::escape_xml(&left_text),
                    crate::utils::escape_xml(&right_text)
                ));
            } else {
                let is_right = p.alignment == "right" || p.bbox.x_min >= 300.0;
                let jc = if is_right { "right" } else { "left" };
                let text = if !left_text.is_empty() { left_text } else { right_text };
                xml.push_str(&format!(
                    r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Header"/>
      <w:jc w:val="{}"/>
    </w:pPr>
    <w:r><w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r>
  </w:p>
"#,
                    jc,
                    crate::utils::escape_xml(&text)
                ));
            }
        }
        xml.push_str("</w:hdr>");
        xml
    }

    /// Generates word/footer1.xml matching original footer alignment and content
    pub fn footer(footer_p: Option<&crate::models::ParagraphBlock>, printable_width_twips: u32) -> String {
        let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
"#);
        if let Some(p) = footer_p {
            let mut left_text = String::new();
            let mut right_text = String::new();
            let mut saw_tab = false;

            for line in &p.lines {
                for run in &line.runs {
                    if run.is_tab {
                        saw_tab = true;
                    } else if !run.text.is_empty() {
                        if saw_tab {
                            if !right_text.is_empty() {
                                right_text.push(' ');
                            }
                            right_text.push_str(&run.text);
                        } else {
                            if !left_text.is_empty() {
                                left_text.push(' ');
                            }
                            left_text.push_str(&run.text);
                        }
                    }
                }
            }

            let full_text: String = p
                .lines
                .iter()
                .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
                .collect::<Vec<_>>()
                .join(" ");

            let has_url = full_text.contains("http://") || full_text.contains("https://");

            if has_url || (saw_tab && !left_text.is_empty() && !right_text.is_empty()) {
                // Dual footer (left URL + right dynamic page number) via native Word right tab stop
                let url_content = if !left_text.is_empty() {
                    crate::utils::escape_xml(&left_text)
                } else {
                    let u = full_text.split_whitespace().next().unwrap_or(&full_text);
                    crate::utils::escape_xml(u)
                };

                let page_field = if right_text.contains('/') || full_text.contains('/') {
                    r#"    <w:fldSimple w:instr="PAGE"/>
    <w:r>
      <w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr>
      <w:t>/</w:t>
    </w:r>
    <w:fldSimple w:instr="NUMPAGES"/>"#
                } else {
                    r#"    <w:fldSimple w:instr="PAGE"/>"#
                };

                xml.push_str(&format!(
                    r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Footer"/>
      <w:tabs>
        <w:tab w:val="right" w:leader="none" w:pos="{}"/>
      </w:tabs>
      <w:jc w:val="left"/>
    </w:pPr>
    <w:r>
      <w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr>
      <w:t xml:space="preserve">{}</w:t>
    </w:r>
    <w:r><w:tab/></w:r>
{}
  </w:p>
"#,
                    printable_width_twips,
                    url_content,
                    page_field
                ));
            } else if let Some(prefix) = crate::dla::strip_trailing_page_token(full_text.trim()) {
                let is_right_aligned = p.alignment == "right" || p.bbox.x_min >= 350.0;
                let jc = if is_right_aligned { "right" } else if p.alignment == "left" { "left" } else { "center" };

                xml.push_str(&format!(
                    r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Footer"/>
      <w:jc w:val="{}"/>
    </w:pPr>
    <w:r><w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr><w:t xml:space="preserve">{} </w:t></w:r>
    <w:fldSimple w:instr="PAGE"/>
  </w:p>
"#,
                    jc,
                    crate::utils::escape_xml(&prefix)
                ));
            } else {
                let is_pure_num = crate::dla::is_page_number_pattern(&full_text) || full_text.chars().all(|c| c.is_ascii_digit());
                let is_right_aligned = p.alignment == "right" || p.bbox.x_min >= 350.0;
                let jc = if is_right_aligned { "right" } else if p.alignment == "left" { "left" } else { "center" };

                if is_pure_num {
                    xml.push_str(&format!(
                        r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Footer"/>
      <w:jc w:val="{}"/>
    </w:pPr>
    <w:fldSimple w:instr="PAGE"/>
  </w:p>
"#,
                        jc
                    ));
                } else {
                    xml.push_str(&format!(
                        r#"  <w:p>
    <w:pPr>
      <w:pStyle w:val="Footer"/>
      <w:jc w:val="{}"/>
    </w:pPr>
    <w:r><w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/><w:color w:val="595959"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r>
  </w:p>
"#,
                        jc,
                        crate::utils::escape_xml(&full_text)
                    ));
                }
            }
        }
        xml.push_str("</w:ftr>");
        xml
    }

    /// Generates word/settings.xml
    pub fn settings() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
</w:settings>"#.to_string()
    }
}

