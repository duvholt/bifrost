mod bridge;
mod clock;
mod error;
pub mod fixture;
mod hue_client;
mod response;
mod z2m;

pub use bridge::{TestBridge, TestLight, TestRoom, TestZ2mDeviceOrGroup, TestZone};
pub use error::{TestError, TestResult};
pub use hue_client::HueClient;
pub use response::HueClipResponse;
pub use z2m::{TestZ2m, TestZ2mBackend, Z2mRequests, create_m2m_service};

pub fn init() {
    let _ = pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .is_test(true)
        .try_init();
}
