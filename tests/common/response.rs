#[derive(Debug, serde::Deserialize)]
pub struct HueClipResponse<T> {
    pub data: Vec<T>,
}
