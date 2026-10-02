//! Export to Word's `.docx` (Office Open XML, ISO/IEC 29500), which Word, LibreOffice, Google Docs
//! and Pages all open. Mapping:
//!
//! * paragraphs = lines of the flow, hard page breaks = page-break runs
//! * font, size, bold, underline = run properties
//! * page setup = section properties (paper, orientation, margins), page numbers = a footer field
//! * notes = Word comments anchored to the same text

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use crate::model::{Doc, Note, PAGE_BREAK, Style};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CT: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";

/// Fonts that are only look-alikes of the usual Office fonts are written under the Office name,
/// so the document looks the same in Word (LibreOffice maps them back automatically).
fn office_font(name: &str) -> &str {
    match name {
        "Liberation Serif" | "Tinos" | "Nimbus Roman" => "Times New Roman",
        "Liberation Sans" | "Arimo" | "Nimbus Sans" => "Arial",
        "Liberation Mono" | "Cousine" | "Nimbus Mono PS" => "Courier New",
        "Carlito" => "Calibri",
        "Caladea" => "Cambria",
        "P052" => "Palatino Linotype",
        other => other,
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {} // not allowed in XML
            c => out.push(c),
        }
    }
    out
}

fn twips(points: f32) -> i32 {
    (points * 20.0).round() as i32
}

fn run_props(st: &Style) -> String {
    let font = esc(office_font(&st.font));
    let half_points = (st.size * 2.0).round().max(2.0) as i32;
    let mut s = format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/>");
    if st.bold {
        s.push_str("<w:b/><w:bCs/>");
    }
    let _ = write!(s, "<w:sz w:val=\"{half_points}\"/><w:szCs w:val=\"{half_points}\"/>");
    if st.underline {
        s.push_str("<w:u w:val=\"single\"/>");
    }
    s
}

/// Builds `<w:body>` content, tracking the open paragraph and the pending run.
struct Body {
    xml: String,
    in_paragraph: bool,
    run: Option<(Style, String)>,
}

impl Body {
    fn open_paragraph(&mut self) {
        if !self.in_paragraph {
            self.xml.push_str("<w:p>");
            self.in_paragraph = true;
        }
    }

    fn flush_run(&mut self) {
        if let Some((st, text)) = self.run.take() {
            self.open_paragraph();
            let _ = write!(
                self.xml,
                "<w:r><w:rPr>{}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                run_props(&st),
                esc(&text)
            );
        }
    }

    fn end_paragraph(&mut self) {
        self.flush_run();
        self.open_paragraph();
        self.xml.push_str("</w:p>");
        self.in_paragraph = false;
    }

    fn push_char(&mut self, c: char, st: &Style) {
        match c {
            '\n' => self.end_paragraph(),
            PAGE_BREAK => {
                self.flush_run();
                self.open_paragraph();
                self.xml.push_str("<w:r><w:br w:type=\"page\"/></w:r>");
            }
            '\t' => {
                self.flush_run();
                self.open_paragraph();
                let _ = write!(self.xml, "<w:r><w:rPr>{}</w:rPr><w:tab/></w:r>", run_props(st));
            }
            c => match &mut self.run {
                Some((cur, text)) if cur == st => text.push(c),
                _ => {
                    self.flush_run();
                    self.run = Some((st.clone(), c.to_string()));
                }
            },
        }
    }

    fn comment_start(&mut self, id: usize) {
        self.flush_run();
        self.open_paragraph();
        let _ = write!(self.xml, "<w:commentRangeStart w:id=\"{id}\"/>");
    }

    fn comment_end(&mut self, id: usize) {
        self.flush_run();
        self.open_paragraph();
        let _ = write!(
            self.xml,
            "<w:commentRangeEnd w:id=\"{id}\"/><w:r><w:commentReference w:id=\"{id}\"/></w:r>"
        );
    }
}

fn document_xml(doc: &Doc, has_footer: bool) -> String {
    let mut body = Body { xml: String::new(), in_paragraph: false, run: None };
    let notes: &[Note] = &doc.notes;
    let fallback = Style::new("Times New Roman");
    let mut chars = doc.flow.text.chars().zip(doc.flow.styles.iter().chain(std::iter::repeat(&fallback)));

    let n = doc.total_chars();
    for k in 0..=n {
        for (id, note) in notes.iter().enumerate() {
            if note.start == k {
                body.comment_start(id);
            }
        }
        for (id, note) in notes.iter().enumerate() {
            if note.end == k {
                body.comment_end(id);
            }
        }
        if let Some((c, st)) = chars.next() {
            body.push_char(c, st);
        }
    }
    if body.in_paragraph || body.run.is_some() || body.xml.is_empty() {
        body.end_paragraph();
    }

    let s = &doc.setup;
    let size = s.size();
    let footer_ref = if has_footer { "<w:footerReference w:type=\"default\" r:id=\"rIdFooter\"/>" } else { "" };
    let orient = if size.x > size.y { " w:orient=\"landscape\"" } else { "" };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:body>{}<w:sectPr>{footer_ref}\
<w:pgSz w:w=\"{}\" w:h=\"{}\"{orient}/>\
<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>\
</w:sectPr></w:body></w:document>",
        body.xml,
        twips(size.x),
        twips(size.y),
        twips(s.margin_top),
        twips(s.margin_right),
        twips(s.margin_bottom),
        twips(s.margin_left),
        twips(s.margin_top / 2.0),
        twips(s.margin_bottom / 2.0),
    )
}

