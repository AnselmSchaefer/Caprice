//! Export to Word's `.docx` (Office Open XML, ISO/IEC 29500), which Word, LibreOffice, Google Docs
//! and Pages all open. Mapping:
//!
//! * paragraphs = paragraphs of the flow; alignment, line spacing, bullets and numbering = paragraph properties
//! * hard page breaks = "page break before" on the paragraph that follows
//! * font, size, bold, underline = run properties
//! * page setup = section properties (paper, orientation, margins), page numbers = a footer field
//! * notes = Word comments anchored to the same text

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use crate::model::{Align, Doc, ListKind, Note, PAGE_BREAK, ParaAttrs, Style};

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

/// Builds `<w:body>` content. A paragraph's properties are only known at its end, so its runs are
/// collected first.
struct Body {
    /// Finished paragraphs.
    xml: String,
    /// Runs and comment markers of the open paragraph.
    para: String,
    run: Option<(Style, String)>,
    /// Does the open paragraph have anything in it (text, a tab or a comment marker)?
    has_content: bool,
    /// Is the next paragraph the first on its page?
    at_page_start: bool,
    /// A hard page break came just before.
    page_break_before: bool,
    last_list: ListKind,
    /// How many separate numbered lists there have been (each restarts at 1).
    numbered_lists: u32,
}

impl Body {
    fn new() -> Self {
        Self {
            xml: String::new(),
            para: String::new(),
            run: None,
            has_content: false,
            at_page_start: true,
            page_break_before: false,
            last_list: ListKind::None,
            numbered_lists: 0,
        }
    }

    fn flush_run(&mut self) {
        if let Some((st, text)) = self.run.take() {
            let _ = write!(
                self.para,
                "<w:r><w:rPr>{}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                run_props(&st),
                esc(&text)
            );
            self.has_content = true;
        }
    }

    fn push_char(&mut self, c: char, st: &Style) {
        match c {
            '\t' => {
                self.flush_run();
                let _ = write!(self.para, "<w:r><w:rPr>{}</w:rPr><w:tab/></w:r>", run_props(st));
                self.has_content = true;
            }
            c => match &mut self.run {
                Some((cur, text)) if cur.same_char(st) => text.push(c),
                _ => {
                    self.flush_run();
                    self.run = Some((st.clone(), c.to_string()));
                }
            },
        }
    }

    fn comment_start(&mut self, id: usize) {
        self.flush_run();
        let _ = write!(self.para, "<w:commentRangeStart w:id=\"{id}\"/>");
        self.has_content = true;
    }

    fn comment_end(&mut self, id: usize) {
        self.flush_run();
        let _ = write!(
            self.para,
            "<w:commentRangeEnd w:id=\"{id}\"/><w:r><w:commentReference w:id=\"{id}\"/></w:r>"
        );
        self.has_content = true;
    }

    /// The paragraph ends with `term` (`\n`, or a page break). `\n` always makes a paragraph; a page
    /// break only does if it has text or it is what holds an otherwise empty page.
    fn end_paragraph(&mut self, term: char, st: &Style) {
        self.flush_run();
        let is_break = term == PAGE_BREAK;
        if !is_break || self.has_content || self.at_page_start {
            self.emit(st.para, st);
        }
        if is_break {
            self.page_break_before = true;
            self.at_page_start = true;
        }
    }

