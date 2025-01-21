use crate::hue::api::MirekSchema;
use crate::z2m::api::ExposeNumeric;

impl ExposeNumeric {
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn extract_mirek_schema(&self) -> Option<MirekSchema> {
        if self.unit.as_deref() == Some("mired") {
            if let (Some(min), Some(max)) = (self.value_min, self.value_max) {
                return Some(MirekSchema {
                    mirek_minimum: min as u32,
                    mirek_maximum: max as u32,
                });
            }
        }
        None
    }
}
