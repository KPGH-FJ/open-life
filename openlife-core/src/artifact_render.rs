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
    output.push_str("<w:r><w:t xml:space=\"preserve\">");
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
    let rows = std::iter::once(&sheet.headers)
        .chain(sheet.rows.iter())
        .enumerate()
        .map(|(row_index, row)| {
            let row_number = row_index + 1;
            let cells = row
                .iter()
                .enumerate()
                .map(|(column_index, cell)| {
                    format!(
                        r#"<c r="{}{}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                        spreadsheet_column_name(column_index + 1),
                        row_number,
                        escape_xml(cell)
                    )
                })
                .collect::<String>();
            format!(r#"<row r="{row_number}">{cells}</row>"#)
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#
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
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="457200"/><a:ext cx="10820400" cy="1143000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="2800" b="1"/><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="914400" y="1828800"/><a:ext cx="10058400" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
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
        r#"<a:p>{paragraph_properties}<a:r><a:rPr lang="zh-CN" sz="1800"/><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p>"#,
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
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:rPr><w:b/><w:sz w:val="36"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style></w:styles>"#;

const XLSX_ROOT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const XLSX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Aptos"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs></styleSheet>"#;

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
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="OpenLife"><a:themeElements><a:clrScheme name="OpenLife"><a:dk1><a:srgbClr val="111827"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="374151"/></a:dk2><a:lt2><a:srgbClr val="F3F4F6"/></a:lt2><a:accent1><a:srgbClr val="2563EB"/></a:accent1><a:accent2><a:srgbClr val="059669"/></a:accent2><a:accent3><a:srgbClr val="D97706"/></a:accent3><a:accent4><a:srgbClr val="7C3AED"/></a:accent4><a:accent5><a:srgbClr val="DB2777"/></a:accent5><a:accent6><a:srgbClr val="0891B2"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme><a:fontScheme name="OpenLife"><a:majorFont><a:latin typeface="Aptos Display"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="OpenLife"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#;

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
    #[ignore = "requires OPENLIFE_SOFFICE_BIN pointing to a native LibreOffice executable"]
    fn rendered_office_artifacts_open_in_native_office_engine() {
        let executable = std::env::var("OPENLIFE_SOFFICE_BIN")
            .expect("OPENLIFE_SOFFICE_BIN must point to soffice");
        let directory = tempfile::tempdir().unwrap();
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
            let input = directory.path().join(file_name);
            let output = directory.path().join(format!("out-{file_name}"));
            std::fs::create_dir(&output).unwrap();
            std::fs::write(&input, bytes).unwrap();
            let result = std::process::Command::new(&executable)
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
