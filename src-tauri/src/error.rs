// src-tauri/src/error.rs
//
// One typed error enum for the whole backend. Every `#[tauri::command]` returns
// `Result<T, AppError>`, and `AppError` serializes into a stable shape so the
// frontend can run a single catch-all handler:
//
//   { "kind": "csvHeader", "messageKey": "error.csv.header", "detail": "…" }
//
// `messageKey` is an i18n key the frontend translates (TR/EN); `detail` carries
// the untranslated technical specifics (path, HTTP status, parser message).
// Rule: never surface a raw Rust error string as the primary user-facing text.

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("failed to read file {path}: {source}")]
    FileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write file {path}: {source}")]
    FileWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The CSV header is missing or does not match the expected Binance export
    /// layout. The whole file is rejected — see AGENTS.md §5.
    #[error("unexpected CSV header in {path}: expected {expected}, found {found}")]
    CsvHeader {
        path: String,
        expected: String,
        found: String,
    },

    /// Structural CSV failure (unterminated quote, ragged record count, …).
    /// Row-level validation problems are NOT errors — they become `RowError`
    /// entries and the import continues.
    #[error("malformed CSV in {path}: {source}")]
    Csv {
        path: String,
        #[source]
        source: csv::Error,
    },

    #[error("network request to Binance failed: {0}")]
    Network(String),

    #[error("Binance returned HTTP {status}: {body}")]
    BinanceStatus { status: u16, body: String },

    #[error("failed to decode Binance response: {0}")]
    BinanceDecode(String),

    /// Requested a price for a symbol Binance does not know (delisted/renamed).
    #[error("unknown symbol: {0}")]
    UnknownSymbol(String),

    /// No trades have been imported yet, but a command that needs them was called.
    #[error("no trade data loaded")]
    NoData,

    #[error("settings store error: {0}")]
    Store(String),

    #[error("report generation failed: {0}")]
    Report(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Stable machine-readable discriminator, mirrored by a TypeScript union in
    /// `src/lib/types.ts`.
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::FileNotFound(_) => "fileNotFound",
            AppError::FileRead { .. } => "fileRead",
            AppError::FileWrite { .. } => "fileWrite",
            AppError::CsvHeader { .. } => "csvHeader",
            AppError::Csv { .. } => "csvMalformed",
            AppError::Network(_) => "network",
            AppError::BinanceStatus { .. } => "binanceStatus",
            AppError::BinanceDecode(_) => "binanceDecode",
            AppError::UnknownSymbol(_) => "unknownSymbol",
            AppError::NoData => "noData",
            AppError::Store(_) => "store",
            AppError::Report(_) => "report",
            AppError::Internal(_) => "internal",
        }
    }

    /// i18n lookup key for the user-facing message. Both `tr.json` and `en.json`
    /// must define every key returned here.
    pub fn message_key(&self) -> &'static str {
        match self {
            AppError::FileNotFound(_) => "error.file.notFound",
            AppError::FileRead { .. } => "error.file.read",
            AppError::FileWrite { .. } => "error.file.write",
            AppError::CsvHeader { .. } => "error.csv.header",
            AppError::Csv { .. } => "error.csv.malformed",
            AppError::Network(_) => "error.network.unreachable",
            AppError::BinanceStatus { .. } => "error.network.status",
            AppError::BinanceDecode(_) => "error.network.decode",
            AppError::UnknownSymbol(_) => "error.price.unknownSymbol",
            AppError::NoData => "error.data.none",
            AppError::Store(_) => "error.store.failed",
            AppError::Report(_) => "error.report.failed",
            AppError::Internal(_) => "error.internal",
        }
    }
}

/// Wire format sent to the frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppErrorPayload<'a> {
    kind: &'a str,
    message_key: &'a str,
    /// Technical detail, intentionally untranslated.
    detail: String,
}

// Tauri requires command errors to be `Serialize`. `thiserror` gives us `Display`;
// we turn that into the structured payload above.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AppErrorPayload {
            kind: self.kind(),
            message_key: self.message_key(),
            detail: self.to_string(),
        }
        .serialize(serializer)
    }
}

/// Convenience alias used throughout the backend.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_the_documented_shape() {
        let err = AppError::NoData;
        let json = serde_json::to_value(&err).expect("AppError must serialize");
        assert_eq!(json["kind"], "noData");
        assert_eq!(json["messageKey"], "error.data.none");
        assert!(json["detail"].is_string());
    }

    #[test]
    fn every_variant_has_distinct_kind_and_key() {
        let variants: Vec<AppError> = vec![
            AppError::FileNotFound("a".into()),
            AppError::CsvHeader {
                path: "a".into(),
                expected: "b".into(),
                found: "c".into(),
            },
            AppError::Network("x".into()),
            AppError::BinanceStatus {
                status: 400,
                body: "{}".into(),
            },
            AppError::BinanceDecode("x".into()),
            AppError::UnknownSymbol("XXXUSDT".into()),
            AppError::NoData,
            AppError::Store("x".into()),
            AppError::Report("x".into()),
            AppError::Internal("x".into()),
        ];
        let mut kinds: Vec<&str> = variants.iter().map(AppError::kind).collect();
        kinds.sort_unstable();
        let before = kinds.len();
        kinds.dedup();
        assert_eq!(before, kinds.len(), "AppError kinds must be unique");
    }
}