    fn emit(&mut self, attrs: ParaAttrs, term: &Style) {
        let mut ppr = String::new();
        if self.page_break_before {
            ppr.push_str("<w:pageBreakBefore/>");
        }
        match attrs.list {
            ListKind::None => {}
            kind => {
                if kind == ListKind::Numbered && self.last_list != ListKind::Numbered {
                    self.numbered_lists += 1; // a new list: numbering starts over
                }
                let num_id = if kind == ListKind::Bullet { 1 } else { 1 + self.numbered_lists };
                let _ = write!(ppr, "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr>");
            }
        }
        if (attrs.spacing - 1.0).abs() > 0.001 {
            let _ = write!(ppr, "<w:spacing w:line=\"{}\" w:lineRule=\"auto\"/>", (attrs.spacing * 240.0).round() as i32);
        }
        if attrs.list != ListKind::None {
            ppr.push_str("<w:ind w:left=\"560\" w:hanging=\"360\"/>");
        }
        match attrs.align {
            Align::Left => {}
            Align::Center => ppr.push_str("<w:jc w:val=\"center\"/>"),
            Align::Right => ppr.push_str("<w:jc w:val=\"right\"/>"),
            Align::Justify => ppr.push_str("<w:jc w:val=\"both\"/>"),
        }
        // The paragraph mark's own size keeps empty lines the right height.
        let _ = write!(ppr, "<w:rPr>{}</w:rPr>", run_props(term));
        let _ = write!(self.xml, "<w:p><w:pPr>{ppr}</w:pPr>{}</w:p>", std::mem::take(&mut self.para));
        self.has_content = false;
        self.at_page_start = false;
        self.page_break_before = false;
        self.last_list = attrs.list;
    }
}

/// The body XML, and how many separate numbered lists it uses.
fn document_xml(doc: &Doc, has_footer: bool) -> (String, u32) {
    let mut body = Body::new();
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
            if crate::model::is_terminator(c) {
                body.end_paragraph(c, st);
            } else {
                body.push_char(c, st);
            }
        }
    }
    body.flush_run();
    if body.has_content || !body.para.is_empty() {
        body.emit(ParaAttrs::default(), &fallback); // markers after the final paragraph mark
    }

    let s = &doc.setup;
    let size = s.size();
    let footer_ref = if has_footer { "<w:footerReference w:type=\"default\" r:id=\"rIdFooter\"/>" } else { "" };
    let orient = if size.x > size.y { " w:orient=\"landscape\"" } else { "" };
    let xml = format!(
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
    );
    (xml, body.numbered_lists)
}

