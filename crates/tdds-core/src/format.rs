//! Convert a slice of typed thetadatadx ticks into one of the four output
//! formats. All formats are written zstd-compressed where applicable.

use std::fs::File;
use std::path::Path;

use arrow_array::RecordBatch;
use arrow_csv::WriterBuilder as CsvWriterBuilder;
use arrow_json::ArrayWriter as JsonArrayWriter;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
pub use thetadatadx::frames::TicksArrowExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Parquet,
    Csv,
    Json,
    Jsonl,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Parquet => "parquet",
            OutputFormat::Csv => "csv",
            OutputFormat::Json => "json",
            OutputFormat::Jsonl => "jsonl",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "parquet" | "pq" => OutputFormat::Parquet,
            "csv" => OutputFormat::Csv,
            "json" => OutputFormat::Json,
            "jsonl" | "ndjson" => OutputFormat::Jsonl,
            _ => return None,
        })
    }
}

pub fn write_batch(batch: &RecordBatch, path: &Path, fmt: OutputFormat) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: stage to `<path>.partial`, only `rename` to the
    // final path if every step succeeded. A crash mid-write therefore
    // leaves a `.partial` file the worker will overwrite next time
    // instead of a truncated parquet that the on-disk-check would
    // mistake for "already done".
    let mut tmp = path.to_path_buf();
    tmp.as_mut_os_string().push(".partial");
    let result = match fmt {
        OutputFormat::Parquet => write_parquet(batch, &tmp),
        OutputFormat::Csv => write_csv(batch, &tmp),
        OutputFormat::Json => write_json(batch, &tmp),
        OutputFormat::Jsonl => write_jsonl(batch, &tmp),
    };
    match result {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp, path) {
                let _ = std::fs::remove_file(&tmp);
                return Err(e.into());
            }
            Ok(())
        }
        Err(e) => {
            // Best-effort cleanup so a future rerun starts fresh.
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn write_parquet(batch: &RecordBatch, path: &Path) -> crate::Result<()> {
    // ZstdLevel::try_new(3) is infallible for [1,22] — a panic here would
    // kill the worker silently. Map the (impossible) error so the worker
    // sees an Err and reports it cleanly.
    let zstd =
        ZstdLevel::try_new(3).map_err(|e| crate::Error::Other(format!("zstd level: {e}")))?;
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(zstd))
        .set_dictionary_enabled(true)
        .build();
    let f = File::create(path)?;
    let mut w = ArrowWriter::try_new(f, batch.schema(), Some(props))?;
    w.write(batch)?;
    w.close()?;
    Ok(())
}

fn write_csv(batch: &RecordBatch, path: &Path) -> crate::Result<()> {
    let f = File::create(path)?;
    let mut w = CsvWriterBuilder::new().with_header(true).build(f);
    w.write(batch)
        .map_err(|e| crate::Error::Other(format!("csv: {e}")))?;
    Ok(())
}

fn write_json(batch: &RecordBatch, path: &Path) -> crate::Result<()> {
    let f = File::create(path)?;
    let mut w = JsonArrayWriter::new(f);
    w.write(batch)
        .map_err(|e| crate::Error::Other(format!("json: {e}")))?;
    w.finish()
        .map_err(|e| crate::Error::Other(format!("json finish: {e}")))?;
    Ok(())
}

fn write_jsonl(batch: &RecordBatch, path: &Path) -> crate::Result<()> {
    let f = File::create(path)?;
    let mut w = arrow_json::LineDelimitedWriter::new(f);
    w.write(batch)
        .map_err(|e| crate::Error::Other(format!("jsonl: {e}")))?;
    w.finish()
        .map_err(|e| crate::Error::Other(format!("jsonl finish: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use arrow_array::Int32Array;
    use arrow_schema::{Field, Schema};

    fn sample_batch() -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new(
                "value",
                arrow_schema::DataType::Int32,
                false,
            )])),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3]))],
        )
        .unwrap()
    }

    #[test]
    fn write_batch_cleans_partial_file_when_rename_fails() {
        let root = std::env::temp_dir().join(format!("tdds-format-tests-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let final_path = root.join("already-a-directory");
        std::fs::create_dir_all(&final_path).unwrap();

        let err = write_batch(&sample_batch(), &final_path, OutputFormat::Json).unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("directory"));

        let mut partial_path = final_path.clone();
        partial_path.as_mut_os_string().push(".partial");
        assert!(!partial_path.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }
}
