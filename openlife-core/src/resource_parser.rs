//! Bounded, provenance-preserving extraction for imported resources.
//!
//! These parsers never execute document content. The process-isolation wrapper
//! lives in `resource_gateway`; this module is deliberately synchronous so the
//! worker process can be killed instead of leaving detached blocking work.

use crate::resource::{
    ResourceChunkDraft, ResourceFormat, ResourceProvenance, MAX_CHUNKS_PER_RESOURCE,
    MAX_CHUNK_CHARS, MAX_RESOURCE_BYTES,
};
use anyhow::{Context, Result};
use calamine::{Data, Reader, Xlsx};
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::path::Path;

pub const MAX_EXPANDED_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_COMPRESSION_RATIO: u64 = 20;
pub const MAX_ZIP_ENTRIES: usize = 2_048;
pub const MAX_PDF_PAGES: usize = 300;
pub const MAX_XLSX_SHEETS: usize = 20;
pub const MAX_SPREADSHEET_CELLS: usize = 100_000;

const TARGET_CHUNK_CHARS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceExtractionRequest {
    pub filename: String,
    pub declared_mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceExtraction {
    pub detected_mime: String,
    pub format: ResourceFormat,
    pub expanded_bytes: u64,
    pub chunks: Vec<ResourceChunkDraft>,
}

pub fn extract_resource(request: ResourceExtractionRequest) -> Result<ResourceExtraction> {
    validate_request_shape(&request)?;
    let format = classify_format(&request.filename, &request.declared_mime, &request.bytes)?;
    let (detected_mime, expanded_bytes, chunks) = match format {
        ResourceFormat::Text
        | ResourceFormat::Markdown
        | ResourceFormat::Json
        | ResourceFormat::Source => extract_textual(format, &request.bytes)?,
        ResourceFormat::Pdf => extract_pdf(&request.bytes)?,
        ResourceFormat::Docx => extract_docx(&request.bytes)?,
        ResourceFormat::Csv => extract_csv(&request.bytes)?,
        ResourceFormat::Xlsx => extract_xlsx(&request.bytes)?,
    };
    validate_chunks(&chunks)?;
    Ok(ResourceExtraction {
        detected_mime,
        format,
        expanded_bytes,
        chunks,
    })
}

fn validate_request_shape(request: &ResourceExtractionRequest) -> Result<()> {
    let filename = request.filename.trim();
    if filename.is_empty() || filename.len() > 255 || filename.contains(['/', '\\', '\0']) {
        anyhow::bail!("resource_filename_invalid");
    }
    if request.declared_mime.trim().is_empty() || request.declared_mime.len() > 128 {
        anyhow::bail!("resource_mime_invalid");
    }
    if request.bytes.is_empty() || request.bytes.len() > MAX_RESOURCE_BYTES {
        anyhow::bail!("resource_file_bytes_exceeded");
    }
    Ok(())
}

fn classify_format(filename: &str, declared_mime: &str, bytes: &[u8]) -> Result<ResourceFormat> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| anyhow::anyhow!("resource_extension_missing"))?;
    let declared = declared_mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let magic = infer::get(bytes).map(|kind| kind.mime_type().to_ascii_lowercase());

    let expected = match extension.as_str() {
        "txt" => (ResourceFormat::Text, &["text/plain"][..]),
        "md" | "markdown" => (
            ResourceFormat::Markdown,
            &["text/markdown", "text/plain"][..],
        ),
        "json" => (ResourceFormat::Json, &["application/json", "text/json"][..]),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "h" | "cpp" | "hpp"
        | "swift" | "kt" | "kts" | "toml" | "yaml" | "yml" | "xml" | "html" | "css" | "sql"
        | "sh" => (
            ResourceFormat::Source,
            &[
                "text/plain",
                "text/x-source",
                "application/xml",
                "text/xml",
                "text/html",
                "text/css",
                "application/sql",
                "application/x-sh",
            ][..],
        ),
        "pdf" => (ResourceFormat::Pdf, &["application/pdf"][..]),
        "docx" => (
            ResourceFormat::Docx,
            &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"][..],
        ),
        "csv" => (
            ResourceFormat::Csv,
            &["text/csv", "application/csv", "text/plain"][..],
        ),
        "xlsx" => (
            ResourceFormat::Xlsx,
            &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"][..],
        ),
        _ => anyhow::bail!("resource_format_unsupported"),
    };
    if !expected.1.contains(&declared.as_str()) {
        anyhow::bail!("resource_declared_mime_mismatch");
    }
    match expected.0 {
        ResourceFormat::Pdf if magic.as_deref() != Some("application/pdf") => {
            anyhow::bail!("resource_magic_mime_mismatch")
        }
        ResourceFormat::Docx | ResourceFormat::Xlsx
            if !bytes.starts_with(b"PK\x03\x04") && !bytes.starts_with(b"PK\x05\x06") =>
        {
            anyhow::bail!("resource_magic_mime_mismatch")
        }
        ResourceFormat::Text
        | ResourceFormat::Markdown
        | ResourceFormat::Json
        | ResourceFormat::Source
        | ResourceFormat::Csv
            if magic.as_deref().is_some_and(|mime| {
                mime != "application/json"
                    && mime != "application/xml"
                    && !mime.starts_with("text/")
            }) =>
        {
            anyhow::bail!("resource_magic_mime_mismatch")
        }
        _ => {}
    }
    Ok(expected.0)
}

