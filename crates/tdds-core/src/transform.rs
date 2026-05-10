//! Post-decode RecordBatch transforms applied right before the file
//! writer touches disk:
//!
//!   - rename columns (e.g. `strike` -> `strike_dollars`)
//!   - drop columns the caller doesn't want
//!   - rescale numeric columns by a constant (the canonical use is the
//!     "strike unit" toggle: thetadatadx 9.x emits `strike` in dollars
//!     as `f64`; users who keep older parquet have it in thousands of
//!     dollars as `i32`. A scale of `0.001` undoes that, `1000.0` does
//!     the inverse. Auto-detection lives in the UI; the backend only
//!     applies what it's told.)
//!
//! All transforms are no-ops by default — `Transforms::default()`
//! returns the input batch untouched.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{Array, Float64Array, Int32Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transforms {
    /// Per-column scale factor. Multiplied into numeric columns of any
    /// numeric dtype; non-numeric columns are left alone.
    #[serde(default)]
    pub scale: BTreeMap<String, f64>,
    /// Column renames: old_name -> new_name. Applied after scaling so the
    /// scale key matches the original column name.
    #[serde(default)]
    pub rename: BTreeMap<String, String>,
    /// Columns to omit from the final batch.
    #[serde(default)]
    pub drop: Vec<String>,
}

impl Transforms {
    pub fn is_noop(&self) -> bool {
        self.scale.is_empty() && self.rename.is_empty() && self.drop.is_empty()
    }

    /// Apply the transforms to a `RecordBatch`. Returns a new
    /// `RecordBatch` with mutated schema + columns, leaving the input
    /// untouched.
    pub fn apply(&self, batch: &RecordBatch) -> crate::Result<RecordBatch> {
        if self.is_noop() {
            return Ok(batch.clone());
        }
        let mut fields: Vec<Field> = Vec::with_capacity(batch.num_columns());
        let mut columns: Vec<Arc<dyn Array>> = Vec::with_capacity(batch.num_columns());

        let drop_set: std::collections::HashSet<&str> =
            self.drop.iter().map(String::as_str).collect();

        for (idx, field) in batch.schema().fields().iter().enumerate() {
            let orig_name = field.name();
            if drop_set.contains(orig_name.as_str()) {
                continue;
            }
            let array = batch.column(idx);
            let scaled = self.scale_one(orig_name, array, field.data_type())?;
            let new_name = self
                .rename
                .get(orig_name)
                .cloned()
                .unwrap_or_else(|| orig_name.clone());
            // After scaling the dtype may change (i32 -> f64 when the
            // scale isn't an integer). Use the scaled array's dtype.
            let new_field = Field::new(&new_name, scaled.data_type().clone(), field.is_nullable());
            fields.push(new_field);
            columns.push(scaled);
        }
        let schema = Arc::new(Schema::new(fields));
        Ok(RecordBatch::try_new(schema, columns)?)
    }

    /// Scale `array` if the user requested a factor for `name`. Casts
    /// integer columns to `f64` when the scale isn't 1.0.
    fn scale_one(
        &self,
        name: &str,
        array: &Arc<dyn Array>,
        dtype: &DataType,
    ) -> crate::Result<Arc<dyn Array>> {
        let Some(&factor) = self.scale.get(name) else {
            return Ok(array.clone());
        };
        if (factor - 1.0).abs() < f64::EPSILON {
            return Ok(array.clone());
        }
        match dtype {
            DataType::Float64 => {
                let a = array
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| crate::Error::Other(format!("scale: bad f64 col {name}")))?;
                let new: Float64Array = a.iter().map(|v| v.map(|x| x * factor)).collect();
                Ok(Arc::new(new))
            }
            DataType::Int32 => {
                let a = array
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .ok_or_else(|| crate::Error::Other(format!("scale: bad i32 col {name}")))?;
                let new: Float64Array = a.iter().map(|v| v.map(|x| x as f64 * factor)).collect();
                Ok(Arc::new(new))
            }
            DataType::Int64 => {
                let a = array
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .ok_or_else(|| crate::Error::Other(format!("scale: bad i64 col {name}")))?;
                let new: Float64Array = a.iter().map(|v| v.map(|x| x as f64 * factor)).collect();
                Ok(Arc::new(new))
            }
            _ => Ok(array.clone()),
        }
    }
}
