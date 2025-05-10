mod backend_event;
mod bridge_event;
pub mod entertainment;
pub mod learn;
pub mod websocket;
pub mod zclcommand;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use maplit::btreeset;
use native_tls::TlsConnector;
use serde_json::json;
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;
use tokio_tungstenite::{Connector, connect_async_tls_with_config};
use uuid::Uuid;

use bifrost_api::backend::BackendRequest;
use hue::api::{
    BridgeHome, Button, ButtonData, ButtonMetadata, ButtonReport, DeviceArchetype,
    DeviceProductData, Entertainment, EntertainmentSegment, EntertainmentSegments, GroupedLight,
    Light, LightEffect, LightEffects, LightEffectsV2, LightEffectsV2Update, LightGradientMode,
    LightMetadata, LightUpdate, Metadata, RType, Resource, ResourceLink, Room, RoomArchetype,
    RoomMetadata, Scene, SceneActive, SceneMetadata, SceneRecall, SceneStatus, Stub, Taurus,
    ZigbeeConnectivity, ZigbeeConnectivityStatus,
};
use hue::clamp::Clamp;
use hue::scene_icons;
use hue::zigbee::{EffectType, GradientParams, GradientStyle, HueZigbeeUpdate};
use z2m::api::ExposeLight;
use z2m::convert::{
    ExtractColorTemperature, ExtractDeviceProductData, ExtractDimming, ExtractLightColor,
    ExtractLightGradient,
};
use z2m::update::DeviceUpdate;

use crate::backend::Backend;
use crate::backend::z2m::entertainment::EntStream;
use crate::backend::z2m::learn::SceneLearn;
use crate::backend::z2m::websocket::Z2mWebSocket;
use crate::config::{AppConfig, Z2mServer};
use crate::error::{ApiError, ApiResult};
use crate::model::state::AuxData;
use crate::model::throttle::Throttle;
use crate::resource::Resources;

pub struct Z2mBackend {
    name: String,
    server: Z2mServer,
    config: Arc<AppConfig>,
    state: Arc<Mutex<Resources>>,
    map: HashMap<String, ResourceLink>,
    rmap: HashMap<ResourceLink, String>,
    learner: SceneLearn,
    ignore: HashSet<String>,
    network: HashMap<String, z2m::api::Device>,
    entstream: Option<EntStream>,
    counter: u32,
    fps: u32,
    throttle: Throttle,

    // for sending delayed messages over the websocket
    message_rx: mpsc::UnboundedReceiver<(String, DeviceUpdate)>,
    message_tx: mpsc::UnboundedSender<(String, DeviceUpdate)>,
}

impl Z2mBackend {
    const DEFAULT_FPS: u32 = 20;
    const LIGHT_BREATHE_DURATION: Duration = Duration::from_secs(2);

    pub fn new(
        name: String,
        server: Z2mServer,
        config: Arc<AppConfig>,
        state: Arc<Mutex<Resources>>,
    ) -> ApiResult<Self> {
        let fps = server.streaming_fps.map_or(Self::DEFAULT_FPS, u32::from);
        let map = HashMap::new();
        let rmap = HashMap::new();
        let ignore = HashSet::new();
        let learner = SceneLearn::new(name.clone());
        let network = HashMap::new();
        let entstream = None;
        let throttle = Throttle::from_fps(fps);
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        Ok(Self {
            name,
            server,
            config,
            state,
            map,
            rmap,
            learner,
            ignore,
            network,
            entstream,
            throttle,
            fps,
            message_rx,
            message_tx,
            counter: 0,
        })
    }