fn extract_textual(
    format: ResourceFormat,
    bytes: &[u8],
) -> Result<(String, u64, Vec<ResourceChunkDraft>)> {
    let text = std::str::from_utf8(bytes).context("resource_text_not_utf8")?;
    if format == ResourceFormat::Json {
        let _: serde_json::Value = serde_json::from_str(text).context("resource_json_invalid")?;
    }
    let chunks = chunk_lines(text, |start, end| ResourceProvenance::Text {
        start_line: start as u32,
        end_line: end as u32,
    })?;
    let mime = match format {
        ResourceFormat::Markdown => "text/markdown",
        ResourceFormat::Json => "application/json",
        ResourceFormat::Source => "text/x-source",
        _ => "text/plain",
    };
    Ok((mime.to_string(), bytes.len() as u64, chunks))
}

fn extract_pdf(bytes: &[u8]) -> Result<(String, u64, Vec<ResourceChunkDraft>)> {
    let document = lopdf::Document::load_mem(bytes).context("resource_pdf_corrupt")?;
    if document.is_encrypted() {
        anyhow::bail!("resource_pdf_encrypted_unsupported");
    }
    let pages = document.get_pages();
    if pages.is_empty() {
        anyhow::bail!("resource_pdf_has_no_pages");
    }
    if pages.len() > MAX_PDF_PAGES {
        anyhow::bail!("resource_pdf_page_limit_exceeded");
    }
    let mut chunks = Vec::with_capacity(pages.len());
    for page in pages.keys().copied() {
        let text = document
            .extract_text(&[page])
            .with_context(|| format!("resource_pdf_page_extract_failed:{page}"))?;
        let normalized = normalize_extracted_text(&text);
        if !normalized.is_empty() {
            chunks.push(ResourceChunkDraft {
                content: normalized,
                provenance: ResourceProvenance::Pdf { page },
            });
        }
    }
    if chunks.is_empty() {
        anyhow::bail!("resource_pdf_has_no_extractable_text");
    }
    Ok(("application/pdf".to_string(), bytes.len() as u64, chunks))
}

fn extract_docx(bytes: &[u8]) -> Result<(String, u64, Vec<ResourceChunkDraft>)> {
    let mut archive = open_bounded_zip(bytes)?;
    validate_ooxml_safety(&mut archive)?;
    let content_types = read_bounded_zip_entry(&mut archive, "[Content_Types].xml")?;
    require_xml_safe(&content_types)?;
    let content_types_text =
        String::from_utf8(content_types).context("resource_docx_content_types_invalid")?;
    if !content_types_text.contains("wordprocessingml.document.main+xml") {
        anyhow::bail!("resource_docx_content_type_mismatch");
    }
    let document_xml = read_bounded_zip_entry(&mut archive, "word/document.xml")?;
    require_xml_safe(&document_xml)?;
    let paragraphs = extract_docx_paragraphs(&document_xml)?;
    let expanded_bytes = zip_expanded_bytes(&mut archive)?;
    let mut chunks = Vec::new();
    for (ordinal, paragraph) in paragraphs.into_iter().enumerate() {
        let content = normalize_extracted_text(&paragraph);
        if content.is_empty() {
            continue;
        }
        chunks.push(ResourceChunkDraft {
            content,
            provenance: ResourceProvenance::Docx {
                paragraph_start: (ordinal + 1) as u32,
                paragraph_end: (ordinal + 1) as u32,
            },
        });
    }
    if chunks.is_empty() {
        anyhow::bail!("resource_docx_has_no_extractable_text");
    }
    Ok((
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        expanded_bytes,
        chunks,
    ))
}

