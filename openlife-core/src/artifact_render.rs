//! Deterministic format adapters for reviewed knowledge-work artifacts.
//!
//! The model supplies bounded semantic content. These adapters own binary
//! serialization and immediately re-parse the produced bytes before they can
//! enter the canonical Artifact/Review/materialization path.

use crate::resource::ResourceFormat;
use crate::resource_parser::{extract_resource, ResourceExtractionRequest};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};

const MAX_DOCUMENT_SECTIONS: usize = 64;
const MAX_DOCUMENT_PARAGRAPHS: usize = 512;
const MAX_DOCUMENT_TEXT_CHARS: usize = 500_000;
const MAX_SPREADSHEET_SHEETS: usize = 16;
const MAX_SPREADSHEET_COLUMNS: usize = 64;
const MAX_SPREADSHEET_ROWS: usize = 1_000;
const MAX_SPREADSHEET_CELLS: usize = 10_000;
const MAX_PRESENTATION_SLIDES: usize = 100;
const MAX_PRESENTATION_BULLETS: usize = 1_000;
const PDF_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentArtifactDraft {
    pub title: String,
    pub sections: Vec<DocumentArtifactSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentArtifactSection {
    pub heading: String,
    pub paragraphs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpreadsheetArtifactDraft {
    pub sheets: Vec<SpreadsheetSheetDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpreadsheetSheetDraft {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationArtifactDraft {
    pub title: String,
    pub slides: Vec<PresentationSlideDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationSlideDraft {
    pub title: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedArtifact {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub format: ResourceFormat,
    pub verified_chunk_count: usize,
    /// Text re-extracted from the rendered bytes. Semantic verification uses
    /// this observed representation rather than trusting the model's input
    /// object or opaque binary bytes.
    pub verified_text: String,
}

pub fn render_docx(draft: &DocumentArtifactDraft) -> Result<RenderedArtifact> {
    validate_document_draft(draft)?;
    let document_xml = document_xml(draft);
    let output = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    write_zip_entry(&mut writer, options, "[Content_Types].xml", CONTENT_TYPES)?;
    write_zip_entry(&mut writer, options, "_rels/.rels", ROOT_RELATIONSHIPS)?;
    write_zip_entry(
        &mut writer,
        options,
        "word/_rels/document.xml.rels",
        DOCUMENT_RELATIONSHIPS,
    )?;
    write_zip_entry(&mut writer, options, "word/styles.xml", STYLES)?;
    write_zip_entry(&mut writer, options, "word/document.xml", &document_xml)?;
    let bytes = writer
        .finish()
        .context("artifact_docx_finish_failed")?
        .into_inner();
    verify_docx(bytes)
}

pub fn render_xlsx(draft: &SpreadsheetArtifactDraft) -> Result<RenderedArtifact> {
    validate_spreadsheet_draft(draft)?;
    let output = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    write_zip_entry(
        &mut writer,
        options,
        "[Content_Types].xml",
        &xlsx_content_types(draft.sheets.len()),
    )?;
    write_zip_entry(&mut writer, options, "_rels/.rels", XLSX_ROOT_RELATIONSHIPS)?;
    write_zip_entry(
        &mut writer,
        options,
        "xl/workbook.xml",
        &xlsx_workbook(draft),
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        &xlsx_workbook_relationships(draft.sheets.len()),
    )?;
    write_zip_entry(&mut writer, options, "xl/styles.xml", XLSX_STYLES)?;
    for (index, sheet) in draft.sheets.iter().enumerate() {
        write_zip_entry(
            &mut writer,
            options,
            &format!("xl/worksheets/sheet{}.xml", index + 1),
            &xlsx_sheet(sheet),
        )?;
    }
    let bytes = writer
        .finish()
        .context("artifact_xlsx_finish_failed")?
        .into_inner();
    verify_xlsx(bytes)
}

pub fn render_pptx(draft: &PresentationArtifactDraft) -> Result<RenderedArtifact> {
    validate_presentation_draft(draft)?;
    let output = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    write_zip_entry(
        &mut writer,
        options,
        "[Content_Types].xml",
        &pptx_content_types(draft.slides.len()),
    )?;
    write_zip_entry(&mut writer, options, "_rels/.rels", PPTX_ROOT_RELATIONSHIPS)?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/presentation.xml",
        &pptx_presentation(draft.slides.len()),
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/_rels/presentation.xml.rels",
        &pptx_presentation_relationships(draft.slides.len()),
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/slideMasters/slideMaster1.xml",
        PPTX_SLIDE_MASTER,
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        PPTX_SLIDE_MASTER_RELATIONSHIPS,
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/slideLayouts/slideLayout1.xml",
        PPTX_SLIDE_LAYOUT,
    )?;
    write_zip_entry(
        &mut writer,
        options,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        PPTX_SLIDE_LAYOUT_RELATIONSHIPS,
    )?;
    write_zip_entry(&mut writer, options, "ppt/theme/theme1.xml", PPTX_THEME)?;
    for (index, slide) in draft.slides.iter().enumerate() {
        write_zip_entry(
            &mut writer,
            options,
            &format!("ppt/slides/slide{}.xml", index + 1),
            &pptx_slide(slide, index == 0, &draft.title),
        )?;
        write_zip_entry(
            &mut writer,
            options,
            &format!("ppt/slides/_rels/slide{}.xml.rels", index + 1),
            PPTX_SLIDE_RELATIONSHIPS,
        )?;
    }
    let bytes = writer
        .finish()
        .context("artifact_pptx_finish_failed")?
        .into_inner();
    verify_pptx(bytes)
}

#[derive(Debug, Clone)]
struct PdfTextLine {
    text: String,
    x_mm: f32,
    y_mm: f32,
    size_pt: f32,
    color: (f32, f32, f32),
}

#[derive(Debug, Clone, Copy)]
struct PdfTextStyle {
    size_pt: f32,
    line_height_mm: f32,
    max_width_units: usize,
    color: (f32, f32, f32),
}

pub fn render_pdf(draft: &DocumentArtifactDraft) -> Result<RenderedArtifact> {
    use printpdf::{
        Color, Mm, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
        PdfParseErrorSeverity, PdfSaveOptions, Point, Pt, Rect, Rgb, TextItem,
    };

    validate_document_draft(draft)?;
    let mut font_warnings = Vec::new();
    let font = ParsedFont::from_bytes(PDF_FONT_BYTES, 0, &mut font_warnings)
        .ok_or_else(|| anyhow::anyhow!("artifact_pdf_font_invalid"))?;
    if pdf_document_text(draft)
        .chars()
        .filter(|character| !character.is_control())
        .any(|character| font.lookup_glyph_index(character as u32).is_none())
    {
        anyhow::bail!("artifact_pdf_font_missing_glyph");
    }

    let mut lines = vec![Vec::<PdfTextLine>::new()];
    let mut y_mm = 272.0f32;
    push_pdf_text(
        &mut lines,
        &mut y_mm,
        &draft.title,
        PdfTextStyle {
            size_pt: 22.0,
            line_height_mm: 10.0,
            max_width_units: 58,
            color: (0.067, 0.094, 0.153),
        },
    );
    y_mm -= 8.0;
    for section in &draft.sections {
        if y_mm < 40.0 {
            lines.push(Vec::new());
            y_mm = 272.0;
        }
        push_pdf_text(
            &mut lines,
            &mut y_mm,
            &section.heading,
            PdfTextStyle {
                size_pt: 15.0,
                line_height_mm: 7.5,
                max_width_units: 76,
                color: (0.145, 0.388, 0.922),
            },
        );
        y_mm -= 2.0;
        for paragraph in &section.paragraphs {
            push_pdf_text(
                &mut lines,
                &mut y_mm,
                paragraph,
                PdfTextStyle {
                    size_pt: 11.0,
                    line_height_mm: 6.5,
                    max_width_units: 100,
                    color: (0.216, 0.255, 0.32),
                },
            );
            y_mm -= 3.5;
        }
        y_mm -= 3.0;
    }

    let mut document = PdfDocument::new(&draft.title);
    let font_id = document.add_font(&font);
    let pages = lines
        .into_iter()
        .map(|page_lines| {
            let mut operations = vec![
                Op::SetFillColor {
                    col: Color::Rgb(Rgb::new(0.145, 0.388, 0.922, None)),
                },
                Op::DrawRectangle {
                    rectangle: Rect {
                        x: Mm(18.0).into(),
                        y: Mm(18.0).into(),
                        width: Mm(1.5).into(),
                        height: Mm(261.0).into(),
                        mode: Some(PaintMode::Fill),
                        winding_order: None,
                    },
                },
            ];
            for line in page_lines {
                operations.extend([
                    Op::StartTextSection,
                    Op::SetTextCursor {
                        pos: Point::new(Mm(line.x_mm), Mm(line.y_mm)),
                    },
                    Op::SetFont {
                        font: PdfFontHandle::External(font_id.clone()),
                        size: Pt(line.size_pt),
                    },
                    Op::SetFillColor {
                        col: Color::Rgb(Rgb::new(line.color.0, line.color.1, line.color.2, None)),
                    },
                    Op::ShowText {
                        items: vec![TextItem::Text(line.text)],
                    },
                    Op::EndTextSection,
                ]);
            }
            PdfPage::new(Mm(210.0), Mm(297.0), operations)
        })
        .collect::<Vec<_>>();
    document.with_pages(pages);
    let mut warnings = Vec::new();
    let bytes = document.save(&PdfSaveOptions::default(), &mut warnings);
    if warnings
        .iter()
        .any(|warning| warning.severity == PdfParseErrorSeverity::Error)
    {
        anyhow::bail!("artifact_pdf_generation_failed");
    }
    verify_pdf(bytes)
}

fn pdf_document_text(draft: &DocumentArtifactDraft) -> String {
    std::iter::once(draft.title.as_str())
        .chain(draft.sections.iter().flat_map(|section| {
            std::iter::once(section.heading.as_str())
                .chain(section.paragraphs.iter().map(String::as_str))
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_pdf_text(
    pages: &mut Vec<Vec<PdfTextLine>>,
    y_mm: &mut f32,
    text: &str,
    style: PdfTextStyle,
) {
    for line in pdf_wrap_text(text, style.max_width_units) {
        if *y_mm < 24.0 {
            pages.push(Vec::new());
            *y_mm = 272.0;
        }
        pages.last_mut().unwrap().push(PdfTextLine {
            text: line,
            x_mm: 25.0,
            y_mm: *y_mm,
            size_pt: style.size_pt,
            color: style.color,
        });
        *y_mm -= style.line_height_mm;
    }
}

fn pdf_wrap_text(text: &str, max_width_units: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for source_line in text.lines() {
        let mut current = String::new();
        let mut width = 0usize;
        for character in source_line.chars() {
            let character_width = if character.is_ascii() { 1 } else { 2 };
            if width + character_width > max_width_units && !current.is_empty() {
                lines.push(current.trim_end().to_string());
                current.clear();
                width = 0;
            }
            current.push(character);
            width += character_width;
        }
        if !current.is_empty() {
            lines.push(current.trim_end().to_string());
        }
    }
    lines
}

fn verify_docx(bytes: Vec<u8>) -> Result<RenderedArtifact> {
    let extraction = extract_resource(ResourceExtractionRequest {
        filename: "openlife-result.docx".into(),
        declared_mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            .into(),
        bytes: bytes.clone(),
    })
    .context("artifact_docx_verification_failed")?;
    if extraction.format != ResourceFormat::Docx || extraction.chunks.is_empty() {
        anyhow::bail!("artifact_docx_verification_invalid");
    }
    Ok(RenderedArtifact {
        bytes,
        media_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        format: ResourceFormat::Docx,
        verified_chunk_count: extraction.chunks.len(),
        verified_text: extraction
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

fn verify_xlsx(bytes: Vec<u8>) -> Result<RenderedArtifact> {
    let extraction = extract_resource(ResourceExtractionRequest {
        filename: "openlife-result.xlsx".into(),
        declared_mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        bytes: bytes.clone(),
    })
    .context("artifact_xlsx_verification_failed")?;
    if extraction.format != ResourceFormat::Xlsx || extraction.chunks.is_empty() {
        anyhow::bail!("artifact_xlsx_verification_invalid");
    }
    Ok(RenderedArtifact {
        bytes,
        media_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        format: ResourceFormat::Xlsx,
        verified_chunk_count: extraction.chunks.len(),
        verified_text: extraction
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

fn verify_pptx(bytes: Vec<u8>) -> Result<RenderedArtifact> {
    let extraction = extract_resource(ResourceExtractionRequest {
        filename: "openlife-result.pptx".into(),
        declared_mime: "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            .into(),
        bytes: bytes.clone(),
    })
    .context("artifact_pptx_verification_failed")?;
    if extraction.format != ResourceFormat::Pptx || extraction.chunks.is_empty() {
        anyhow::bail!("artifact_pptx_verification_invalid");
    }
    Ok(RenderedArtifact {
        bytes,
        media_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        format: ResourceFormat::Pptx,
        verified_chunk_count: extraction.chunks.len(),
        verified_text: extraction
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

fn verify_pdf(bytes: Vec<u8>) -> Result<RenderedArtifact> {
    let extraction = extract_resource(ResourceExtractionRequest {
        filename: "openlife-result.pdf".into(),
        declared_mime: "application/pdf".into(),
        bytes: bytes.clone(),
    })
    .context("artifact_pdf_verification_failed")?;
    if extraction.format != ResourceFormat::Pdf || extraction.chunks.is_empty() {
        anyhow::bail!("artifact_pdf_verification_invalid");
    }
    Ok(RenderedArtifact {
        bytes,
        media_type: "application/pdf",
        format: ResourceFormat::Pdf,
        verified_chunk_count: extraction.chunks.len(),
        verified_text: extraction
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

fn validate_document_draft(draft: &DocumentArtifactDraft) -> Result<()> {
    if draft.title.trim().is_empty()
        || draft.title.chars().count() > 512
        || draft.sections.is_empty()
        || draft.sections.len() > MAX_DOCUMENT_SECTIONS
    {
        anyhow::bail!("artifact_document_shape_invalid");
    }
    let mut paragraph_count = 0usize;
    let mut text_chars = draft.title.chars().count();
    for section in &draft.sections {
        if section.heading.trim().is_empty()
            || section.heading.chars().count() > 512
            || section.paragraphs.is_empty()
        {
            anyhow::bail!("artifact_document_section_invalid");
        }
        paragraph_count = paragraph_count
            .checked_add(section.paragraphs.len())
            .ok_or_else(|| anyhow::anyhow!("artifact_document_paragraph_count_overflow"))?;
        text_chars = text_chars
            .checked_add(section.heading.chars().count())
            .ok_or_else(|| anyhow::anyhow!("artifact_document_text_count_overflow"))?;
        for paragraph in &section.paragraphs {
            if paragraph.trim().is_empty() || paragraph.chars().count() > 16_384 {
                anyhow::bail!("artifact_document_paragraph_invalid");
            }
            text_chars = text_chars
                .checked_add(paragraph.chars().count())
                .ok_or_else(|| anyhow::anyhow!("artifact_document_text_count_overflow"))?;
        }
    }
    if paragraph_count > MAX_DOCUMENT_PARAGRAPHS || text_chars > MAX_DOCUMENT_TEXT_CHARS {
        anyhow::bail!("artifact_document_size_exceeded");
    }
    Ok(())
}

fn validate_spreadsheet_draft(draft: &SpreadsheetArtifactDraft) -> Result<()> {
    if draft.sheets.is_empty() || draft.sheets.len() > MAX_SPREADSHEET_SHEETS {
        anyhow::bail!("artifact_spreadsheet_shape_invalid");
    }
    let mut names = std::collections::HashSet::new();
    let mut cell_count = 0usize;
    for sheet in &draft.sheets {
        let name = sheet.name.trim();
        if name.is_empty()
            || name.chars().count() > 31
            || name
                .chars()
                .any(|ch| matches!(ch, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
            || !names.insert(name.to_lowercase())
            || sheet.headers.is_empty()
            || sheet.headers.len() > MAX_SPREADSHEET_COLUMNS
            || sheet.rows.len() > MAX_SPREADSHEET_ROWS
        {
            anyhow::bail!("artifact_spreadsheet_sheet_invalid");
        }
        for row in std::iter::once(&sheet.headers).chain(sheet.rows.iter()) {
            if row.len() != sheet.headers.len()
                || row
                    .iter()
                    .any(|cell| cell.chars().count() > 32_767 || cell.contains('\0'))
            {
                anyhow::bail!("artifact_spreadsheet_row_invalid");
            }
            cell_count = cell_count
                .checked_add(row.len())
                .ok_or_else(|| anyhow::anyhow!("artifact_spreadsheet_cell_count_overflow"))?;
        }
    }
    if cell_count > MAX_SPREADSHEET_CELLS {
        anyhow::bail!("artifact_spreadsheet_size_exceeded");
    }
    Ok(())
}

fn validate_presentation_draft(draft: &PresentationArtifactDraft) -> Result<()> {
    if draft.title.trim().is_empty()
        || draft.title.chars().count() > 512
        || draft.slides.is_empty()
        || draft.slides.len() > MAX_PRESENTATION_SLIDES
    {
        anyhow::bail!("artifact_presentation_shape_invalid");
    }
    let mut bullets = 0usize;
    for slide in &draft.slides {
        if slide.title.trim().is_empty()
            || slide.title.chars().count() > 512
            || slide.bullets.is_empty()
            || slide
                .bullets
                .iter()
                .any(|bullet| bullet.trim().is_empty() || bullet.chars().count() > 4_096)
        {
            anyhow::bail!("artifact_presentation_slide_invalid");
        }
        bullets = bullets
            .checked_add(slide.bullets.len())
            .ok_or_else(|| anyhow::anyhow!("artifact_presentation_bullet_count_overflow"))?;
    }
    if bullets > MAX_PRESENTATION_BULLETS {
        anyhow::bail!("artifact_presentation_size_exceeded");
    }
    Ok(())
}

fn document_xml(draft: &DocumentArtifactDraft) -> String {
    let mut body = String::new();
    push_paragraph(&mut body, &draft.title, Some("Title"));
    for section in &draft.sections {
        push_paragraph(&mut body, &section.heading, Some("Heading1"));
        for paragraph in &section.paragraphs {
            push_paragraph(&mut body, paragraph, None);
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

fn push_paragraph(output: &mut String, text: &str, style: Option<&str>) {
    output.push_str("<w:p>");
    if let Some(style) = style {
        output.push_str("<w:pPr><w:pStyle w:val=\"");
        output.push_str(style);
        output.push_str("\"/></w:pPr>");
    }
    output.push_str(
        "<w:r><w:rPr><w:rFonts w:ascii=\"Arial Unicode MS\" w:hAnsi=\"Arial Unicode MS\" w:eastAsia=\"Arial Unicode MS\" w:cs=\"Arial Unicode MS\"/></w:rPr><w:t xml:space=\"preserve\">",
    );
    output.push_str(&escape_xml(text));
    output.push_str("</w:t></w:r></w:p>");
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xlsx_content_types(sheet_count: usize) -> String {
    let mut overrides = String::new();
    for index in 1..=sheet_count {
        overrides.push_str(&format!(r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#
    )
}

fn xlsx_workbook(draft: &SpreadsheetArtifactDraft) -> String {
    let sheets = draft
        .sheets
        .iter()
        .enumerate()
        .map(|(index, sheet)| {
            format!(
                r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
                escape_xml(sheet.name.trim()),
                index + 1,
                index + 1
            )
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{sheets}</sheets></workbook>"#
    )
}

fn xlsx_workbook_relationships(sheet_count: usize) -> String {
    let mut relationships = String::new();
    for index in 1..=sheet_count {
        relationships.push_str(&format!(r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{index}.xml"/>"#));
    }
    relationships.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#, sheet_count + 1));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn xlsx_sheet(sheet: &SpreadsheetSheetDraft) -> String {
    let columns = sheet
        .headers
        .iter()
        .enumerate()
        .map(|(column_index, header)| {
            let width = std::iter::once(header)
                .chain(sheet.rows.iter().filter_map(|row| row.get(column_index)))
                .map(|cell| {
                    cell.chars()
                        .map(|character| if character.is_ascii() { 1usize } else { 2usize })
                        .sum::<usize>()
                })
                .max()
                .unwrap_or(0)
                .saturating_add(2)
                .clamp(10, 48);
            format!(
                r#"<col min="{column}" max="{column}" width="{width}" customWidth="1"/>"#,
                column = column_index + 1
            )
        })
        .collect::<String>();
    let rows = std::iter::once(&sheet.headers)
        .chain(sheet.rows.iter())
        .enumerate()
        .map(|(row_index, row)| {
            let row_number = row_index + 1;
            let style = if row_index == 0 { 1 } else { 2 };
            let cells = row
                .iter()
                .enumerate()
                .map(|(column_index, cell)| {
                    format!(
                        r#"<c r="{}{}" s="{style}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                        spreadsheet_column_name(column_index + 1),
                        row_number,
                        escape_xml(cell)
                    )
                })
                .collect::<String>();
            if row_index == 0 {
                format!(r#"<row r="{row_number}" ht="24" customHeight="1">{cells}</row>"#)
            } else {
                format!(r#"<row r="{row_number}" ht="21" customHeight="1">{cells}</row>"#)
            }
        })
        .collect::<String>();
    let last_column = spreadsheet_column_name(sheet.headers.len());
    let last_row = sheet.rows.len() + 1;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="21"/><cols>{columns}</cols><sheetData>{rows}</sheetData><autoFilter ref="A1:{last_column}{last_row}"/></worksheet>"#
    )
}

fn spreadsheet_column_name(mut column: usize) -> String {
    let mut result = String::new();
    while column > 0 {
        column -= 1;
        result.insert(0, (b'A' + (column % 26) as u8) as char);
        column /= 26;
    }
    result
}

fn pptx_content_types(slide_count: usize) -> String {
    let mut overrides = String::new();
    for index in 1..=slide_count {
        overrides.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>{overrides}</Types>"#
    )
}

fn pptx_presentation(slide_count: usize) -> String {
    let slides = (1..=slide_count)
        .map(|index| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + index, index + 1))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{slides}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000" type="screen16x9"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle/></p:presentation>"#
    )
}

fn pptx_presentation_relationships(slide_count: usize) -> String {
    let mut relationships = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for index in 1..=slide_count {
        relationships.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#, index + 1));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn pptx_slide(slide: &PresentationSlideDraft, first: bool, deck_title: &str) -> String {
    let title = if first { deck_title } else { &slide.title };
    let mut paragraphs = String::new();
    if first && slide.title.trim() != deck_title.trim() {
        paragraphs.push_str(&pptx_text_paragraph(&slide.title, false));
    }
    for bullet in &slide.bullets {
        paragraphs.push_str(&pptx_text_paragraph(bullet, true));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="4" name="Accent"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="365760"/><a:ext cx="91440" cy="5486400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="2563EB"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="914400" y="548640"/><a:ext cx="10515600" cy="1143000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="2800" b="1"><a:solidFill><a:srgbClr val="111827"/></a:solidFill><a:latin typeface="Arial Unicode MS"/><a:ea typeface="Arial Unicode MS"/></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="1143000" y="1828800"/><a:ext cx="9829800" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
        escape_xml(title)
    )
}

fn pptx_text_paragraph(text: &str, bullet: bool) -> String {
    let paragraph_properties = if bullet {
        r#"<a:pPr lvl="0"><a:buChar char="•"/></a:pPr>"#
    } else {
        "<a:pPr/>"
    };
    format!(
        r#"<a:p>{paragraph_properties}<a:r><a:rPr lang="zh-CN" sz="1800"><a:solidFill><a:srgbClr val="374151"/></a:solidFill><a:latin typeface="Arial Unicode MS"/><a:ea typeface="Arial Unicode MS"/></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p>"#,
        escape_xml(text)
    )
}

fn write_zip_entry(
    writer: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
    options: zip::write::SimpleFileOptions,
    path: &str,
    content: &str,
) -> Result<()> {
    writer
        .start_file(path, options)
        .with_context(|| format!("artifact_docx_entry_start_failed:{path}"))?;
    writer
        .write_all(content.as_bytes())
        .with_context(|| format!("artifact_docx_entry_write_failed:{path}"))?;
    Ok(())
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;

const ROOT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOCUMENT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="160" w:line="336" w:lineRule="auto"/></w:pPr><w:rPr><w:rFonts w:ascii="Arial Unicode MS" w:hAnsi="Arial Unicode MS" w:eastAsia="Arial Unicode MS" w:cs="Arial Unicode MS"/><w:color w:val="374151"/><w:sz w:val="22"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="0" w:after="360"/></w:pPr><w:rPr><w:rFonts w:ascii="Arial Unicode MS" w:hAnsi="Arial Unicode MS" w:eastAsia="Arial Unicode MS" w:cs="Arial Unicode MS"/><w:b/><w:color w:val="111827"/><w:sz w:val="40"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="320" w:after="120"/></w:pPr><w:rPr><w:rFonts w:ascii="Arial Unicode MS" w:hAnsi="Arial Unicode MS" w:eastAsia="Arial Unicode MS" w:cs="Arial Unicode MS"/><w:b/><w:color w:val="2563EB"/><w:sz w:val="28"/></w:rPr></w:style></w:styles>"#;

const XLSX_ROOT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const XLSX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><color rgb="FF374151"/><name val="Arial Unicode MS"/><family val="2"/><charset val="134"/></font><font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Arial Unicode MS"/><family val="2"/><charset val="134"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF2563EB"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="2"><border/><border><left/><right/><top/><bottom style="thin"><color rgb="FFD1D5DB"/></bottom><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1" applyAlignment="1"><alignment vertical="center"/></xf><xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyFont="1" applyBorder="1" applyAlignment="1"><alignment vertical="center" wrapText="1"/></xf></cellXfs></styleSheet>"#;

const PPTX_ROOT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#;

const PPTX_SLIDE_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#;

const PPTX_SLIDE_MASTER_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;

const PPTX_SLIDE_LAYOUT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;

const PPTX_SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/><p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#;

const PPTX_SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;

const PPTX_THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="OpenLife"><a:themeElements><a:clrScheme name="OpenLife"><a:dk1><a:srgbClr val="111827"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="374151"/></a:dk2><a:lt2><a:srgbClr val="F3F4F6"/></a:lt2><a:accent1><a:srgbClr val="2563EB"/></a:accent1><a:accent2><a:srgbClr val="059669"/></a:accent2><a:accent3><a:srgbClr val="D97706"/></a:accent3><a:accent4><a:srgbClr val="7C3AED"/></a:accent4><a:accent5><a:srgbClr val="DB2777"/></a:accent5><a:accent6><a:srgbClr val="0891B2"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme><a:fontScheme name="OpenLife"><a:majorFont><a:latin typeface="Arial Unicode MS"/><a:ea typeface="Arial Unicode MS"/><a:cs typeface="Arial Unicode MS"/></a:majorFont><a:minorFont><a:latin typeface="Arial Unicode MS"/><a:ea typeface="Arial Unicode MS"/><a:cs typeface="Arial Unicode MS"/></a:minorFont></a:fontScheme><a:fmtScheme name="OpenLife"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_render_round_trips_unicode_and_xml_characters() {
        let rendered = render_docx(&DocumentArtifactDraft {
            title: "OpenLife 研究简报".into(),
            sections: vec![DocumentArtifactSection {
                heading: "结论 & 风险".into(),
                paragraphs: vec!["内容来自已授权资料；2 < 3，且不执行外部动作。".into()],
            }],
        })
        .unwrap();
        assert_eq!(rendered.format, ResourceFormat::Docx);
        assert_eq!(rendered.verified_chunk_count, 3);
        assert!(rendered.bytes.starts_with(b"PK"));
    }

    #[test]
    fn docx_render_rejects_empty_or_unbounded_content() {
        let empty = DocumentArtifactDraft {
            title: "".into(),
            sections: vec![],
        };
        assert!(render_docx(&empty).is_err());

        let too_many = DocumentArtifactDraft {
            title: "Title".into(),
            sections: (0..=MAX_DOCUMENT_SECTIONS)
                .map(|index| DocumentArtifactSection {
                    heading: format!("Section {index}"),
                    paragraphs: vec!["Body".into()],
                })
                .collect(),
        };
        assert!(render_docx(&too_many).is_err());
    }

    #[test]
    fn xlsx_render_round_trips_multiple_sheets_as_literal_cells() {
        let rendered = render_xlsx(&SpreadsheetArtifactDraft {
            sheets: vec![
                SpreadsheetSheetDraft {
                    name: "Summary".into(),
                    headers: vec!["Metric".into(), "Value".into()],
                    rows: vec![vec!["Status".into(), "Ready & verified".into()]],
                },
                SpreadsheetSheetDraft {
                    name: "风险".into(),
                    headers: vec!["Item".into(), "Detail".into()],
                    rows: vec![vec![
                        "Formula".into(),
                        "=WEBSERVICE(\"https://example.com\")".into(),
                    ]],
                },
            ],
        })
        .unwrap();
        assert_eq!(rendered.format, ResourceFormat::Xlsx);
        assert!(rendered.verified_chunk_count >= 2);
        assert!(rendered.bytes.starts_with(b"PK"));
    }

    #[test]
    fn xlsx_render_rejects_bad_shape_and_sheet_names() {
        let draft = SpreadsheetArtifactDraft {
            sheets: vec![SpreadsheetSheetDraft {
                name: "bad/name".into(),
                headers: vec!["A".into(), "B".into()],
                rows: vec![vec!["only one".into()]],
            }],
        };
        assert!(render_xlsx(&draft).is_err());
    }

    #[test]
    fn pptx_render_round_trips_slide_text() {
        let rendered = render_pptx(&PresentationArtifactDraft {
            title: "OpenLife 发布简报".into(),
            slides: vec![
                PresentationSlideDraft {
                    title: "概览".into(),
                    bullets: vec!["可信的通用个人 Agent".into()],
                },
                PresentationSlideDraft {
                    title: "结论".into(),
                    bullets: vec!["主链已验证".into(), "仍需原生复核".into()],
                },
            ],
        })
        .unwrap();
        assert_eq!(rendered.format, ResourceFormat::Pptx);
        assert_eq!(rendered.verified_chunk_count, 2);
        assert!(rendered.bytes.starts_with(b"PK"));
    }

    #[test]
    fn pdf_render_embeds_a_subset_font_and_round_trips_chinese_text() {
        let rendered = render_pdf(&DocumentArtifactDraft {
            title: "OpenLife 项目简报".into(),
            sections: vec![
                DocumentArtifactSection {
                    heading: "结论".into(),
                    paragraphs: vec!["核心能力已通过文件级核验。".into()],
                },
                DocumentArtifactSection {
                    heading: "下一步".into(),
                    paragraphs: vec!["继续完成正式应用中的视觉验收。".into()],
                },
            ],
        })
        .unwrap();

        assert_eq!(rendered.format, ResourceFormat::Pdf);
        assert_eq!(rendered.media_type, "application/pdf");
        assert!(rendered.bytes.starts_with(b"%PDF-"));
        assert!(
            rendered.bytes.len() < 2 * 1024 * 1024,
            "subset PDF remained {} bytes",
            rendered.bytes.len()
        );
        assert!(rendered.verified_text.contains("OpenLife 项目简报"));
        assert!(rendered.verified_text.contains("核心能力已通过文件级核验"));

        let parsed = lopdf::Document::load_mem(&rendered.bytes).unwrap();
        assert!(parsed
            .objects
            .values()
            .filter_map(|object| object.as_dict().ok())
            .any(|dictionary| {
                dictionary.get(b"FontFile2").is_ok() || dictionary.get(b"FontFile3").is_ok()
            }));
        if let Some(directory) = std::env::var_os("OPENLIFE_PDF_EVIDENCE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("verified.pdf"), &rendered.bytes).unwrap();
        }
    }

    #[test]
    fn pdf_render_fails_closed_when_the_bundled_font_lacks_a_glyph() {
        let result = render_pdf(&DocumentArtifactDraft {
            title: "Emoji 😀".into(),
            sections: vec![DocumentArtifactSection {
                heading: "结论".into(),
                paragraphs: vec!["不能静默丢失字符。".into()],
            }],
        });

        assert_eq!(
            result.unwrap_err().to_string(),
            "artifact_pdf_font_missing_glyph"
        );
    }

    #[test]
    fn pdf_render_paginates_long_documents_without_dropping_tail_text() {
        let paragraphs = (1..=72)
            .map(|index| format!("第 {index} 项核验结论：文件内容、字体和分页必须保持可读取。"))
            .collect::<Vec<_>>();
        let rendered = render_pdf(&DocumentArtifactDraft {
            title: "OpenLife 多页核验报告".into(),
            sections: vec![DocumentArtifactSection {
                heading: "核验记录".into(),
                paragraphs,
            }],
        })
        .unwrap();

        let parsed = lopdf::Document::load_mem(&rendered.bytes).unwrap();
        assert!(parsed.get_pages().len() >= 3);
        assert!(rendered.verified_text.contains("第 1 项核验结论"));
        assert!(rendered.verified_text.contains("第 72 项核验结论"));
        assert!(
            rendered.bytes.len() <= 100 * 1024,
            "representative multi-page PDF exceeded the canonical write boundary: {} bytes",
            rendered.bytes.len()
        );
        if let Some(directory) = std::env::var_os("OPENLIFE_PDF_EVIDENCE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("verified-multipage.pdf"), &rendered.bytes).unwrap();
        }
    }

    #[test]
    fn office_packages_declare_an_explicit_east_asian_font() {
        fn entry(bytes: Vec<u8>, path: &str) -> String {
            use std::io::Read;

            let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut content = String::new();
            archive
                .by_name(path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();
            content
        }

        let docx = render_docx(&DocumentArtifactDraft {
            title: "中文文档".into(),
            sections: vec![DocumentArtifactSection {
                heading: "结论".into(),
                paragraphs: vec!["可视文本".into()],
            }],
        })
        .unwrap();
        assert!(entry(docx.bytes, "word/styles.xml").contains("w:eastAsia=\"Arial Unicode MS\""));

        let xlsx = render_xlsx(&SpreadsheetArtifactDraft {
            sheets: vec![SpreadsheetSheetDraft {
                name: "数据".into(),
                headers: vec!["项目".into()],
                rows: vec![vec!["可用".into()]],
            }],
        })
        .unwrap();
        assert!(entry(xlsx.bytes, "xl/styles.xml").contains("<name val=\"Arial Unicode MS\"/>"));

        let pptx = render_pptx(&PresentationArtifactDraft {
            title: "中文演示".into(),
            slides: vec![PresentationSlideDraft {
                title: "结论".into(),
                bullets: vec!["可视文本".into()],
            }],
        })
        .unwrap();
        assert!(entry(pptx.bytes, "ppt/theme/theme1.xml")
            .contains("<a:ea typeface=\"Arial Unicode MS\"/>"));
    }

    #[test]
    fn office_packages_include_basic_readability_affordances() {
        fn entry(bytes: Vec<u8>, path: &str) -> String {
            use std::io::Read;

            let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut content = String::new();
            archive
                .by_name(path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();
            content
        }

        let docx = render_docx(&DocumentArtifactDraft {
            title: "季度总结".into(),
            sections: vec![DocumentArtifactSection {
                heading: "结论".into(),
                paragraphs: vec!["核心指标保持稳定。".into()],
            }],
        })
        .unwrap();
        let styles = entry(docx.bytes, "word/styles.xml");
        assert!(styles.contains("w:line=\"336\""));
        assert!(styles.contains("w:color w:val=\"2563EB\""));

        let xlsx = render_xlsx(&SpreadsheetArtifactDraft {
            sheets: vec![SpreadsheetSheetDraft {
                name: "指标".into(),
                headers: vec!["项目".into(), "状态".into()],
                rows: vec![vec!["交付".into(), "完成".into()]],
            }],
        })
        .unwrap();
        let styles = entry(xlsx.bytes.clone(), "xl/styles.xml");
        let sheet = entry(xlsx.bytes, "xl/worksheets/sheet1.xml");
        assert!(styles.contains("<cellXfs count=\"3\">"));
        assert!(sheet.contains("state=\"frozen\""));
        assert!(sheet.contains("<autoFilter ref=\"A1:B2\"/>"));
        assert!(sheet.contains("<cols>"));
        assert!(sheet.contains("<c r=\"A1\" s=\"1\""));

        let pptx = render_pptx(&PresentationArtifactDraft {
            title: "季度总结".into(),
            slides: vec![PresentationSlideDraft {
                title: "结论".into(),
                bullets: vec!["核心指标保持稳定。".into()],
            }],
        })
        .unwrap();
        let slide = entry(pptx.bytes, "ppt/slides/slide1.xml");
        assert!(slide.contains("<p:bg>"));
        assert!(slide.contains("val=\"2563EB\""));
        assert!(slide.contains("name=\"Accent\""));
    }

    #[test]
    #[ignore = "requires OPENLIFE_SOFFICE_BIN pointing to a native LibreOffice executable"]
    fn rendered_office_artifacts_open_in_native_office_engine() {
        let executable = std::env::var("OPENLIFE_SOFFICE_BIN")
            .expect("OPENLIFE_SOFFICE_BIN must point to soffice");
        let temporary_directory = tempfile::tempdir().unwrap();
        let evidence_directory = std::env::var_os("OPENLIFE_OFFICE_EVIDENCE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| temporary_directory.path().to_path_buf());
        std::fs::create_dir_all(&evidence_directory).unwrap();
        let artifacts = [
            (
                "verified.docx",
                render_docx(&DocumentArtifactDraft {
                    title: "OpenLife 验证".into(),
                    sections: vec![DocumentArtifactSection {
                        heading: "结论".into(),
                        paragraphs: vec!["DOCX 可由 Office 引擎加载。".into()],
                    }],
                })
                .unwrap()
                .bytes,
            ),
            (
                "verified.xlsx",
                render_xlsx(&SpreadsheetArtifactDraft {
                    sheets: vec![SpreadsheetSheetDraft {
                        name: "验证".into(),
                        headers: vec!["项目".into(), "状态".into()],
                        rows: vec![vec!["XLSX".into(), "可加载".into()]],
                    }],
                })
                .unwrap()
                .bytes,
            ),
            (
                "verified.pptx",
                render_pptx(&PresentationArtifactDraft {
                    title: "OpenLife 验证".into(),
                    slides: vec![PresentationSlideDraft {
                        title: "结论".into(),
                        bullets: vec!["PPTX 可由 Office 引擎加载。".into()],
                    }],
                })
                .unwrap()
                .bytes,
            ),
        ];
        for (file_name, bytes) in artifacts {
            let input = evidence_directory.join(file_name);
            let output = evidence_directory.join(format!("out-{file_name}"));
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(&input, bytes).unwrap();
            let profile = evidence_directory.join("libreoffice-profile");
            let mut command = std::process::Command::new(&executable);
            command.arg(format!(
                "-env:UserInstallation=file://{}",
                profile.to_string_lossy()
            ));
            if let Some(font_directory) = std::env::var_os("OPENLIFE_OFFICE_FONT_DIR") {
                command.env("SAL_FONTPATH", font_directory);
            }
            let result = command
                .args([
                    "--headless",
                    "--convert-to",
                    "pdf",
                    "--outdir",
                    output.to_str().unwrap(),
                    input.to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{file_name} failed Office validation: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(output.join("verified.pdf").exists());
        }
    }
}
