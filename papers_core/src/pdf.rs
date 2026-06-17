//! Enhanced PDF export — generates structured PDF reports with sections,
//! tables, code blocks, and equation rendering using a minimal PDF writer.

use std::io::Write;
use anyhow::Result;

/// Dimensions and layout constants.
const PAGE_WIDTH: f64 = 595.0;  // A4 in points
const PAGE_HEIGHT: f64 = 842.0;
const MARGIN_LEFT: f64 = 50.0;
const MARGIN_RIGHT: f64 = 50.0;
const MARGIN_TOP: f64 = 60.0;
const MARGIN_BOTTOM: f64 = 60.0;
const LINE_HEIGHT: f64 = 14.0;
const FONT_SIZE_NORMAL: f64 = 10.0;
const FONT_SIZE_TITLE: f64 = 16.0;
const FONT_SIZE_HEADER: f64 = 13.0;

/// A fully structured PDF document.
pub struct PdfDocument {
    objects: Vec<PdfObject>,
    current_y: f64,
    current_page: usize,
}

#[derive(Debug, Clone)]
enum PdfObject {
    Text { content: String, x: f64, y: f64, font_size: f64, bold: bool, page: usize },
    Line { x1: f64, y1: f64, x2: f64, y2: f64, page: usize },
    Table { headers: Vec<String>, rows: Vec<Vec<String>>, x: f64, y: f64, page: usize },
    CodeBlock { content: String, x: f64, y: f64, page: usize },
}