fn extract_csv(bytes: &[u8]) -> Result<(String, u64, Vec<ResourceChunkDraft>)> {
    let text = std::str::from_utf8(bytes).context("resource_csv_not_utf8")?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .context("resource_csv_header_invalid")?
        .clone();
    if headers.is_empty() {
        anyhow::bail!("resource_csv_empty");
    }
    let mut rows = vec![headers.iter().map(str::to_string).collect::<Vec<_>>()];
    let mut cell_count = headers.len();
    for record in reader.records() {
        let record = record.context("resource_csv_record_invalid")?;
        cell_count = cell_count
            .checked_add(record.len())
            .ok_or_else(|| anyhow::anyhow!("resource_spreadsheet_cell_count_overflow"))?;
        if cell_count > MAX_SPREADSHEET_CELLS {
            anyhow::bail!("resource_spreadsheet_cell_limit_exceeded");
        }
        rows.push(record.iter().map(str::to_string).collect());
    }
    let chunks = table_rows_to_chunks(None, &rows, ResourceFormat::Csv)?;
    Ok(("text/csv".to_string(), bytes.len() as u64, chunks))
}

fn extract_xlsx(bytes: &[u8]) -> Result<(String, u64, Vec<ResourceChunkDraft>)> {
    let mut archive = open_bounded_zip(bytes)?;
    validate_ooxml_safety(&mut archive)?;
    let content_types = read_bounded_zip_entry(&mut archive, "[Content_Types].xml")?;
    require_xml_safe(&content_types)?;
    let content_types_text =
        String::from_utf8(content_types).context("resource_xlsx_content_types_invalid")?;
    if !content_types_text.contains("spreadsheetml.sheet.main+xml") {
        anyhow::bail!("resource_xlsx_content_type_mismatch");
    }
    let expanded_bytes = zip_expanded_bytes(&mut archive)?;
    drop(archive);

    let mut workbook = Xlsx::new(Cursor::new(bytes)).context("resource_xlsx_corrupt")?;
    let sheets = workbook.sheet_names().to_vec();
    if sheets.is_empty() {
        anyhow::bail!("resource_xlsx_empty");
    }
    if sheets.len() > MAX_XLSX_SHEETS {
        anyhow::bail!("resource_xlsx_sheet_limit_exceeded");
    }
    let mut chunks = Vec::new();
    let mut cell_count = 0usize;
    for sheet in sheets {
        let range = workbook
            .worksheet_range(&sheet)
            .with_context(|| format!("resource_xlsx_sheet_invalid:{sheet}"))?;
        let mut rows = Vec::new();
        for row in range.rows() {
            cell_count = cell_count
                .checked_add(row.len())
                .ok_or_else(|| anyhow::anyhow!("resource_spreadsheet_cell_count_overflow"))?;
            if cell_count > MAX_SPREADSHEET_CELLS {
                anyhow::bail!("resource_spreadsheet_cell_limit_exceeded");
            }
            rows.push(row.iter().map(format_xlsx_cell).collect::<Vec<_>>());
        }
        chunks.extend(table_rows_to_chunks(
            Some(&sheet),
            &rows,
            ResourceFormat::Xlsx,
        )?);
    }
    if chunks.is_empty() {
        anyhow::bail!("resource_xlsx_has_no_extractable_cells");
    }
    Ok((
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
        expanded_bytes,
        chunks,
    ))
}

fn format_xlsx_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("#{value:?}"),
    }
}

fn open_bounded_zip(bytes: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).context("resource_ooxml_corrupt_zip")?;
    validate_zip_limits(&mut archive, bytes.len() as u64)?;
    Ok(archive)
}