    pub async fn add_light(
        &mut self,
        apidev: &z2m::api::Device,
        expose: &ExposeLight,
    ) -> ApiResult<()> {
        let name = &apidev.friendly_name;

        let link_device = RType::Device.deterministic(&apidev.ieee_address);
        let link_light = RType::Light.deterministic(&apidev.ieee_address);
        let link_enttm = RType::Entertainment.deterministic(&apidev.ieee_address);
        let link_taurus = RType::Taurus.deterministic(&apidev.ieee_address);
        let link_zigcon = RType::ZigbeeConnectivity.deterministic(&apidev.ieee_address);

        let product_data = DeviceProductData::guess_from_device(apidev);
        let metadata = LightMetadata::new(product_data.product_archetype.clone(), name);

        let effects =
            apidev.manufacturer.as_deref() == Some(DeviceProductData::SIGNIFY_MANUFACTURER_NAME);
        let gradient = apidev.expose_gradient();

        let dev = hue::api::Device {
            product_data,
            metadata: metadata.clone().into(),
            services: btreeset![link_zigcon, link_light, link_enttm, link_taurus],
            identify: Some(Stub),
            usertest: None,
        };

        self.map.insert(name.to_string(), link_light);
        self.rmap.insert(link_device, name.to_string());
        self.rmap.insert(link_light, name.to_string());

        let mut light = Light::new(link_device, metadata);

        light.dimming = expose
            .feature("brightness")
            .and_then(ExtractDimming::extract_from_expose);
        log::trace!("Detected dimming: {:?}", &light.dimming);

        light.color_temperature = expose
            .feature("color_temp")
            .and_then(ExtractColorTemperature::extract_from_expose);
        log::trace!("Detected color temperature: {:?}", &light.color_temperature);

        light.color = expose
            .feature("color_xy")
            .and_then(ExtractLightColor::extract_from_expose);
        log::trace!("Detected color: {:?}", &light.color);

        light.gradient = gradient.and_then(ExtractLightGradient::extract_from_expose);
        log::trace!("Detected gradient support: {:?}", &light.gradient);

        if effects {
            log::trace!("Detected Hue light: enabling effects");
            light.effects = Some(LightEffects::all());
            light.effects_v2 = Some(LightEffectsV2::all());
        }

        let segments = if gradient.is_some() {
            EntertainmentSegments {
                configurable: false,
                max_segments: 10,
                segments: (0..7)
                    .map(|x| EntertainmentSegment {
                        start: x,
                        length: 1,
                    })
                    .collect(),
            }
        } else {
            EntertainmentSegments {
                configurable: false,
                max_segments: 1,
                segments: vec![EntertainmentSegment {
                    start: 0,
                    length: 1,
                }],
            }
        };

        // FIXME: This should be feature-detected, not always enabled
        let enttm = Entertainment {
            equalizer: true,
            owner: link_device,
            proxy: true,
            renderer: true,
            max_streams: None,
            renderer_reference: Some(link_light),
            segments: Some(segments),
        };

        // FIXME: The Taurus objects are seen on Hue Entertainment devices on a
        // real hue bridge, but nobody knows what it does. Some clients seem to
        // want them present, though.
        let taurus = Taurus {
            capabilities: vec![
                "sensor".to_string(),
                "collector".to_string(),
                "sync".to_string(),
            ],
            owner: link_device,
        };

        let zigcon = ZigbeeConnectivity {
            channel: None,
            extended_pan_id: None,
            mac_address: apidev.ieee_address.to_string(),
            owner: link_device,
            status: ZigbeeConnectivityStatus::Connected,
        };

        let mut res = self.state.lock().await;
        res.aux_set(&link_light, AuxData::new().with_topic(name));
        res.add(&link_device, Resource::Device(dev))?;
        res.add(&link_light, Resource::Light(light))?;
        res.add(&link_enttm, Resource::Entertainment(enttm))?;
        res.add(&link_taurus, Resource::Taurus(taurus))?;
        res.add(&link_zigcon, Resource::ZigbeeConnectivity(zigcon))?;
        drop(res);

        Ok(())
    }

