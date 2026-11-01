mod fonts;
mod layout;

use serde::Deserialize;

pub use layout::render;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PdfStyle {
    Registro,
    Tavola,
    Banco,
    Listino,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PdfTone {
    Plain,
    Muted,
    High,
    Medium,
    Low,
    Ok,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct PdfTally {
    pub value: String,
    pub label: String,
    pub tone: PdfTone,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PdfBlock {
    Heading {
        text: String,
        count: Option<String>,
        tone: PdfTone,
    },
    Paragraph {
        text: String,
        tone: PdfTone,
    },
    List {
        items: Vec<String>,
        tone: PdfTone,
    },
    Finding {
        severity: String,
        tone: PdfTone,
        category: String,
        title: String,
        what: String,
        subjects_label: String,
        subjects: Vec<String>,
        fix_label: String,
        fix: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct PdfReport {
    pub style: PdfStyle,
    pub language: String,
    pub title: String,
    pub subtitle: String,
    pub lead: Option<String>,
    pub tally: Vec<PdfTally>,
    pub blocks: Vec<PdfBlock>,
    pub footer: String,
    pub pages: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("the PDF could not be written: {0}")]
    Write(String),
}