impl PdfDocument {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            current_y: PAGE_HEIGHT - MARGIN_TOP,
            current_page: 1,
        }
    }

    /// Add a title heading.
    pub fn add_title(&mut self, text: &str) {
        let y = self.advance(FONT_SIZE_TITLE + 4.0);
        self.objects.push(PdfObject::Text {
            content: text.to_string(),
            x: MARGIN_LEFT,
            y,
            font_size: FONT_SIZE_TITLE,
            bold: true,
            page: self.current_page,
        });
        self.advance(LINE_HEIGHT);
    }

    /// Add a section header.
    pub fn add_header(&mut self, text: &str) {
        let y = self.advance(FONT_SIZE_HEADER + 4.0);
        self.objects.push(PdfObject::Text {
            content: text.to_string(),
            x: MARGIN_LEFT,
            y,
            font_size: FONT_SIZE_HEADER,
            bold: true,
            page: self.current_page,
        });
        self.advance(LINE_HEIGHT / 2.0);
    }

    /// Add a normal text paragraph.
    pub fn add_text(&mut self, text: &str) {
        let max_width = PAGE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
        let chars_per_line = (max_width / (FONT_SIZE_NORMAL * 0.55)) as usize;

        for line in wrap_text(text, chars_per_line) {
            let y = self.advance(LINE_HEIGHT);
            self.objects.push(PdfObject::Text {
                content: line.to_string(),
                x: MARGIN_LEFT,
                y,
                font_size: FONT_SIZE_NORMAL,
                bold: false,
                page: self.current_page,
            });
        }
    }

    /// Add a horizontal divider.
    pub fn add_divider(&mut self) {
        let y = self.advance(LINE_HEIGHT / 2.0);
        self.objects.push(PdfObject::Line {
            x1: MARGIN_LEFT,
            y1: y,
            x2: PAGE_WIDTH - MARGIN_RIGHT,
            y2: y,
            page: self.current_page,
        });
        self.advance(LINE_HEIGHT / 2.0);
    }

    /// Add a table with headers and rows.
    pub fn add_table(&mut self, headers: &[&str], rows: &[Vec<String>]) {
        let y = self.advance(LINE_HEIGHT);
        self.objects.push(PdfObject::Table {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: rows.to_vec(),
            x: MARGIN_LEFT,
            y,
            page: self.current_page,
        });
        self.advance((rows.len() + 2) as f64 * LINE_HEIGHT);
    }

    /// Add a code block.
    pub fn add_code_block(&mut self, code: &str) {
        for line in code.lines() {
            let y = self.advance(LINE_HEIGHT * 0.85);
            self.objects.push(PdfObject::CodeBlock {
                content: line.to_string(),
                x: MARGIN_LEFT + 15.0,
                y,
                page: self.current_page,
            });
        }
        self.advance(LINE_HEIGHT);
    }

    /// Advance current_y, starting a new page if needed.
    fn advance(&mut self, dy: f64) -> f64 {
        self.current_y -= dy;
        if self.current_y < MARGIN_BOTTOM {
            self.current_page += 1;
            self.current_y = PAGE_HEIGHT - MARGIN_TOP;
        }
        self.current_y
    }

    /// Render and write the PDF to a byte vector.
    pub fn render(&self) -> Result<Vec<u8>> {
        let num_pages = self.current_page.max(1);

        // Group objects by page
        let mut pages_objects: Vec<Vec<&PdfObject>> = vec![Vec::new(); num_pages];
        for obj in &self.objects {
            let page_idx = match obj {
                PdfObject::Text { page, .. }
                | PdfObject::Line { page, .. }
                | PdfObject::Table { page, .. }
                | PdfObject::CodeBlock { page, .. } => (page - 1).min(num_pages - 1),
            };
            pages_objects[page_idx].push(obj);
        }

        // Object IDs: 1=catalog, 2=pages, 3..3+N-1=page objects, 3+N..=content streams
        let page_object_ids: Vec<usize> = (0..num_pages).map(|i| 3 + i).collect();
        let content_object_ids: Vec<usize> = (0..num_pages).map(|i| 3 + num_pages + i).collect();

        // Build content streams for each page
        let mut page_streams: Vec<String> = vec![String::new(); num_pages];
        for (page_idx, objects) in pages_objects.iter().enumerate() {
            let stream = &mut page_streams[page_idx];
            stream.push_str("BT\n");
            for obj in objects {
                match obj {
                    PdfObject::Text { content, x, y, font_size, bold, .. } => {
                        let font = if *bold { "/F2" } else { "/F1" };
                        stream.push_str(&format!("{} {} Tf 1 0 0 1 {} {} Tm ({}) Tj\n",
                            font, font_size, x, y, escape_pdf_string(content)));
                    }
                    PdfObject::Line { x1, y1, x2, y2, .. } => {
                        stream.push_str(&format!(
                            "{} {} m {} {} l S\n", x1, y1, x2, y2));
                    }
                    PdfObject::Table { headers, rows, x, y, .. } => {
                        let num_cols = headers.len();
                        let col_width = (PAGE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT) / num_cols as f64;

                        for (i, h) in headers.iter().enumerate() {
                            stream.push_str(&format!("/F2 10 Tf 1 0 0 1 {} {} Tm ({}) Tj\n",
                                x + i as f64 * col_width, y,
                                escape_pdf_string(h)));
                        }
                        let line_y = y - LINE_HEIGHT / 2.0;
                        stream.push_str(&format!("{} {} m {} {} l S\n",
                            x, line_y, x + num_cols as f64 * col_width, line_y));

                        for (r, row) in rows.iter().enumerate() {
                            let row_y = y - (r + 2) as f64 * LINE_HEIGHT;
                            for (i, cell) in row.iter().enumerate() {
                                stream.push_str(&format!("/F1 9 Tf 1 0 0 1 {} {} Tm ({}) Tj\n",
                                    x + i as f64 * col_width, row_y,
                                    escape_pdf_string(cell)));
                            }
                        }
                    }
                    PdfObject::CodeBlock { content, x, y, .. } => {
                        stream.push_str(&format!("/F3 8 Tf 1 0 0 1 {} {} Tm ({}) Tj\n",
                            *x, *y, escape_pdf_string(content)));
                    }
                }
            }
            stream.push_str("ET\n");
        }

        self.render_two_pass(&page_streams, num_pages, &page_object_ids, &content_object_ids)
    }

    fn render_two_pass(
        &self,
        page_streams: &[String],
        num_pages: usize,
        page_object_ids: &[usize],
        content_object_ids: &[usize],
    ) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut object_offsets = Vec::new();

        writeln!(buf, "%PDF-1.4")?;

        // Object 1: Catalog
        object_offsets.push(buf.len());
        writeln!(buf, "1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj")?;

        // Object 2: Pages — we know all page IDs now
        object_offsets.push(buf.len());
        let kids: String = page_object_ids.iter()
            .map(|id| format!("{} 0 R", id))
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(buf, "2 0 obj<</Type/Pages/Kids[{}]/Count {}>>endobj", kids, num_pages)?;

        // Page objects
        for i in 0..num_pages {
            object_offsets.push(buf.len());
            writeln!(buf, "{} 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 {} {}]/\
                Resources<</Font<</F1<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>\
                /F2<</Type/Font/Subtype/Type1/BaseFont/Helvetica-Bold>>\
                /F3<</Type/Font/Subtype/Type1/BaseFont/Courier>>>>>>\
                /Contents {} 0 R>>endobj",
                page_object_ids[i], PAGE_WIDTH, PAGE_HEIGHT, content_object_ids[i])?;
        }

        // Content streams
        for i in 0..num_pages {
            let stream_bytes = page_streams[i].as_bytes();
            object_offsets.push(buf.len());
            writeln!(buf, "{} 0 obj<</Length {}>>stream", content_object_ids[i], stream_bytes.len())?;
            buf.write_all(stream_bytes)?;
            writeln!(buf)?;
            writeln!(buf, "endstream")?;
            writeln!(buf, "endobj")?;
        }

        // Cross-reference table
        let xref_offset = buf.len();
        writeln!(buf, "xref")?;
        writeln!(buf, "0 {}", object_offsets.len() + 1)?;
        writeln!(buf, "0000000000 65535 f ")?;
        for &offset in &object_offsets {
            writeln!(buf, "{:010} 00000 n ", offset)?;
        }

        // Trailer
        writeln!(buf, "trailer")?;
        writeln!(buf, "<</Size {}/Root 1 0 R>>", object_offsets.len() + 1)?;
        writeln!(buf, "startxref")?;
        writeln!(buf, "{}", xref_offset)?;
        writeln!(buf, "%%%%EOF")?;

        Ok(buf)
    }

    /// Generate a complete analysis report as PDF.
    #[allow(clippy::too_many_arguments)]
    pub fn from_analysis_report(
        title: &str,
        authors: &[String],
        source: &str,
        score: f64,
        recommendation: &str,
        summary: &str,
        contributions: &[String],
        equations: &[String],
        algorithms: &[(&str, Option<&str>)],
    ) -> Result<Vec<u8>> {
        let mut doc = Self::new();

        doc.add_title(title);
        doc.add_text(&format!("Authors: {}", authors.join(", ")));
        doc.add_text(&format!("Source: {}", source));
        doc.add_text(&format!("Integration Score: {:.2} — Recommendation: {}", score, recommendation));
        doc.add_divider();

        doc.add_header("Executive Summary");
        doc.add_text(summary);
        doc.add_divider();

        doc.add_header("Contributions");
        for c in contributions {
            doc.add_text(&format!("- {}", c));
        }
        doc.add_divider();

        if !equations.is_empty() {
            doc.add_header("Key Equations");
            for eq in equations {
                doc.add_code_block(eq);
            }
            doc.add_divider();
        }

        if !algorithms.is_empty() {
            doc.add_header("Algorithms");
            for (name, complexity) in algorithms {
                let label = if let Some(c) = complexity {
                    format!("{} (complexity: {})", name, c)
                } else {
                    name.to_string()
                };
                doc.add_text(&format!("- {}", label));
            }
        }

        doc.add_divider();
        doc.add_text(&format!("Generated by PAPERS V2 — {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

        doc.render()
    }
}

/// Wrap text to fit within a given character width.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();
        for word in words {
            if current_line.len() + word.len() + 1 > max_chars && !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line = word.to_string();
            } else if current_line.is_empty() {
                current_line = word.to_string();
            } else {
                current_line.push(' ');
                current_line.push_str(word);
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

/// Escape special characters for PDF string literals.
fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_ascii_control() {
                ch.to_string()
            } else if ch == '\n' {
                String::new()
            } else {
                format!("\\{:03o}", ch as u32)
            }
        })
        .collect::<String>()
}

