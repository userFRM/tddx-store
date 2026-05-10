//! Read a slice of rows from any local parquet file as JSON-friendly
//! `serde_json::Value`s. Used by the desktop "Sample" tab.
//!
//! We deliberately keep this separate from the Arrow → file writer in
//! `format.rs` — preview is read-only, doesn't care about partitioning,
//! and runs against arbitrary user-supplied paths.

use std::fs::File;
use std::path::Path;

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

#[derive(Debug, serde::Serialize)]
pub struct PreviewResult {
    pub schema: Vec<PreviewField>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total_rows: u64,
    pub returned: usize,
    pub bytes: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct PreviewField {
    pub name: String,
    pub dtype: String,
    pub nullable: bool,
}

/// Read up to `limit` rows starting at `offset` (best-effort — we honour
/// row-group boundaries).
pub fn preview(path: &Path, offset: usize, limit: usize) -> crate::Result<PreviewResult> {
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file = File::open(path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let total_rows = builder.metadata().file_metadata().num_rows().max(0) as u64;
    let arrow_schema = builder.schema().clone();
    let mut reader = builder.with_batch_size(2_048).build()?;

    let mut skipped = 0usize;
    let mut out_rows: Vec<Vec<serde_json::Value>> = Vec::new();
    'outer: for batch in reader.by_ref() {
        let batch = batch?;
        let n = batch.num_rows();
        if skipped + n <= offset {
            skipped += n;
            continue;
        }
        let start = offset.saturating_sub(skipped);
        for row in start..n {
            if out_rows.len() >= limit {
                break 'outer;
            }
            let mut cells: Vec<serde_json::Value> = Vec::with_capacity(batch.num_columns());
            for col_idx in 0..batch.num_columns() {
                let col = batch.column(col_idx);
                cells.push(arrow_array_value_to_json(col, row));
            }
            out_rows.push(cells);
        }
        skipped += n;
    }

    let schema: Vec<PreviewField> = arrow_schema
        .fields()
        .iter()
        .map(|f| PreviewField {
            name: f.name().clone(),
            dtype: format!("{:?}", f.data_type()),
            nullable: f.is_nullable(),
        })
        .collect();

    Ok(PreviewResult {
        schema,
        returned: out_rows.len(),
        rows: out_rows,
        total_rows,
        bytes,
    })
}

/// Best-effort cell → JSON converter. Falls back to "Debug" string for
/// unsupported arrow types so the viewer never crashes — the user gets
/// SOMETHING back even for exotic columns.
fn arrow_array_value_to_json(arr: &dyn arrow_array::Array, row: usize) -> serde_json::Value {
    use arrow_array::cast::AsArray;
    use arrow_schema::DataType;
    if arr.is_null(row) {
        return serde_json::Value::Null;
    }
    match arr.data_type() {
        DataType::Boolean => serde_json::Value::Bool(arr.as_boolean().value(row)),
        DataType::Int8 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Int8Type>()
            .value(row)),
        DataType::Int16 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Int16Type>()
            .value(row)),
        DataType::Int32 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Int32Type>()
            .value(row)),
        DataType::Int64 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Int64Type>()
            .value(row)),
        DataType::UInt8 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::UInt8Type>()
            .value(row)),
        DataType::UInt16 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::UInt16Type>()
            .value(row)),
        DataType::UInt32 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::UInt32Type>()
            .value(row)),
        DataType::UInt64 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::UInt64Type>()
            .value(row)),
        DataType::Float32 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Float32Type>()
            .value(row)),
        DataType::Float64 => serde_json::json!(arr
            .as_primitive::<arrow_array::types::Float64Type>()
            .value(row)),
        DataType::Utf8 => serde_json::Value::String(arr.as_string::<i32>().value(row).to_string()),
        DataType::LargeUtf8 => {
            serde_json::Value::String(arr.as_string::<i64>().value(row).to_string())
        }
        _ => serde_json::Value::String(format!("{:?}", arr.slice(row, 1))),
    }
}
