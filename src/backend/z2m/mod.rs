pub mod learn;
pub mod stream;
pub mod zclcommand;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use maplit::btreeset;
use native_tls::TlsConnector;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::Mutex;
use tokio::sync::broadcast::Receiver;
use tokio::time::sleep;
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config, tungstenite,
};
use uuid::Uuid;

use hue::api::{
    BridgeHome, Button, ButtonData, ButtonMetadata, ButtonReport, DeviceArchetype,
    DeviceProductData, DimmingUpdate, Entertainment, EntertainmentConfiguration,
    EntertainmentSegment, EntertainmentSegments, GroupedLight, Light, LightEffect, LightEffects,
    LightEffectsV2, LightEffectsV2Update, LightGradientMode, LightGradientPoint,
    LightGradientUpdate, LightMetadata, LightUpdate, Metadata, RType, Resource, ResourceLink, Room,
    RoomArchetype, RoomMetadata, Scene, SceneActive, SceneMetadata, SceneRecall, SceneStatus,
    SceneStatusEnum, Stub, Taurus, ZigbeeConnectivity, ZigbeeConnectivityStatus,
};
use hue::clamp::Clamp;
use hue::error::HueError;
use hue::scene_icons;
use hue::stream::HueStreamLights;
use hue::zigbee::{
    EffectType, EntertainmentZigbeeStream, GradientParams, GradientStyle, HueEntFrameLightRecord,
    HueZigbeeUpdate, LightRecordMode, PHILIPS_HUE_ZIGBEE_VENDOR_ID, ZigbeeTarget,
};
use z2m::api::{ExposeLight, Message, RawMessage};
use z2m::convert::{
    ExtractColorTemperature, ExtractDeviceProductData, ExtractDimming, ExtractLightColor,
    ExtractLightGradient,
};
use z2m::request::Z2mRequest;
use z2m::update::{DeviceColorMode, DeviceUpdate};

use crate::backend::z2m::learn::SceneLearn;
use crate::backend::z2m::stream::Z2mTarget;
use crate::backend::{Backend, BackendRequest};
use crate::config::{AppConfig, Z2mServer};
use crate::error::{ApiError, ApiResult};
use crate::model::state::AuxData;
use crate::resource::Resources;

struct EntStream {
    stream: EntertainmentZigbeeStream,
    target: Z2mTarget,
    addrs: BTreeMap<String, Vec<u16>>,
    modes: Vec<(u16, LightRecordMode)>,
}

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
}

fn z2m_set_entertainment_brightness(brightness: u8) -> Z2mRequest<'static> {
    Z2mRequest::RawWrite(json!({
        "cluster": EntertainmentZigbeeStream::CLUSTER,
        "payload": {
            "5": {
                "manufacturerCode": PHILIPS_HUE_ZIGBEE_VENDOR_ID,
                "type": 32,
                "value": brightness,
            }
        }
    }))
}