fn styles_xml(default: &Style) -> String {
    let font = esc(office_font(&default.font));
    let half_points = (default.size * 2.0).round() as i32;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:styles xmlns:w=\"{NS_W}\"><w:docDefaults><w:rPrDefault><w:rPr>\
<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/>\
<w:sz w:val=\"{half_points}\"/><w:szCs w:val=\"{half_points}\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
</w:docDefaults><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
</w:styles>"
    )
}

/// Current UTC time as `2026-10-02T17:00:00Z` (days-to-date after Howard Hinnant's algorithm).
fn timestamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    iso8601(secs)
}

fn iso8601(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn comments_xml(notes: &[Note]) -> String {
    let now = timestamp_now();
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:comments xmlns:w=\"{NS_W}\">");
    for (id, note) in notes.iter().enumerate() {
        let _ = write!(s, "<w:comment w:id=\"{id}\" w:author=\"Caprice\" w:date=\"{now}\" w:initials=\"C\">");
        for line in note.text.split('\n') {
            let _ = write!(s, "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", esc(line));
        }
        s.push_str("</w:comment>");
    }
    s.push_str("</w:comments>");
    s
}

fn footer_xml(default: &Style) -> String {
    let font = esc(office_font(&default.font));
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:ftr xmlns:w=\"{NS_W}\"><w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/><w:sz w:val=\"20\"/></w:rPr><w:fldChar w:fldCharType=\"begin\"/></w:r>\
<w:r><w:rPr><w:sz w:val=\"20\"/></w:rPr><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r>\
<w:r><w:rPr><w:sz w:val=\"20\"/></w:rPr><w:fldChar w:fldCharType=\"separate\"/></w:r>\
<w:r><w:rPr><w:sz w:val=\"20\"/></w:rPr><w:t>1</w:t></w:r>\
<w:r><w:rPr><w:sz w:val=\"20\"/></w:rPr><w:fldChar w:fldCharType=\"end\"/></w:r></w:p></w:ftr>"
    )
}