fn validate_zip_limits<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    compressed_container_bytes: u64,
) -> Result<()> {
    if archive.is_empty() || archive.len() > MAX_ZIP_ENTRIES {
        anyhow::bail!("resource_zip_entry_limit_exceeded");
    }
    let mut expanded = 0u64;
    let mut compressed = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .with_context(|| format!("resource_zip_entry_missing:{index}"))?;
        if entry.encrypted() {
            anyhow::bail!("resource_zip_encrypted_unsupported");
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or_else(|| anyhow::anyhow!("resource_zip_expanded_bytes_overflow"))?;
        compressed = compressed
            .checked_add(entry.compressed_size())
            .ok_or_else(|| anyhow::anyhow!("resource_zip_compressed_bytes_overflow"))?;
    }
    if expanded > MAX_EXPANDED_BYTES {
        anyhow::bail!("resource_zip_expanded_bytes_exceeded");
    }
    let ratio_base = compressed.max(1).min(compressed_container_bytes.max(1));
    if expanded > ratio_base.saturating_mul(MAX_COMPRESSION_RATIO) {
        anyhow::bail!("resource_zip_expansion_ratio_exceeded");
    }
    Ok(())
}

fn zip_expanded_bytes<R: Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>) -> Result<u64> {
    (0..archive.len()).try_fold(0u64, |total, index| {
        let size = archive
            .by_index(index)
            .with_context(|| format!("resource_zip_entry_missing:{index}"))?
            .size();
        total
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("resource_zip_expanded_bytes_overflow"))
    })
}

fn read_bounded_zip_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let entry = archive
        .by_name(name)
        .with_context(|| format!("resource_ooxml_entry_missing:{name}"))?;
    if entry.size() > MAX_EXPANDED_BYTES {
        anyhow::bail!("resource_zip_entry_too_large");
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_EXPANDED_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("resource_ooxml_entry_read_failed:{name}"))?;
    if bytes.len() as u64 > MAX_EXPANDED_BYTES {
        anyhow::bail!("resource_zip_entry_too_large");
    }
    Ok(bytes)
}

