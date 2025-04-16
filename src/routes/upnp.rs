use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use hyper::HeaderMap;
use hyper::header::CONTENT_TYPE;
use url::Url;
use uuid::Uuid;

use crate::error::ApiResult;
use crate::model::upnp;
use crate::server::appstate::AppState;

async fn description_xml(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let mac = state.api_short_config().await.mac;
    let config = &state.config().bridge;
    let ip = config.ipaddress;
    let port = config.http_port;

    let url_base = Url::parse(&format!("http://{ip}:{port}/"))?;
    let friendly_name = format!("Bifrost {ip}");
    let manufacturer = "Christian Iversen";
    let model_name = "Bifrost Bridge";
    let udn = Uuid::new_v5(&Uuid::NAMESPACE_OID, &mac.bytes());

    let device = upnp::Device::new(friendly_name, manufacturer, model_name, udn)
        .with_manufacturer_url(Url::parse("http://github.com/chrivers/bifrost")?)
        .with_model_description("Bifrost Hue Bridge Emulator")
        .with_model_number(hue::HUE_BRIDGE_V2_MODEL_ID)
        .with_model_url(Url::parse("http://www.philips-hue.com")?)
        .with_serial_number(hex::encode(mac.bytes()))
        .with_presentation_url("index.html");

    let root = upnp::Root::new(url_base, device);

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/xml".parse().unwrap());
    Ok((headers, upnp::to_xml(&root)?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(description_xml))
}