/// The whole `.docx` file as bytes.
pub fn to_docx(doc: &Doc) -> Result<Vec<u8>, String> {
    let has_footer = doc.setup.page_numbers;
    let has_comments = !doc.notes.is_empty();
    let default = doc.flow.styles.first().cloned().unwrap_or_else(|| Style::new("Times New Roman"));

    let mut content_types = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"{CT}.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"{CT}.styles+xml\"/>"
    );
    let mut rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rIdStyles\" Type=\"{REL}/styles\" Target=\"styles.xml\"/>"
    );
    if has_comments {
        let _ = write!(content_types, "<Override PartName=\"/word/comments.xml\" ContentType=\"{CT}.comments+xml\"/>");
        let _ = write!(rels, "<Relationship Id=\"rIdComments\" Type=\"{REL}/comments\" Target=\"comments.xml\"/>");
    }
    if has_footer {
        let _ = write!(content_types, "<Override PartName=\"/word/footer1.xml\" ContentType=\"{CT}.footer+xml\"/>");
        let _ = write!(rels, "<Relationship Id=\"rIdFooter\" Type=\"{REL}/footer\" Target=\"footer1.xml\"/>");
    }
    content_types.push_str("</Types>");
    rels.push_str("</Relationships>");

    let root_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"word/document.xml\"/></Relationships>"
    );

    let mut parts: Vec<(&str, String)> = vec![
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", root_rels),
        ("word/document.xml", document_xml(doc, has_footer)),
        ("word/_rels/document.xml.rels", rels),
        ("word/styles.xml", styles_xml(&default)),
    ];
    if has_comments {
        parts.push(("word/comments.xml", comments_xml(&doc.notes)));
    }
    if has_footer {
        parts.push(("word/footer1.xml", footer_xml(&default)));
    }

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, xml) in parts {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn read_part(bytes: &[u8], name: &str) -> Option<String> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut file = zip.by_name(name).ok()?;
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        Some(s)
    }

    fn doc_with(text: &str, styles: Vec<Style>) -> Doc {
        let mut d = Doc::new();
        d.flow.text = text.to_owned();
        d.flow.styles = styles;
        d
    }

    #[test]
    fn every_part_is_well_formed_xml() {
        let st = Style::new("Liberation Serif");
        let mut d = doc_with("a < b & c\n\u{c}tab\there", vec![st; 18]);
        d.setup.page_numbers = true;
        d.notes.push(Note { id: 1, start: 0, end: 5, text: "x & <y>\nsecond".into(), color: 0 });
        let bytes = to_docx(&d).unwrap();
        for name in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/comments.xml",
            "word/footer1.xml",
        ] {
            let xml = read_part(&bytes, name).unwrap_or_else(|| panic!("missing {name}"));
            roxmltree::Document::parse(&xml).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn text_styles_breaks_and_setup_are_written() {
        let plain = Style::new("Liberation Serif");
        let bold = Style { bold: true, underline: true, size: 14.0, ..plain.clone() };
        let mut styles = vec![bold.clone(); 2];
        styles.extend(vec![plain.clone(); 3]); // "hi" bold, "\n\u{c}x"... see below
        let mut d = doc_with("hi\n\u{c}x", { styles.push(plain.clone()); styles });
        d.setup.orientation = crate::model::Orientation::Landscape;
        let bytes = to_docx(&d).unwrap();
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let w = |name: &str| doc.descendants().filter(|n| n.tag_name().name() == name).count();
        assert_eq!(w("p"), 2, "one paragraph per line");
        assert_eq!(w("br"), 1, "hard page break");
        assert!(xml.contains("w:ascii=\"Times New Roman\""), "look-alike font gets its Office name");
        assert!(xml.contains("<w:b/>") && xml.contains("<w:u w:val=\"single\"/>"));
        assert!(xml.contains("w:val=\"28\""), "14pt = 28 half-points");
        assert!(xml.contains("w:orient=\"landscape\""));
        assert!(xml.contains("w:w=\"16840\" w:h=\"11900\""), "A4 landscape in twips");
        assert!(xml.contains("w:top=\"1440\"") && xml.contains("w:left=\"1440\""), "1 inch margins");
        assert!(read_part(&bytes, "word/footer1.xml").is_none(), "no footer without page numbers");
    }

    #[test]
    fn notes_become_comments_over_the_same_text() {
        let st = Style::new("Arial");
        let mut d = doc_with("one two three", vec![st; 13]);
        d.notes.push(Note { id: 1, start: 4, end: 7, text: "check".into(), color: 0 });
        let bytes = to_docx(&d).unwrap();
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        let start = xml.find("commentRangeStart").unwrap();
        let end = xml.find("commentRangeEnd").unwrap();
        let between = &xml[start..end];
        assert!(between.contains(">two<"), "the comment covers exactly the noted word: {between}");
        assert!(!between.contains("one") && !between.contains("three"));
        assert!(read_part(&bytes, "word/comments.xml").unwrap().contains("check"));
    }

    #[test]
    fn timestamps_are_iso_8601() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(951_782_400), "2000-02-29T00:00:00Z"); // leap day
        assert_eq!(iso8601(1_790_960_400), "2026-10-02T17:00:00Z");
    }

    #[test]
    fn an_empty_document_still_has_a_paragraph() {
        let bytes = to_docx(&Doc::new()).unwrap();
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        assert!(xml.contains("<w:p></w:p>"));
    }
}

#[cfg(test)]
mod sample {
    use super::*;

    /// Writes a sample document for checking in real office suites:
    /// `CAPRICE_SAMPLE_OUT=/some/file.docx cargo test sample_docx`
    #[test]
    fn sample_docx() {
        let Some(out) = std::env::var_os("CAPRICE_SAMPLE_OUT") else { return };
        let plain = Style::new("Liberation Serif");
        let bold = Style { bold: true, size: 20.0, ..plain.clone() };
        let under = Style { underline: true, ..plain.clone() };
        let mut d = Doc::new();
        let parts: [(&str, &Style); 5] = [
            ("Title in bold twenty\n", &bold),
            ("Plain text, then ", &plain),
            ("underlined words", &under),
            (" and more plain text.\n", &plain),
            ("\u{c}Second page starts here.", &plain),
        ];
        for (t, s) in parts {
            d.flow.text.push_str(t);
            d.flow.styles.extend(std::iter::repeat_n(s.clone(), t.chars().count()));
        }
        d.setup.page_numbers = true;
        d.setup.margin_left = 100.0;
        d.notes.push(Note { id: 1, start: 28, end: 44, text: "Remember to rephrase this.".into(), color: 1 });
        std::fs::write(out, to_docx(&d).unwrap()).unwrap();
    }
}