impl Z2mBackend {
    pub fn new(
        name: String,
        server: Z2mServer,
        config: Arc<AppConfig>,
        state: Arc<Mutex<Resources>>,
    ) -> ApiResult<Self> {
        let map = HashMap::new();
        let rmap = HashMap::new();
        let learn = SceneLearn::new(name.clone());
        let ignore = HashSet::new();
        let network = HashMap::new();
        let entstream = None;
        Ok(Self {
            name,
            server,
            config,
            state,
            map,
            rmap,
            learner: learn,
            ignore,
            network,
            entstream,
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

    pub async fn handle_update(&mut self, rid: &Uuid, payload: &Value) -> ApiResult<()> {
        let upd = DeviceUpdate::deserialize(payload)?;

        let obj = self.state.lock().await.get_resource_by_id(rid)?.obj;
        match obj {
            Resource::Light(_) => {
                if let Err(e) = self.handle_update_light(rid, &upd).await {
                    log::error!("FAIL: {e:?} in {upd:?}");
                }
            }
            Resource::GroupedLight(_) => {
                if let Err(e) = self.handle_update_grouped_light(rid, &upd).await {
                    log::error!("FAIL: {e:?} in {upd:?}");
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_update_light(&mut self, uuid: &Uuid, devupd: &DeviceUpdate) -> ApiResult<()> {
        let mut res = self.state.lock().await;
        res.update::<Light>(uuid, |light| {
            let mut upd = LightUpdate::new()
                .with_on(devupd.state.map(Into::into))
                .with_brightness(devupd.brightness.map(|b| b / 254.0 * 100.0))
                .with_color_temperature(devupd.color_temp)
                .with_gradient(devupd.gradient.as_ref().map(|s| {
                    LightGradientUpdate {
                        mode: None,
                        points: s
                            .iter()
                            .map(|hc| LightGradientPoint::xy(hc.to_xy_color()))
                            .collect(),
                    }
                }));

            if devupd.color_mode != Some(DeviceColorMode::ColorTemp) {
                upd = upd.with_color_xy(devupd.color.and_then(|col| col.xy));
            }

            *light += upd;
        })?;

        self.learner.learn(uuid, &res, devupd)?;
        self.learner.collect(&mut res)?;
        drop(res);

        Ok(())
    }

    async fn handle_update_grouped_light(&self, uuid: &Uuid, upd: &DeviceUpdate) -> ApiResult<()> {
        let mut res = self.state.lock().await;
        res.update::<GroupedLight>(uuid, |glight| {
            if let Some(state) = &upd.state {
                glight.on = Some((*state).into());
            }

            if let Some(b) = upd.brightness {
                glight.dimming = Some(DimmingUpdate {
                    brightness: b / 254.0 * 100.0,
                });
            }
        })
    }

    async fn handle_bridge_message(&mut self, msg: Message) -> ApiResult<()> {
        #[allow(unused_variables)]
        match &msg {
            Message::BridgeInfo(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeLogging(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeExtensions(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeEvent(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeDefinitions(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeState(obj) => { /* println!("{obj:#?}"); */ }
            Message::BridgeConverters(obj) => { /* println!("{obj:#?}"); */ }

            Message::BridgeDevices(obj) => {
                for dev in obj {
                    self.network.insert(dev.friendly_name.clone(), dev.clone());
                    if let Some(exp) = dev.expose_light() {
                        log::info!(
                            "[{}] Adding light {:?}: [{}] ({})",
                            self.name,
                            dev.ieee_address,
                            dev.friendly_name,
                            dev.model_id.as_deref().unwrap_or("<unknown model>")
                        );
                        self.add_light(dev, exp).await?;
                    } else {
                        log::debug!(
                            "[{}] Ignoring unsupported device {}",
                            self.name,
                            dev.friendly_name
                        );
                        self.ignore.insert(dev.friendly_name.to_string());
                    }
                    /*
                    if dev.expose_action() {
                        log::info!(
                            "[{}] Adding switch {:?}: [{}] ({})",
                            self.name,
                            dev.ieee_address,
                            dev.friendly_name,
                            dev.model_id.as_deref().unwrap_or("<unknown model>")
                        );
                        self.add_switch(dev).await?;
                    }
                    */
                }
            }

            Message::BridgeGroups(obj) => {
                /* println!("{obj:#?}"); */
                for grp in obj {
                    self.add_group(grp).await?;
                }
            }

            Message::BridgeGroupMembersAdd(change) | Message::BridgeGroupMembersRemove(change) => {
                if let Some(light) = self.map.get(&change.data.device) {
                    let mut lock = self.state.lock().await;
                    let device = lock.get::<Light>(light)?.clone();

                    let device_link = device.owner;
                    if let Some(room) = self.map.get(&change.data.group) {
                        let room_link = lock.get::<GroupedLight>(room)?.owner;
                        let exists = lock
                            .get::<Room>(&room_link)?
                            .children
                            .contains(&device_link);

                        match msg {
                            Message::BridgeGroupMembersAdd(_) => {
                                if !exists {
                                    lock.update(&room_link.rid, |room: &mut Room| {
                                        room.children.insert(device_link);
                                    })?;
                                }
                            }
                            Message::BridgeGroupMembersRemove(_) => {
                                if exists {
                                    lock.update(&room_link.rid, |room: &mut Room| {
                                        room.children.remove(&device_link);
                                    })?;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_device_message(&mut self, msg: RawMessage) -> ApiResult<()> {
        if msg.topic.ends_with("/availability") || msg.topic.ends_with("/action") {
            // availability: https://www.zigbee2mqtt.io/guide/usage/mqtt_topics_and_messages.html#zigbee2mqtt-friendly-name-availability
            // action: https://www.home-assistant.io/integrations/device_trigger.mqtt/
            return Ok(());
        }

        let Some(ref val) = self.map.get(&msg.topic).copied() else {
            if !self.ignore.contains(&msg.topic) {
                log::warn!(
                    "[{}] Notification on unknown topic {}",
                    self.name,
                    &msg.topic
                );
            }
            return Ok(());
        };

        let res = self.handle_update(&val.rid, &msg.payload).await;
        if let Err(ref err) = res {
            log::error!(
                "Cannot parse update: {err}\n{}",
                serde_json::to_string_pretty(&msg.payload)?
            );
        }

        /* return Ok here, since we do not want to break the event loop */
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

    async fn websocket_read(&mut self, pkt: tungstenite::Message) -> ApiResult<()> {
        let tungstenite::Message::Text(txt) = pkt else {
            log::error!("[{}] Received non-text message on websocket :(", self.name);
            return Err(ApiError::UnexpectedZ2mReply(pkt));
        };

        let raw_msg: Result<RawMessage, _> = serde_json::from_str(&txt);

        let msg = raw_msg.map_err(|err| {
            log::error!(
                "[{}] Invalid websocket message: {:#?} [{}..]",
                self.name,
                err,
                &txt.chars().take(128).collect::<String>()
            );
            err
        })?;

        /* bridge messages are handled differently. everything else is a device message */
        if !msg.topic.starts_with("bridge/") {
            return self.handle_device_message(msg).await;
        }

        match serde_json::from_str(&txt) {
            Ok(bridge_msg) => self.handle_bridge_message(bridge_msg).await,
            Err(err) => {
                match msg.topic.as_str() {
                    topic @ ("bridge/devices" | "bridge/groups") => {
                        log::error!(
                            "[{}] Failed to parse critical z2m bridge message on [{}]:",
                            self.name,
                            topic,
                        );
                        log::error!("[{}] {}", self.name, serde_json::to_string(&msg.payload)?);
                        Err(err)?
                    }
                    topic => {
                        log::error!(
                            "[{}] Failed to parse (non-critical) z2m bridge message on [{}]:",
                            self.name,
                            topic
                        );
                        log::error!("{}", serde_json::to_string(&msg.payload)?);

                        /* Suppress this non-critical error, to avoid breaking the event loop */
                        Ok(())
                    }
                }
            }
        }
    }

    async fn websocket_send(
        &self,
        socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        topic: &str,
        payload: Z2mRequest<'_>,
    ) -> ApiResult<()> {
        let Some(link) = self.map.get(topic) else {
            log::trace!(
                "[{}] Topic [{topic}] unknown on this z2m connection",
                self.name
            );
            return Ok(());
        };

        log::trace!(
            "[{}] Topic [{topic}] known as {link:?} on this z2m connection, sending event..",
            self.name
        );

        let api_req = match &payload {
            Z2mRequest::Untyped { endpoint, value } => RawMessage {
                topic: format!("{topic}/{endpoint}/set"),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::RawWrite(value) => RawMessage {
                topic: format!("{topic}/set/write"),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::GroupMemberAdd(value) => RawMessage {
                topic: "bridge/request/group/members/add".into(),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::GroupMemberRemove(value) => RawMessage {
                topic: "bridge/request/group/members/remove".into(),
                payload: serde_json::to_value(value)?,
            },
            _ => RawMessage {
                topic: format!("{topic}/set"),
                payload: serde_json::to_value(&payload)?,
            },
        };

        let json = serde_json::to_string(&api_req)?;
        log::debug!("[{}] Sending {json}", self.name);
        let msg = tungstenite::Message::text(json);
        Ok(socket.send(msg).await?)
    }

    #[allow(clippy::too_many_lines)]
    async fn websocket_write(
        &mut self,
        socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        req: Arc<BackendRequest>,
    ) -> ApiResult<()> {
        self.learner.cleanup();

        let mut lock = self.state.lock().await;

        match (*req).clone() {
            BackendRequest::LightUpdate(link, upd) => {
                if let Some(topic) = self.rmap.get(&link) {
                    // We cannot recover .mode from backend updates, since these only contain
                    // the gradient colors. So we have no choice, but to update the mode
                    // here. Otherwise, the information would be lost.
                    if let Some(mode) = upd.gradient.as_ref().and_then(|gr| gr.mode) {
                        lock.update::<Light>(&link.rid, |light| {
                            if let Some(gr) = &mut light.gradient {
                                gr.mode = mode;
                            }
                        })?;
                    }
                    let hue_effects = lock.get::<Light>(&link)?.effects.is_some();
                    drop(lock);

                    /* step 1: send generic light update */
                    let mut payload = DeviceUpdate::default()
                        .with_state(upd.on.map(|on| on.on))
                        .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 254.0))
                        .with_color_temp(upd.color_temperature.and_then(|ct| ct.mirek))
                        .with_color_xy(upd.color.map(|col| col.xy));

                    // We don't want to send gradient updates twice, but if hue
                    // effects are not supported for this light, this is the best
                    // (and only) way to do it
                    if !hue_effects {
                        payload = payload.with_gradient(upd.gradient.clone());
                    }

                    let z2mreq = Z2mRequest::Update(&payload);

                    self.websocket_send(socket, topic, z2mreq).await?;

                    /* step 2: if supported (and needed) send hue-specific effects update */

                    if hue_effects {
                        let mut hz = Self::make_hue_specific_update(&upd)?;

                        if !hz.is_empty() {
                            hz = hz.with_fade_speed(0x0001);

                            let data = hz.to_vec()?;
                            log::debug!("Sending hue-specific frame: {}", hex::encode(&data));

                            let upd = json!({
                                "command": {
                                    "cluster": 0xFC03,
                                    "command": 0,
                                    "payload": {
                                        "data": data
                                    }
                                }}
                            );
                            let z2mreq = Z2mRequest::Untyped {
                                endpoint: 11,
                                value: &upd,
                            };

                            self.websocket_send(socket, topic, z2mreq).await?;
                        }
                    }
                }
            }

            BackendRequest::SceneCreate(link_scene, sid, scene) => {
                if let Some(topic) = self.rmap.get(&scene.group) {
                    log::info!("New scene: {link_scene:?} ({})", scene.metadata.name);

                    lock.aux_set(
                        &link_scene,
                        AuxData::new()
                            .with_topic(&scene.metadata.name)
                            .with_index(sid),
                    );
                    let z2mreq = Z2mRequest::SceneStore {
                        name: &scene.metadata.name.clone(),
                        id: sid,
                    };

                    lock.add(&link_scene, Resource::Scene(scene))?;
                    drop(lock);

                    self.websocket_send(socket, topic, z2mreq).await?;
                }
            }

            BackendRequest::SceneUpdate(link, upd) => {
                if let Some(recall) = upd.recall {
                    let scene = lock.get::<Scene>(&link)?;
                    if recall.action == Some(SceneStatusEnum::Active) {
                        let index = lock
                            .aux_get(&link)?
                            .index
                            .ok_or(HueError::NotFound(link.rid))?;

                        let scenes = lock.get_scenes_for_room(&scene.group.rid);
                        for rid in scenes {
                            lock.update::<Scene>(&rid, |scn| {
                                scn.status = Some(SceneStatus {
                                    active: if rid == link.rid {
                                        SceneActive::Static
                                    } else {
                                        SceneActive::Inactive
                                    },
                                    last_recall: None,
                                });
                            })?;
                        }

                        let room = lock.get::<Scene>(&link)?.group;
                        drop(lock);

                        if let Some(topic) = self.rmap.get(&room).cloned() {
                            log::info!("[{}] Recall scene: {link:?}", self.name);

                            let mut lock = self.state.lock().await;
                            self.learner.learn_scene_recall(&link, &mut lock)?;

                            let z2mreq = Z2mRequest::SceneRecall(index);
                            self.websocket_send(socket, &topic, z2mreq).await?;
                        }
                    } else {
                        log::error!("Scene recall type not supported: {recall:?}");
                    }
                }
            }

            BackendRequest::GroupedLightUpdate(link, upd) => {
                let room = lock.get::<GroupedLight>(&link)?.owner;
                drop(lock);

                let payload = DeviceUpdate::default()
                    .with_state(upd.on.map(|on| on.on))
                    .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 254.0))
                    .with_color_temp(upd.color_temperature.and_then(|ct| ct.mirek))
                    .with_color_xy(upd.color.map(|col| col.xy));

                if let Some(topic) = self.rmap.get(&room) {
                    let z2mreq = Z2mRequest::Update(&payload);
                    self.websocket_send(socket, topic, z2mreq).await?;
                }
            }

            BackendRequest::Delete(link) => {
                if link.rtype != RType::Scene {
                    return Ok(());
                }

                let room = lock.get::<Scene>(&link)?.group;
                let index = lock
                    .aux_get(&link)?
                    .index
                    .ok_or(HueError::NotFound(link.rid))?;
                drop(lock);

                if let Some(topic) = self.rmap.get(&room) {
                    let z2mreq = Z2mRequest::SceneRemove(index);
                    self.websocket_send(socket, topic, z2mreq).await?;
                }
            }

            BackendRequest::EntertainmentStart(ent_id) => {
                let ent: &EntertainmentConfiguration = lock.get_id(ent_id)?;

                let mut chans = ent.channels.clone();

                let mut addrs: BTreeMap<String, Vec<u16>> = BTreeMap::new();
                let mut targets = vec![];
                chans.sort_by_key(|c| c.channel_id);

                for chan in chans {
                    for member in &chan.members {
                        let ent: &Entertainment = lock.get(&member.service)?;
                        let light_id = ent
                            .renderer_reference
                            .ok_or(HueError::NotFound(member.service.rid))?;
                        let topic = self
                            .rmap
                            .get(&light_id)
                            .ok_or(HueError::NotFound(member.service.rid))?;
                        let dev = self
                            .network
                            .get(topic)
                            .ok_or(HueError::NotFound(member.service.rid))?;

                        let segment_addr = dev.network_address + member.index;

                        addrs
                            .entry(dev.friendly_name.clone())
                            .or_default()
                            .push(segment_addr);

                        targets.push(topic);
                    }
                }
                log::debug!("Entertainment addresses: {addrs:04x?}");
                drop(lock);

                if let Some(target) = targets.first() {
                    let mut modes = vec![];

                    for segments in addrs.values() {
                        let mode = if segments.len() <= 1 {
                            LightRecordMode::Device
                        } else {
                            LightRecordMode::Segment
                        };

                        for seg in segments {
                            modes.push((*seg, mode));
                        }
                    }

                    let mut es = EntStream {
                        stream: EntertainmentZigbeeStream::new(self.counter),
                        target: Z2mTarget::new(target),
                        addrs,
                        modes,
                    };

                    log::debug!("Entertainment addrs: {:#?}", &es.addrs);
                    log::debug!("Entertainment modes: {:#?}", &es.modes);
                    for (dev, segments) in &es.addrs {
                        let z2mreq = z2m_set_entertainment_brightness(0xFE);
                        self.websocket_send(socket, dev, z2mreq).await?;

                        if segments.len() <= 1 {
                            continue;
                        }

                        let z2mreq = es.target.send(es.stream.segment_mapping(segments)?)?;
                        self.websocket_send(socket, dev, z2mreq).await?;
                    }

                    let z2mreq = es.target.send(es.stream.reset()?)?;
                    for topic in es.addrs.keys() {
                        log::debug!("Sending stop to {topic}");
                        self.websocket_send(socket, topic, z2mreq.clone()).await?;
                    }

                    self.entstream = Some(es);
                }
            }

            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            BackendRequest::EntertainmentFrame(frame) => {
                if let Some(es) = &mut self.entstream {
                    let mut blks = vec![];

                    if let HueStreamLights::Rgb(rgb) = frame {
                        for light in rgb {
                            let (xy, bright) = light.to_xy();

                            let brightness = (bright / 255.0 * 2047.0).clamp(1.0, 2047.0) as u16;
                            let (chan, mode) = es.modes[light.channel as usize % es.modes.len()];
                            let raw = xy.to_quant();
                            let lrec = HueEntFrameLightRecord::new(chan, brightness, mode, raw);

                            blks.push(lrec);
                        }
                    } else {
                        // FIXME
                        unimplemented!();
                    }

                    let z2mreq = es.target.send(es.stream.frame(blks)?)?;
                    let device = es.target.device.clone();
                    self.websocket_send(socket, &device, z2mreq).await?;
                }
            }

            BackendRequest::EntertainmentStop() => {
                log::debug!("Stopping entertainment mode..");
                if let Some(es) = &mut self.entstream.take() {
                    let z2mreq = es.target.send(es.stream.reset()?)?;
                    for topic in es.addrs.keys() {
                        log::debug!("Sending stop to {topic}");
                        self.websocket_send(socket, topic, z2mreq.clone()).await?;
                    }
                    self.counter = es.stream.counter();
                }
            }
        }

        Ok(())
    }

    pub async fn event_loop(
        &mut self,
        chan: &mut Receiver<Arc<BackendRequest>>,
        mut socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> ApiResult<()> {
        loop {
            select! {
                pkt = chan.recv() => {
                    let api_req = pkt?;
                    self.websocket_write(&mut socket, api_req).await?;
                    // FIXME: this used to be our "throttle" feature, but it breaks entertainment mode
                    /* tokio::time::sleep(std::time::Duration::from_millis(100)).await; */
                },
                pkt = socket.next() => {
                    self.websocket_read(pkt.ok_or(ApiError::UnexpectedZ2mEof)??).await?;
                },
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
                    let res = self.event_loop(&mut chan, socket).await;
                    if let Err(err) = res {
                        log::error!("[{}] Event loop broke: {err}", self.name);
                    }
                }
                Err(err) => {
                    log::error!("[{}] Connect failed: {err:?}", self.name);
                }
            }
            sleep(std::time::Duration::from_millis(2000)).await;
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