/// Bullet and numbering definitions: one bullet list, and one numbered list per run of numbered paragraphs.
fn numbering_xml(numbered_lists: u32) -> String {
    let level = |fmt: &str, text: &str| {
        format!(
            "<w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{text}\"/>\
<w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"560\" w:hanging=\"360\"/></w:pPr></w:lvl>"
        )
    };
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:numbering xmlns:w=\"{NS_W}\">");
    let _ = write!(s, "<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"singleLevel\"/>{}</w:abstractNum>", level("bullet", "\u{2022}"));
    let _ = write!(s, "<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"singleLevel\"/>{}</w:abstractNum>", level("decimal", "%1."));
    s.push_str("<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>");
    for k in 0..numbered_lists {
        let _ = write!(
            s,
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/></w:lvlOverride></w:num>",
            k + 2
        );
    }
    s.push_str("</w:numbering>");
    s
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
    let (document, numbered_lists) = document_xml(doc, has_footer);
    let has_numbering = document.contains("<w:numPr>");

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
    if has_numbering {
        let _ = write!(content_types, "<Override PartName=\"/word/numbering.xml\" ContentType=\"{CT}.numbering+xml\"/>");
        let _ = write!(rels, "<Relationship Id=\"rIdNumbering\" Type=\"{REL}/numbering\" Target=\"numbering.xml\"/>");
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
        ("word/document.xml", document),
        ("word/_rels/document.xml.rels", rels),
        ("word/styles.xml", styles_xml(&default)),
    ];
    if has_comments {
        parts.push(("word/comments.xml", comments_xml(&doc.notes)));
    }
    if has_numbering {
        parts.push(("word/numbering.xml", numbering_xml(numbered_lists)));
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
    use crate::model::Orientation;
    use std::io::Read;

    fn read_part(bytes: &[u8], name: &str) -> Option<String> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut file = zip.by_name(name).ok()?;
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        Some(s)
    }

    /// A document with this text (which must end in the final paragraph mark), all in `style`.
    fn doc_with(text: &str, style: Style) -> Doc {
        assert!(text.ends_with('\n'));
        let mut d = Doc::new();
        d.flow.text = text.to_owned();
        d.flow.styles = vec![style; text.chars().count()];
        d
    }

    fn count(xml: &str, tag: &str) -> usize {
        roxmltree::Document::parse(xml).unwrap().descendants().filter(|n| n.tag_name().name() == tag).count()
    }

    #[test]
    fn every_part_is_well_formed_xml() {
        let st = Style::new("Liberation Serif");
        let mut d = doc_with("a < b & c\n\u{c}tab\there\nitem\n", st.clone());
        d.setup.page_numbers = true;
        let n = d.flow.styles.len();
        d.flow.styles[n - 1].para = ParaAttrs { list: ListKind::Numbered, ..Default::default() };
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
            "word/numbering.xml",
        ] {
            let xml = read_part(&bytes, name).unwrap_or_else(|| panic!("missing {name}"));
            roxmltree::Document::parse(&xml).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn text_styles_breaks_and_setup_are_written() {
        let plain = Style::new("Liberation Serif");
        let bold = Style { bold: true, underline: true, size: 14.0, ..plain.clone() };
        let mut d = doc_with("hi\n\u{c}x\n", plain.clone());
        d.flow.styles[0] = bold.clone();
        d.flow.styles[1] = bold.clone();
        d.setup.orientation = Orientation::Landscape;
        let bytes = to_docx(&d).unwrap();
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        assert_eq!(count(&xml, "p"), 2, "a paragraph for each of the two lines, none for the bare break");
        assert_eq!(count(&xml, "pageBreakBefore"), 1, "the paragraph after the break starts a page");
        assert!(xml.contains("w:ascii=\"Times New Roman\""), "look-alike font gets its Office name");
        assert!(xml.contains("<w:b/>") && xml.contains("<w:u w:val=\"single\"/>"));
        assert!(xml.contains("w:val=\"28\""), "14pt = 28 half-points");
        assert!(xml.contains("w:orient=\"landscape\""));
        assert!(xml.contains("w:w=\"16840\" w:h=\"11900\""), "A4 landscape in twips");
        assert!(xml.contains("w:top=\"1440\"") && xml.contains("w:left=\"1440\""), "1 inch margins");
        assert!(read_part(&bytes, "word/footer1.xml").is_none(), "no footer without page numbers");
        assert!(read_part(&bytes, "word/numbering.xml").is_none(), "no numbering part without lists");
    }

    #[test]
    fn an_empty_page_between_breaks_is_kept() {
        let st = Style::new("Arial");
        let d = doc_with("a\u{c}\u{c}b\n", st);
        let xml = read_part(&to_docx(&d).unwrap(), "word/document.xml").unwrap();
        assert_eq!(count(&xml, "p"), 3, "a, an empty page, b");
        assert_eq!(count(&xml, "pageBreakBefore"), 2);
    }

    #[test]
    fn paragraph_formats_are_written() {
        let st = Style::new("Arial");
        let mut d = doc_with("one\ntwo\nthree\nfour\nfive\n", st);
        let para = |d: &mut Doc, end: usize, p: ParaAttrs| d.flow.styles[end].para = p;
        para(&mut d, 3, ParaAttrs { align: Align::Center, ..Default::default() });
        para(&mut d, 7, ParaAttrs { align: Align::Justify, spacing: 1.5, ..Default::default() });
        para(&mut d, 13, ParaAttrs { list: ListKind::Bullet, ..Default::default() });
        para(&mut d, 18, ParaAttrs { list: ListKind::Numbered, ..Default::default() });
        para(&mut d, 23, ParaAttrs { list: ListKind::Numbered, ..Default::default() });
        let bytes = to_docx(&d).unwrap();
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        assert!(xml.contains("<w:jc w:val=\"center\"/>") && xml.contains("<w:jc w:val=\"both\"/>"));
        assert!(xml.contains("w:line=\"360\""), "1.5 line spacing");
        assert_eq!(count(&xml, "numPr"), 3);
        let numbering = read_part(&bytes, "word/numbering.xml").unwrap();
        assert!(numbering.contains("w:numFmt w:val=\"bullet\"") && numbering.contains("w:numFmt w:val=\"decimal\""));
        assert_eq!(count(&numbering, "num"), 2, "one bullet list and one numbered list");
    }

    #[test]
    fn two_separate_numbered_lists_each_restart() {
        let st = Style::new("Arial");
        let mut d = doc_with("a\nb\nbreak\nc\n", st);
        let numbered = ParaAttrs { list: ListKind::Numbered, ..Default::default() };
        for end in [1, 3, 11] {
            d.flow.styles[end].para = numbered;
        }
        let bytes = to_docx(&d).unwrap();
        let numbering = read_part(&bytes, "word/numbering.xml").unwrap();
        assert_eq!(numbering.matches("startOverride").count(), 2);
        let xml = read_part(&bytes, "word/document.xml").unwrap();
        assert!(xml.contains("<w:numId w:val=\"2\"/>") && xml.contains("<w:numId w:val=\"3\"/>"));
    }

    #[test]
    fn notes_become_comments_over_the_same_text() {
        let st = Style::new("Arial");
        let mut d = doc_with("one two three\n", st);
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
        assert_eq!(count(&xml, "p"), 1);
    }
}