fn validate_ooxml_safety<R: Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>) -> Result<()> {
    let xml_names = archive
        .file_names()
        .filter(|name| name.ends_with(".xml") || name.ends_with(".rels"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    for name in xml_names {
        let bytes = read_bounded_zip_entry(archive, &name)?;
        require_xml_safe(&bytes)?;
        if name.ends_with(".rels") {
            let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            if lower.contains("targetmode=\"external\"") || lower.contains("targetmode='external'")
            {
                anyhow::bail!("resource_ooxml_external_relationship_rejected");
            }
        }
    }
    Ok(())
}

fn require_xml_safe(bytes: &[u8]) -> Result<()> {
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") {
        anyhow::bail!("resource_xml_entity_declaration_rejected");
    }
    Ok(())
}

fn extract_docx_paragraphs(bytes: &[u8]) -> Result<Vec<String>> {
    let mut reader = XmlReader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_paragraph = false;
    loop {
        match reader.read_event().context("resource_docx_xml_invalid")? {
            Event::Start(event) if event.local_name().as_ref() == b"p" => {
                in_paragraph = true;
                current.clear();
            }
            Event::Empty(event) if in_paragraph && event.local_name().as_ref() == b"tab" => {
                current.push('\t');
            }
            Event::Text(text) if in_paragraph => {
                current.push_str(&text.decode().context("resource_docx_text_invalid")?);
            }
            Event::End(event) if event.local_name().as_ref() == b"p" => {
                in_paragraph = false;
                paragraphs.push(std::mem::take(&mut current));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(paragraphs)
}

fn chunk_lines(
    text: &str,
    provenance: impl Fn(usize, usize) -> ResourceProvenance,
) -> Result<Vec<ResourceChunkDraft>> {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        anyhow::bail!("resource_text_empty");
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut start = 1usize;
    for (index, line) in lines.iter().enumerate() {
        let next_len =
            current.chars().count() + line.chars().count() + usize::from(!current.is_empty());
        if !current.is_empty() && next_len > TARGET_CHUNK_CHARS {
            chunks.push(ResourceChunkDraft {
                content: std::mem::take(&mut current),
                provenance: provenance(start, index),
            });
            start = index + 1;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        if current.chars().count() > MAX_CHUNK_CHARS {
            anyhow::bail!("resource_line_exceeds_chunk_limit");
        }
    }
    if !current.trim().is_empty() {
        chunks.push(ResourceChunkDraft {
            content: current,
            provenance: provenance(start, lines.len()),
        });
    }
    if chunks.is_empty() {
        anyhow::bail!("resource_text_empty");
    }
    Ok(chunks)
}

fn table_rows_to_chunks(
    sheet: Option<&str>,
    rows: &[Vec<String>],
    format: ResourceFormat,
) -> Result<Vec<ResourceChunkDraft>> {
    if rows.is_empty() {
        anyhow::bail!("resource_spreadsheet_empty");
    }
    let max_columns = rows.iter().map(Vec::len).max().unwrap_or_default();
    if max_columns == 0 {
        anyhow::bail!("resource_spreadsheet_empty");
    }
    let end_column = spreadsheet_column_name(max_columns)?;
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut start_row = 1usize;
    for (index, row) in rows.iter().enumerate() {
        let line = row
            .iter()
            .map(|cell| cell.replace(['\r', '\n', '\t'], " "))
            .collect::<Vec<_>>()
            .join("\t");
        let next_len =
            current.chars().count() + line.chars().count() + usize::from(!current.is_empty());
        if !current.is_empty() && next_len > TARGET_CHUNK_CHARS {
            chunks.push(table_chunk(
                sheet,
                format,
                std::mem::take(&mut current),
                start_row,
                index,
                &end_column,
            ));
            start_row = index + 1;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&line);
        if current.chars().count() > MAX_CHUNK_CHARS {
            anyhow::bail!("resource_spreadsheet_row_exceeds_chunk_limit");
        }
    }
    if !current.trim().is_empty() {
        chunks.push(table_chunk(
            sheet,
            format,
            current,
            start_row,
            rows.len(),
            &end_column,
        ));
    }
    Ok(chunks)
}

fn table_chunk(
    sheet: Option<&str>,
    format: ResourceFormat,
    content: String,
    start_row: usize,
    end_row: usize,
    end_column: &str,
) -> ResourceChunkDraft {
    let range = format!("A{start_row}:{end_column}{end_row}");
    let provenance = match format {
        ResourceFormat::Csv => ResourceProvenance::Csv { range },
        ResourceFormat::Xlsx => ResourceProvenance::Xlsx {
            sheet: sheet.unwrap_or_default().to_string(),
            range,
        },
        _ => unreachable!("table chunk is only constructed for CSV/XLSX"),
    };
    ResourceChunkDraft {
        content,
        provenance,
    }
}

fn spreadsheet_column_name(column_count: usize) -> Result<String> {
    if column_count == 0 || column_count > 16_384 {
        anyhow::bail!("resource_spreadsheet_column_limit_exceeded");
    }
    let mut value = column_count;
    let mut name = Vec::new();
    while value > 0 {
        value -= 1;
        name.push((b'A' + (value % 26) as u8) as char);
        value /= 26;
    }
    name.reverse();
    Ok(name.into_iter().collect())
}

fn validate_chunks(chunks: &[ResourceChunkDraft]) -> Result<()> {
    if chunks.is_empty() || chunks.len() > MAX_CHUNKS_PER_RESOURCE {
        anyhow::bail!("resource_chunk_count_exceeded");
    }
    if chunks.iter().any(|chunk| {
        chunk.content.trim().is_empty() || chunk.content.chars().count() > MAX_CHUNK_CHARS
    }) {
        anyhow::bail!("resource_chunk_content_invalid");
    }
    Ok(())
}

fn normalize_extracted_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plans/fixtures/openlife_roadshow_core")
                .join(name),
        )
        .unwrap()
    }

    fn docx_with_external_relationship() -> Vec<u8> {
        let source = fixture("roadshow_compare.docx");
        let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
        let output = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(output);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "_rels/.rels" {
                let text = String::from_utf8(bytes).unwrap().replace(
                    "Target=\"word/document.xml\"",
                    "Target=\"https://example.com/payload\" TargetMode=\"External\"",
                );
                bytes = text.into_bytes();
            }
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn expansion_bomb_docx() -> Vec<u8> {
        let output = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(output);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(&vec![b'A'; 1024 * 1024]).unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn encrypted_pdf() -> Vec<u8> {
        let mut document = lopdf::Document::load_mem(&fixture("roadshow_compare.pdf")).unwrap();
        document.trailer.set(
            "ID",
            lopdf::Object::Array(vec![
                lopdf::Object::string_literal("openlife-roadshow-file-id"),
                lopdf::Object::string_literal("openlife-roadshow-file-id"),
            ]),
        );
        let encryption = lopdf::EncryptionVersion::V2 {
            document: &document,
            owner_password: "roadshow-owner",
            user_password: "roadshow-user",
            key_length: 128,
            permissions: lopdf::Permissions::all(),
        };
        let state = lopdf::EncryptionState::try_from(encryption).unwrap();
        document.encrypt(&state).unwrap();
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn frozen_pdf_and_docx_preserve_page_and_paragraph_provenance() {
        let pdf = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.pdf".to_string(),
            declared_mime: "application/pdf".to_string(),
            bytes: fixture("roadshow_compare.pdf"),
        })
        .unwrap();
        assert!(pdf
            .chunks
            .iter()
            .any(|chunk| matches!(chunk.provenance, ResourceProvenance::Pdf { page: 1 })));
        assert!(pdf
            .chunks
            .iter()
            .any(|chunk| matches!(chunk.provenance, ResourceProvenance::Pdf { page: 2 })));

        let docx = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.docx".to_string(),
            declared_mime:
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            bytes: fixture("roadshow_compare.docx"),
        })
        .unwrap();
        assert!(docx.chunks.len() >= 2);
        assert!(docx
            .chunks
            .iter()
            .all(|chunk| matches!(chunk.provenance, ResourceProvenance::Docx { .. })));
    }

    #[test]
    fn frozen_csv_and_xlsx_preserve_range_and_sheet_provenance() {
        let csv = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_metrics.csv".to_string(),
            declared_mime: "text/csv".to_string(),
            bytes: fixture("roadshow_metrics.csv"),
        })
        .unwrap();
        assert!(csv.chunks.iter().any(|chunk| matches!(
            &chunk.provenance,
            ResourceProvenance::Csv { range } if range.starts_with("A1:")
        )));

        let xlsx = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_metrics.xlsx".to_string(),
            declared_mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .to_string(),
            bytes: fixture("roadshow_metrics.xlsx"),
        })
        .unwrap();
        assert!(xlsx.chunks.iter().any(|chunk| matches!(
            &chunk.provenance,
            ResourceProvenance::Xlsx { sheet, range }
                if sheet == "roadshow_metrics" && range.starts_with("A1:")
        )));
        assert!(xlsx
            .chunks
            .iter()
            .any(|chunk| chunk.content.contains("RESOURCE_ROW_SENTINEL")));
        assert!(xlsx
            .chunks
            .iter()
            .any(|chunk| chunk.content.contains("=WEBSERVICE(\"http://127.0.0.1\")")));
    }

    #[test]
    fn wrong_mime_and_corrupt_ooxml_fail_closed() {
        let wrong_mime = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.pdf".to_string(),
            declared_mime: "text/plain".to_string(),
            bytes: fixture("roadshow_compare.pdf"),
        })
        .unwrap_err();
        assert!(wrong_mime.to_string().contains("declared_mime_mismatch"));

        let corrupt = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.docx".to_string(),
            declared_mime:
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            bytes: b"PK\x03\x04not-a-valid-archive".to_vec(),
        })
        .unwrap_err();
        assert!(corrupt.to_string().contains("corrupt_zip"));

        let external_relationship = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.docx".to_string(),
            declared_mime:
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            bytes: docx_with_external_relationship(),
        })
        .unwrap_err();
        assert!(external_relationship
            .to_string()
            .contains("external_relationship_rejected"));

        let expansion = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.docx".to_string(),
            declared_mime:
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            bytes: expansion_bomb_docx(),
        })
        .unwrap_err();
        assert!(expansion.to_string().contains("expansion_ratio_exceeded"));

        let encrypted = extract_resource(ResourceExtractionRequest {
            filename: "roadshow_compare.pdf".to_string(),
            declared_mime: "application/pdf".to_string(),
            bytes: encrypted_pdf(),
        })
        .unwrap_err();
        assert!(encrypted.to_string().contains("encrypted_unsupported"));
    }
}
