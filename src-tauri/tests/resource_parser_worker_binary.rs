use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const WORKER_ARG: &str = "--openlife-resource-parser-worker-v1";
const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn invoke_worker(filename: &str, declared_mime: &str, bytes: &[u8]) -> Output {
    let header = serde_json::to_vec(&json!({
        "filename": filename,
        "declaredMime": declared_mime,
        "byteCount": bytes.len(),
    }))
    .expect("encode worker request header");
    let mut child = Command::new(env!("CARGO_BIN_EXE_openlife-tauri"))
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the real OpenLife Tauri parser-worker binary");
    {
        let stdin = child.stdin.as_mut().expect("worker stdin");
        stdin
            .write_all(&(header.len() as u32).to_be_bytes())
            .expect("write worker header length");
        stdin.write_all(&header).expect("write worker header");
        stdin.write_all(bytes).expect("write worker body");
    }
    child.wait_with_output().expect("wait for parser worker")
}

fn frozen_xlsx() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-fixtures/resources/metrics.xlsx");
    std::fs::read(path).expect("read frozen RC-03 XLSX fixture")
}

fn frozen_resource(filename: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../test-fixtures/resources")
        .join(filename);
    std::fs::read(path).expect("read frozen resource fixture")
}

#[test]
fn rc03_real_binary_worker_preserves_xlsx_sheet_range_and_untrusted_formula_data() {
    let output = invoke_worker("metrics.xlsx", XLSX_MIME, &frozen_xlsx());
    assert!(
        output.status.success(),
        "worker process failed: status={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "worker stderr must stay empty");
    let response: Value = serde_json::from_slice(&output.stdout).expect("decode worker response");
    assert_eq!(response["status"], "success");
    assert_eq!(response["extraction"]["format"], "xlsx");
    assert_eq!(response["extraction"]["detectedMime"], XLSX_MIME);
    let chunks = response["extraction"]["chunks"]
        .as_array()
        .expect("worker XLSX chunks");
    assert!(
        chunks.iter().any(|chunk| {
            chunk["provenance"]["kind"] == "xlsx"
                && chunk["provenance"]["sheet"] == "roadshow_metrics"
                && chunk["provenance"]["range"]
                    .as_str()
                    .is_some_and(|range| range.starts_with("A1:"))
        }),
        "worker lost RC-03 XLSX sheet/range provenance: {response}"
    );
    assert!(
        chunks.iter().any(|chunk| chunk["content"]
            .as_str()
            .is_some_and(|content| content.contains("RESOURCE_ROW_SENTINEL"))),
        "worker lost the frozen anomaly row"
    );
    assert!(
        chunks.iter().any(|chunk| chunk["content"]
            .as_str()
            .is_some_and(|content| content.contains("=WEBSERVICE(\"http://127.0.0.1\")"))),
        "formula-shaped input must remain inert cell data in parser output"
    );
    assert!(
        response.get("toolCall").is_none()
            && response.get("networkRequest").is_none()
            && response.get("proposal").is_none(),
        "the parser protocol may return extraction facts only"
    );
}

#[test]
fn real_binary_worker_returns_typed_failure_for_corrupt_xlsx_without_echoing_bytes() {
    let corrupt = b"PK\x03\x04RC03_PRIVATE_CORRUPT_XLSX_BODY";
    let output = invoke_worker("metrics.xlsx", XLSX_MIME, corrupt);
    assert!(
        output.status.success(),
        "typed parser failures use a successful protocol exit"
    );
    assert!(output.stderr.is_empty(), "worker stderr must stay empty");
    let response: Value = serde_json::from_slice(&output.stdout).expect("decode worker failure");
    assert_eq!(response["status"], "failure");
    assert!(response["code"]
        .as_str()
        .is_some_and(|code| code.starts_with("resource_")));
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("RC03_PRIVATE_CORRUPT_XLSX_BODY"),
        "typed worker failures must not echo input bytes"
    );
}

#[test]
fn real_binary_worker_extracts_pdf_pages_and_docx_paragraphs() {
    let pdf_output = invoke_worker(
        "combined-report.pdf",
        "application/pdf",
        &frozen_resource("combined-report.pdf"),
    );
    assert!(pdf_output.status.success());
    assert!(pdf_output.stderr.is_empty());
    let pdf: Value = serde_json::from_slice(&pdf_output.stdout).expect("decode PDF response");
    assert_eq!(pdf["status"], "success");
    assert_eq!(pdf["extraction"]["format"], "pdf");
    assert!(pdf["extraction"]["chunks"]
        .as_array()
        .expect("PDF chunks")
        .iter()
        .any(|chunk| {
            chunk["provenance"]["kind"] == "pdf"
                && chunk["provenance"]["page"] == 1
                && chunk["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("COMBINED_REPORT_PAGE_ONE"))
        }));

    let docx_output = invoke_worker(
        "checklist.docx",
        DOCX_MIME,
        &frozen_resource("checklist.docx"),
    );
    assert!(docx_output.status.success());
    assert!(docx_output.stderr.is_empty());
    let docx: Value = serde_json::from_slice(&docx_output.stdout).expect("decode DOCX response");
    assert_eq!(docx["status"], "success");
    assert_eq!(docx["extraction"]["format"], "docx");
    assert!(docx["extraction"]["chunks"]
        .as_array()
        .expect("DOCX chunks")
        .iter()
        .any(|chunk| {
            chunk["provenance"]["kind"] == "docx"
                && chunk["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("ROADSHOW_CHECKLIST_SENTINEL"))
        }));
}