impl Default for PdfDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pdf() {
        let doc = PdfDocument::new();
        let result = doc.render();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pdf_with_content() {
        let mut doc = PdfDocument::new();
        doc.add_title("Test Report");
        doc.add_header("Section 1");
        doc.add_text("This is a test paragraph.");
        doc.add_code_block("fn main() {\n    println!(\"hello\");\n}");
        let result = doc.render();
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%%%EOF\n"));
    }

    #[test]
    fn test_wrap_text() {
        let lines = wrap_text("hello world foo bar", 10);
        assert_eq!(lines, vec!["hello", "world foo", "bar"]);
    }

    #[test]
    fn test_escape_pdf_string() {
        assert_eq!(escape_pdf_string("test (parens) \\ slash"), r"test \(parens\) \\ slash");
    }

    #[test]
    fn test_add_title_produces_valid_pdf() {
        let mut doc = PdfDocument::new();
        doc.add_title("My Research Paper");
        let result = doc.render();
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%%%EOF\n"));
        let pdf_str = String::from_utf8_lossy(&bytes);
        assert!(pdf_str.contains("My Research Paper"));
        assert!(pdf_str.contains("/F2"));
        assert!(pdf_str.contains("/Helvetica-Bold"));
    }

    #[test]
    fn test_add_table_produces_valid_pdf() {
        let mut doc = PdfDocument::new();
        doc.add_table(&["Name", "Score"], &[
            vec!["Alice".to_string(), "95".to_string()],
            vec!["Bob".to_string(), "87".to_string()],
        ]);
        let result = doc.render();
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%%%EOF\n"));
        let pdf_str = String::from_utf8_lossy(&bytes);
        assert!(pdf_str.contains("Alice"));
        assert!(pdf_str.contains("95"));
        assert!(pdf_str.contains("Bob"));
        assert!(pdf_str.contains("87"));
        assert!(pdf_str.contains("Name"));
        assert!(pdf_str.contains("Score"));
    }

    #[test]
    fn test_add_divider_produces_valid_pdf() {
        let mut doc = PdfDocument::new();
        doc.add_divider();
        let result = doc.render();
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%%%EOF\n"));
        let pdf_str = String::from_utf8_lossy(&bytes);
        assert!(pdf_str.contains(" m "));
        assert!(pdf_str.contains(" l S"));
    }

    #[test]
    fn test_wrap_text_empty_produces_single_empty_string() {
        let lines = wrap_text("", 80);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_wrap_text_long_word_exceeds_max_chars() {
        let word = "supercalifragilisticexpialidocious";
        let lines = wrap_text(word, 5);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], word);
    }

    #[test]
    fn test_wrap_text_multi_empty_paragraphs() {
        let lines = wrap_text("\n\n", 80);
        assert_eq!(lines, vec!["", "", ""]);
    }
}