    pub async fn add_switch(&mut self, dev: &z2m::api::Device) -> ApiResult<()> {
        let name = &dev.friendly_name;

        let link_device = RType::Device.deterministic(&dev.ieee_address);
        let link_button = RType::Button.deterministic(&dev.ieee_address);
        let link_zbc = RType::ZigbeeConnectivity.deterministic(&dev.ieee_address);

        let dev = hue::api::Device {
            product_data: DeviceProductData::guess_from_device(dev),
            metadata: Metadata::new(DeviceArchetype::UnknownArchetype, "foo"),
            services: btreeset![link_button, link_zbc],
            identify: None,
            usertest: None,
        };

        self.map.insert(name.to_string(), link_button);
        self.rmap.insert(link_button, name.to_string());

        let mut res = self.state.lock().await;
        let button = Button {
            owner: link_device,
            metadata: ButtonMetadata { control_id: 0 },
            button: ButtonData {
                last_event: None,
                button_report: Some(ButtonReport {
                    updated: Utc::now(),
                    event: String::from("initial_press"),
                }),
                repeat_interval: Some(100),
                event_values: Some(json!(["initial_press", "repeat"])),
            },
        };

        let zbc = ZigbeeConnectivity {
            owner: link_device,
            mac_address: String::from("11:22:33:44:55:66:77:89"),
            status: ZigbeeConnectivityStatus::ConnectivityIssue,
            channel: Some(json!({
                "status": "set",
                "value": "channel_25",
            })),
            extended_pan_id: None,
        };

        res.add(&link_device, Resource::Device(dev))?;
        res.add(&link_button, Resource::Button(button))?;
        res.add(&link_zbc, Resource::ZigbeeConnectivity(zbc))?;
        drop(res);

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub async fn add_group(&mut self, grp: &z2m::api::Group) -> ApiResult<()> {
        let room_name;

        if let Some(ref prefix) = self.server.group_prefix {
            if let Some(name) = grp.friendly_name.strip_prefix(prefix) {
                room_name = name;
            } else {
                log::debug!(
                    "[{}] Ignoring room outside our prefix: {}",
                    self.name,
                    grp.friendly_name
                );
                return Ok(());
            }
        } else {
            room_name = &grp.friendly_name;
        }

        let link_room = RType::Room.deterministic(&grp.friendly_name);
        let link_glight = RType::GroupedLight.deterministic((link_room.rid, grp.id));

        let children = grp
            .members
            .iter()
            .map(|f| RType::Device.deterministic(&f.ieee_address))
            .collect();

        let topic = grp.friendly_name.to_string();

        let mut res = self.state.lock().await;

        let mut scenes_new = HashSet::new();

        for scn in &grp.scenes {
            let scene = Scene {
                actions: vec![],
                auto_dynamic: false,
                group: link_room,
                metadata: SceneMetadata {
                    appdata: None,
                    image: guess_scene_icon(&scn.name),
                    name: scn.name.to_string(),
                },
                palette: json!({
                    "color": [],
                    "dimming": [],
                    "color_temperature": [],
                    "effects": [],
                }),
                speed: 0.5,
                recall: SceneRecall {
                    action: None,
                    dimming: None,
                    duration: None,
                },
                status: Some(SceneStatus {
                    active: SceneActive::Inactive,
                    last_recall: None,
                }),
            };

            let link_scene = RType::Scene.deterministic((link_room.rid, scn.id));

            res.aux_set(
                &link_scene,
                AuxData::new().with_topic(&topic).with_index(scn.id),
            );

            scenes_new.insert(link_scene.rid);
            res.add(&link_scene, Resource::Scene(scene))?;
        }

        if let Ok(room) = res.get::<Room>(&link_room) {
            log::info!(
                "[{}] {link_room:?} ({}) known, updating..",
                self.name,
                room.metadata.name
            );

            let scenes_old: HashSet<Uuid> =
                HashSet::from_iter(res.get_scenes_for_room(&link_room.rid));

            log::trace!("[{}] old scenes: {scenes_old:?}", self.name);
            log::trace!("[{}] new scenes: {scenes_new:?}", self.name);
            let gone = scenes_old.difference(&scenes_new);
            log::trace!("[{}]   deleted: {gone:?}", self.name);
            for uuid in gone {
                log::debug!(
                    "[{}] Deleting orphaned {uuid:?} in {link_room:?}",
                    self.name
                );
                let _ = res.delete(&RType::Scene.link_to(*uuid));
            }
        } else {
            log::debug!(
                "[{}] {link_room:?} ({}) is new, adding..",
                self.name,
                room_name
            );
        }

        let mut metadata = RoomMetadata::new(RoomArchetype::Home, room_name);
        if let Some(room_conf) = self.config.rooms.get(&topic) {
            if let Some(name) = &room_conf.name {
                metadata.name = name.to_string();
            }
            if let Some(icon) = &room_conf.icon {
                metadata.archetype = *icon;
            }
        }

        let room = Room {
            children,
            metadata,
            services: btreeset![link_glight],
        };

        self.map.insert(topic.clone(), link_glight);
        self.rmap.insert(link_glight, topic.clone());
        self.rmap.insert(link_room, topic.clone());

        for id in &res.get_resource_ids_by_type(RType::BridgeHome) {
            res.update(id, |bh: &mut BridgeHome| {
                bh.children.insert(link_room);
            })?;
        }

        res.add(&link_room, Resource::Room(room))?;

        let glight = GroupedLight::new(link_room);

        res.add(&link_glight, Resource::GroupedLight(glight))?;
        drop(res);

        Ok(())
    }

    #[allow(clippy::match_same_arms)]
    fn make_hue_specific_update(upd: &LightUpdate) -> ApiResult<HueZigbeeUpdate> {
        let mut hz = HueZigbeeUpdate::new();

        if let Some(grad) = &upd.gradient {
            hz = hz.with_gradient_colors(
                match grad.mode {
                    Some(LightGradientMode::InterpolatedPalette) => GradientStyle::Linear,
                    Some(LightGradientMode::InterpolatedPaletteMirrored) => GradientStyle::Mirrored,
                    Some(LightGradientMode::RandomPixelated) => GradientStyle::Scattered,
                    None => GradientStyle::Linear,
                },
                grad.points.iter().map(|c| c.color.xy).collect(),
            )?;

            hz = hz.with_gradient_params(GradientParams {
                scale: match grad.mode {
                    Some(LightGradientMode::InterpolatedPalette) => 0x28,
                    Some(LightGradientMode::InterpolatedPaletteMirrored) => 0x18,
                    Some(LightGradientMode::RandomPixelated) => 0x38,
                    None => 0x18,
                },
                offset: 0x00,
            });
        }

        if let Some(LightEffectsV2Update {
            action: Some(act), ..
        }) = &upd.effects_v2
        {
            if let Some(fx) = &act.effect {
                let et = match fx {
                    LightEffect::NoEffect => EffectType::NoEffect,
                    LightEffect::Prism => EffectType::Prism,
                    LightEffect::Opal => EffectType::Opal,
                    LightEffect::Glisten => EffectType::Glisten,
                    LightEffect::Sparkle => EffectType::Sparkle,
                    LightEffect::Fire => EffectType::Fireplace,
                    LightEffect::Candle => EffectType::Candle,
                    LightEffect::Underwater => EffectType::Underwater,
                    LightEffect::Cosmos => EffectType::Cosmos,
                    LightEffect::Sunbeam => EffectType::Sunbeam,
                    LightEffect::Enchant => EffectType::Enchant,
                };
                hz = hz.with_effect_type(et);
            }
            if let Some(speed) = &act.parameters.speed {
                hz = hz.with_effect_speed(speed.unit_to_u8_clamped());
            }
            if let Some(mirek) = &act.parameters.color_temperature.and_then(|ct| ct.mirek) {
                hz = hz.with_color_mirek(*mirek);
            }
            if let Some(color) = &act.parameters.color {
                hz = hz.with_color_xy(color.xy);
            }
        }

        Ok(hz)
    }

    pub async fn event_loop(
        &mut self,
        chan: &mut Receiver<Arc<BackendRequest>>,
        mut socket: Z2mWebSocket,
    ) -> ApiResult<()> {
        loop {
            select! {
                // all backend event handling implemented in backend::z2m::backend_event
                pkt = chan.recv() => {
                    let api_req = pkt?;
                    self.handle_backend_event(&mut socket, api_req).await?;
                    // FIXME: this used to be our "throttle" feature, but it breaks entertainment mode
                    /* tokio::time::sleep(std::time::Duration::from_millis(100)).await; */
                },

                // all bridge event handling implemented in backend::z2m::bridge_event
                pkt = socket.next() => {
                    self.websocket_read(pkt.ok_or(ApiError::UnexpectedZ2mEof)??).await?;
                },

                Some((topic, upd)) = self.message_rx.recv() => {
                    socket.send_update(&topic, &upd).await?;
                }
            };
        }
    }
}

#[async_trait]
impl Backend for Z2mBackend {
    async fn run_forever(mut self, mut chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()> {
        // let's not include auth tokens in log output
        let sanitized_url = self.server.get_sanitized_url();
        let url = self.server.get_url();

        if url != self.server.url {
            log::info!(
                "[{}] Rewrote url for compatibility with z2m 2.x.",
                self.name
            );
            log::info!(
                "[{}] Consider updating websocket url to {}",
                self.name,
                sanitized_url
            );
        }

        // if tls verification is disabled, build a TlsConnector that explicitly
        // does not check certificate validity. This is obviously neither safe
        // nor recommended.
        let connector = if self.server.disable_tls_verify.unwrap_or_default() {
            log::warn!(
                "[{}] TLS verification disabled; will accept any certificate!",
                self.name
            );
            Some(Connector::NativeTls(
                TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .build()?,
            ))
        } else {
            None
        };

        loop {
            log::info!("[{}] Connecting to {}", self.name, &sanitized_url);
            match connect_async_tls_with_config(url.as_str(), None, false, connector.clone()).await
            {
                Ok((socket, _)) => {
                    let z2m_socket = Z2mWebSocket::new(self.name.clone(), socket);
                    let res = self.event_loop(&mut chan, z2m_socket).await;
                    if let Err(err) = res {
                        log::error!("[{}] Event loop broke: {err}", self.name);
                    }
                }
                Err(err) => {
                    log::error!("[{}] Connect failed: {err:?}", self.name);
                }
            }
            sleep(Duration::from_millis(2000)).await;
        }
    }
}

#[allow(clippy::match_same_arms)]
fn guess_scene_icon(name: &str) -> Option<ResourceLink> {
    let icon = match name {
        /* Built-in names */
        "Bright" => scene_icons::BRIGHT,
        "Relax" => scene_icons::RELAX,
        "Night Light" => scene_icons::NIGHT_LIGHT,
        "Rest" => scene_icons::REST,
        "Concentrate" => scene_icons::CONCENTRATE,
        "Dimmed" => scene_icons::DIMMED,
        "Energize" => scene_icons::ENERGIZE,
        "Read" => scene_icons::READ,
        "Cool Bright" => scene_icons::COOL_BRIGHT,

        /* Aliases */
        "Night" => scene_icons::NIGHT_LIGHT,
        "Cool" => scene_icons::COOL_BRIGHT,
        "Dim" => scene_icons::DIMMED,

        _ => return None,
    };

    Some(ResourceLink {
        rid: icon,
        rtype: RType::PublicImage,
    })
}