#[cfg(test)]
mod sample {
    use super::*;
    use crate::model::is_terminator;

    /// Writes a sample document for checking in real office suites:
    /// `CAPRICE_SAMPLE_OUT=/some/file.docx cargo test sample_docx`
    #[test]
    fn sample_docx() {
        let Some(out) = std::env::var_os("CAPRICE_SAMPLE_OUT") else { return };
        let plain = Style::new("Liberation Serif");
        let bold = Style { bold: true, size: 20.0, ..plain.clone() };
        let under = Style { underline: true, ..plain.clone() };
        let centered = ParaAttrs { align: Align::Center, ..Default::default() };
        let bullet = ParaAttrs { list: ListKind::Bullet, ..Default::default() };
        let numbered = ParaAttrs { list: ListKind::Numbered, ..Default::default() };
        let double = ParaAttrs { spacing: 2.0, align: Align::Justify, ..Default::default() };
        let mut d = Doc::new();
        d.flow.text.clear();
        d.flow.styles.clear();
        let parts: Vec<(&str, &Style, ParaAttrs)> = vec![
            ("A centered title", &bold, ParaAttrs::default()),
            ("\n", &bold, centered),
            ("Plain text, then ", &plain, ParaAttrs::default()),
            ("underlined words", &under, ParaAttrs::default()),
            (" and more plain text.", &plain, ParaAttrs::default()),
            ("\n", &plain, ParaAttrs::default()),
            ("first bullet", &plain, ParaAttrs::default()),
            ("\n", &plain, bullet),
            ("second bullet", &plain, ParaAttrs::default()),
            ("\n", &plain, bullet),
            ("step one", &plain, ParaAttrs::default()),
            ("\n", &plain, numbered),
            ("step two", &plain, ParaAttrs::default()),
            ("\n", &plain, numbered),
            ("A justified paragraph with double line spacing that is long enough to wrap onto several lines so that the spacing can be seen clearly.", &plain, ParaAttrs::default()),
            ("\n", &plain, double),
            ("\u{c}", &plain, ParaAttrs::default()),
            ("Second page starts here.", &plain, ParaAttrs::default()),
            ("\n", &plain, ParaAttrs::default()),
        ];
        for (t, s, p) in parts {
            d.flow.text.push_str(t);
            for ch in t.chars() {
                d.flow.styles.push(if is_terminator(ch) { s.with_para(p) } else { s.clone() });
            }
        }
        d.setup.page_numbers = true;
        d.setup.margin_left = 100.0;
        d.notes.push(Note { id: 1, start: 26, end: 42, text: "Remember to rephrase this.".into(), color: 1 });
        std::fs::write(out, to_docx(&d).unwrap()).unwrap();
    }
}
