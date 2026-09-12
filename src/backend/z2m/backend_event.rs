use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use hue::clamp::Clamp;
use hue::effect_duration::EffectDuration;
use hue::zigbee::{GradientParams, GradientStyle, HueZigbeeUpdate, LightRecordMode};
use tokio::time::sleep;
use uuid::Uuid;

use bifrost_api::backend::{BackendRequest, DeleteReply, RequestReply};
use hue::api::{
    BridgeHome, ColorTemperatureUpdate, DimmingDeltaAction, Entertainment,
    EntertainmentConfiguration, Group, GroupedLight, GroupedLightUpdate, Light,
    LightEffectsV2Update, LightUpdate, RType, Resource, ResourceLink, Room, RoomNew, RoomUpdate,
    Scene, SceneActive, SceneStatus, SceneStatusEnum, SceneUpdate, ZigbeeDeviceDiscoveryUpdate,
    Zone, ZoneNew, ZoneUpdate,
};
use hue::error::HueError;
use hue::stream::HueStreamLightsV2;
use z2m::api::DeviceRead;
use z2m::update::{DeviceEffect, DeviceUpdate};

use crate::backend::z2m::entertainment::EntStream;
use crate::backend::z2m::websocket::Z2mWebSocket;
use crate::backend::z2m::{Z2mBackend, Z2mMessage};
use crate::error::ApiResult;
use crate::model::state::AuxData;

impl Z2mBackend {
    #[allow(clippy::match_same_arms)]
    fn make_hue_specific_update(upd: &LightUpdate) -> ApiResult<HueZigbeeUpdate> {
        let mut hz = HueZigbeeUpdate::new();

        if let Some(on) = &upd.on {
            hz = hz.with_on_off(on.on);
        }

        if let Some(br) = &upd.dimming {
            hz = hz.with_brightness((br.brightness / 100.0).unit_to_u8_clamped_light());
        }

        if let Some(ColorTemperatureUpdate { mirek: Some(mirek) }) = upd.color_temperature {
            hz = hz.with_color_mirek(mirek);
        }

        if let Some(xy) = &upd.color {
            hz = hz.with_color_xy(xy.xy);
        }

        if let Some(grad) = &upd.gradient {
            hz = hz.with_gradient_colors(
                grad.mode.map_or(GradientStyle::Linear, Into::into),
                grad.points.iter().map(|c| c.color.xy).collect(),
            )?;

            hz = hz.with_gradient_params(GradientParams {
                scale: u8::try_from(grad.points.len())? << 3,
                offset: 0x00,
            });
        }

        if let Some(LightEffectsV2Update {
            action: Some(act), ..
        }) = &upd.effects_v2
        {
            if let Some(fx) = act.effect {
                hz = hz.with_effect_type(fx.into());
            }
            if let Some(speed) = &act.parameters.speed {
                hz = hz.with_effect_speed(speed.unit_to_u8_clamped_light());
            }
            if let Some(mirek) = &act.parameters.color_temperature.and_then(|ct| ct.mirek) {
                hz = hz.with_color_mirek(*mirek);
            }
            if let Some(color) = &act.parameters.color {
                hz = hz.with_color_xy(color.xy);
            }
        }

        if let Some(act) = &upd.timed_effects {
            if let Some(fx) = act.effect {
                hz = hz.with_effect_type(fx.into());
            }

            if let Some(duration) = act.duration {
                hz = hz.with_effect_duration(EffectDuration::from_ms(duration)?);
            }
        }

        Ok(hz)
    }

    async fn backend_light_update(
        &self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        upd: &LightUpdate,
    ) -> ApiResult<()> {
        let Some(topic) = self.rmap.get(link) else {
            return Ok(());
        };

        let hue_effects = self
            .state
            .lock()
            .await
            .get::<Light>(link)?
            .effects
            .is_some();

        let mut payload: Option<DeviceUpdate> = None;

        // handle "identify" request (light breathing)
        if upd.identify.is_some() {
            payload = Some(
                payload
                    .unwrap_or_default()
                    .with_effect(DeviceEffect::Breathe),
            );

            let tx = self.message_tx.clone();
            let topic = topic.clone();

            // spawn task to stop effect after a few seconds
            let _job = tokio::spawn(async move {
                sleep(Self::LIGHT_BREATHE_DURATION).await;

                let upd = DeviceUpdate::new().with_effect(DeviceEffect::FinishEffect);
                tx.send((topic, Z2mMessage::DeviceUpdate(upd)))
            });
        }

        if !hue_effects {
            /* send generic light update */
            let transition = upd
                .dynamics
                .as_ref()
                .and_then(|d| d.duration.map(|duration| f64::from(duration) / 1000.0))
                .or_else(|| {
                    if upd.dimming.is_some()
                        || upd.color_temperature.is_some()
                        || upd.color.is_some()
                    {
                        Some(0.4)
                    } else {
                        None
                    }
                });
            payload = Some(
                payload
                    .unwrap_or_default()
                    .with_state(upd.on.map(|on| on.on))
                    .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 254.0))
                    .with_brightness_step(upd.dimming_delta.map(|dim_delta| {
                        let brightness = dim_delta.brightness_delta / 100.0 * 254.0;
                        match dim_delta.action {
                            DimmingDeltaAction::Up => brightness,
                            DimmingDeltaAction::Down => -brightness,
                        }
                    }))
                    .with_color_temp(upd.color_temperature.and_then(|ct| ct.mirek))
                    .with_color_xy(upd.color.map(|col| col.xy))
                    .with_transition(transition)
                    // We don't want to send gradient updates twice, but if hue
                    // effects are not supported for this light, this is the best
                    // (and only) way to do it
                    .with_gradient(upd.gradient.clone()),
            );
        }

        if let Some(payload) = payload {
            z2mws.send_update(topic, &payload).await?;
        }

        /* if supported send hue-specific effects update */
        if hue_effects {
            let mut hz = Self::make_hue_specific_update(upd)?;

            if !hz.is_empty() {
                hz = hz.with_fade_speed(0x0001);

                let read_payload = DeviceRead::default().with_state(true);
                z2mws.send_hue_effects(topic, hz).await?;

                // Do an explicit attribute read since Hue specific updates do not automatically update z2m state
                let tx = self.message_tx.clone();
                let topic = topic.clone();
                let transition_duration = Duration::from_millis(
                    upd.dynamics
                        .clone()
                        .and_then(|d| d.duration)
                        .unwrap_or(500)
                        .into(),
                );
                let _job = tokio::spawn(async move {
                    // wait until transition is done
                    sleep(transition_duration).await;
                    tx.send((topic, Z2mMessage::DeviceRead(read_payload)))
                });
            }
        }

        Ok(())
    }

    async fn backend_scene_create(
        &self,
        z2mws: &mut Z2mWebSocket,
        link_scene: &ResourceLink,
        sid: u32,
        scene: &Scene,
    ) -> ApiResult<()> {
        let Some(topic) = self.rmap.get(&scene.group) else {
            return Ok(());
        };

        log::info!("New scene: {link_scene:?} ({})", scene.metadata.name);

        let mut lock = self.state.lock().await;

        let auxdata = AuxData::new()
            .with_topic(&scene.metadata.name)
            .with_index(sid);

        lock.aux_set(link_scene, auxdata);

        z2mws
            .send_scene_store(topic, &scene.metadata.name, sid)
            .await?;

        lock.add(link_scene, Resource::Scene(scene.clone()))?;
        drop(lock);

        Ok(())
    }

    async fn backend_scene_update(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        upd: &SceneUpdate,
    ) -> ApiResult<()> {
        let mut lock = self.state.lock().await;

        let scene = lock.get::<Scene>(link)?;

        let index = lock
            .aux_get(link)?
            .index
            .ok_or(HueError::NotFound(link.rid))?;

        if let Some(recall) = &upd.recall {
            if recall.action == Some(SceneStatusEnum::Active) {
                let scenes = lock.get_scenes_for_group(&scene.group.rid);
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

                let group = lock.get::<Scene>(link)?.group;
                drop(lock);

                if let Some(topic) = self.rmap.get(&group).cloned() {
                    log::info!("[{}] Recall scene: {link:?}", self.name);

                    let mut lock = self.state.lock().await;
                    self.learner.learn_scene_recall(link, &mut lock)?;

                    z2mws.send_scene_recall(&topic, index).await?;
                }
            } else {
                log::error!("Scene recall type not supported: {recall:?}");
            }
        } else {
            // We're not recalling the scene, so we are updating the scene
            let group = lock.get::<Scene>(link)?.group;

            if let Some(topic) = self.rmap.get(&group).cloned() {
                log::info!("[{}] Store scene: {link:?}", self.name);

                let scene = lock.get::<Scene>(link)?;
                z2mws
                    .send_scene_store(&topic, &scene.metadata.name, index)
                    .await?;

                // We have requested z2m to update the scene, so update
                // the state database accordingly
                lock.update::<Scene>(&link.rid, |scene| {
                    *scene += upd;
                })?;

                drop(lock);
            }
        }

        Ok(())
    }

    async fn backend_grouped_light_update(
        &self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        upd: &GroupedLightUpdate,
    ) -> ApiResult<()> {
        let owner = self.state.lock().await.get::<GroupedLight>(link)?.owner;

        match owner.rtype {
            RType::Room | RType::Zone => {
                if let Some(topic) = self.rmap.get(&owner) {
                    z2mws.send_update(topic, &upd.into()).await?;
                }
            }
            RType::BridgeHome => {
                // Apply grouped light update to all rooms
                let lock = self.state.lock().await;
                let home = lock.get::<BridgeHome>(&owner)?.clone();
                let room_grouped_lights: Vec<_> = home
                    .children
                    .into_iter()
                    .filter_map(|child_link| {
                        if child_link.rtype == RType::Room {
                            let room = lock.get::<Room>(&child_link).ok()?;
                            room.grouped_light_service().copied()
                        } else {
                            None
                        }
                    })
                    .collect();
                drop(lock);
                for room_grouped_light in room_grouped_lights {
                    Box::pin(self.backend_grouped_light_update(z2mws, &room_grouped_light, upd))
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[allow(clippy::ref_option)]
    async fn backend_room_create(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        room_new: &RoomNew,
        existing_link: &Option<ResourceLink>,
        link_reply: &RequestReply<ResourceLink>,
    ) -> ApiResult<()> {
        let group_friendly_name = self.server.group_prefix.as_ref().map_or_else(
            || room_new.metadata.name.clone(),
            |group_prefix| format!("{group_prefix}{}", room_new.metadata.name),
        );

        // Store metadata
        let mut lock = self.state.lock().await;
        let room = Room {
            children: room_new.children.clone(),
            metadata: room_new.metadata.clone(),
            services: BTreeSet::new(),
        };
        let group_id = self.get_next_group_id();
        let link_glight = RType::GroupedLight.deterministic((&self.name, group_id));
        let link = existing_link.unwrap_or_else(|| RType::Room.deterministic(link_glight.rid));

        lock.add(&link, Resource::Room(room.clone()))?;
        let link_glight = RType::GroupedLight.deterministic((&self.name, group_id));
        lock.add(
            &link_glight,
            Resource::GroupedLight(GroupedLight::new(link)),
        )?;
        self.set_group_aux(&mut lock, link_glight, group_id, Some(&group_friendly_name));
        drop(lock);
        self.reserve_group_id(group_id);

        z2mws
            .send_group_add(group_id, group_friendly_name.clone())
            .await?;
        for member in &room.children {
            let Some(friendly_name) = &self.rmap.get(member) else {
                log::warn!("Unable to find friendly name for room member {member:?}. Skipping");
                continue;
            };
            z2mws
                .send_group_member_add(&group_friendly_name, friendly_name)
                .await?;
        }

        if let Some(reply) = link_reply
            && let Some(reply) = reply.lock().await.take()
        {
            let _ = reply.send(link);
        }

        Ok(())
    }

    async fn backend_room_update(
        &self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        upd: &RoomUpdate,
    ) -> ApiResult<()> {
        let lock = self.state.lock().await;

        if let Some(children) = &upd.children
            && let Some(topic) = self.rmap.get(link)
        {
            let room = lock.get::<Room>(link)?.clone();
            drop(lock);

            let known_existing: BTreeSet<_> = room
                .children
                .iter()
                .filter(|device| self.rmap.contains_key(device))
                .collect();

            let known_new: BTreeSet<_> = children
                .iter()
                .filter(|device| self.rmap.contains_key(device))
                .collect();

            for add in known_new.difference(&known_existing) {
                let Some(friendly_name) = &self.rmap.get(add) else {
                    log::warn!(
                        "Unable to find friendly name for room member when adding {add:?}. Skipping"
                    );
                    continue;
                };
                z2mws.send_group_member_add(topic, friendly_name).await?;
            }

            for remove in known_existing.difference(&known_new) {
                let Some(friendly_name) = &self.rmap.get(remove) else {
                    log::warn!(
                        "Unable to find friendly name for room member when removing {remove:?}. Skipping"
                    );
                    continue;
                };
                z2mws.send_group_member_remove(topic, friendly_name).await?;
            }
        }

        Ok(())
    }

    #[allow(clippy::ref_option)]
    async fn backend_zone_create(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        zone_new: &ZoneNew,
        existing_link: &Option<ResourceLink>,
        link_reply: &RequestReply<ResourceLink>,
    ) -> ApiResult<()> {
        let group_friendly_name = self.server.group_prefix.as_ref().map_or_else(
            || zone_new.metadata.name.clone(),
            |group_prefix| format!("{group_prefix}{}", zone_new.metadata.name),
        );

        // Store metadata
        let mut lock = self.state.lock().await;
        let zone = Zone {
            children: zone_new.children.clone(),
            metadata: zone_new.metadata.clone(),
            services: BTreeSet::new(),
        };
        let group_id = self.get_next_group_id();
        let link_glight = RType::GroupedLight.deterministic((&self.name, group_id));
        let link = existing_link.unwrap_or_else(|| RType::Zone.deterministic(link_glight.rid));

        lock.add(&link, Resource::Zone(zone.clone()))?;
        let link_glight = RType::GroupedLight.deterministic((&self.name, group_id));
        lock.add(
            &link_glight,
            Resource::GroupedLight(GroupedLight::new(link)),
        )?;
        self.set_group_aux(&mut lock, link_glight, group_id, Some(&group_friendly_name));
        drop(lock);
        self.reserve_group_id(group_id);

        z2mws
            .send_group_add(group_id, group_friendly_name.clone())
            .await?;
        for member in &zone.children {
            let Some(friendly_name) = &self.rmap.get(member) else {
                log::warn!("Unable to find friendly name for zone member {member:?}. Skipping");
                continue;
            };
            z2mws
                .send_group_member_add(&group_friendly_name, friendly_name)
                .await?;
        }

        if let Some(reply) = link_reply
            && let Some(reply) = reply.lock().await.take()
        {
            let _ = reply.send(link);
        }

        Ok(())
    }

    async fn backend_zone_update(
        &self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        upd: &ZoneUpdate,
    ) -> ApiResult<()> {
        let lock = self.state.lock().await;

        if let Some(children) = &upd.children
            && let Some(topic) = self.rmap.get(link)
        {
            let zone = lock.get::<Zone>(link)?.clone();
            drop(lock);

            let known_existing: BTreeSet<_> = zone
                .children
                .iter()
                .filter(|device| self.rmap.contains_key(device))
                .collect();

            let known_new: BTreeSet<_> = children
                .iter()
                .filter(|device| self.rmap.contains_key(device))
                .collect();

            for add in known_new.difference(&known_existing) {
                let Some(friendly_name) = &self.rmap.get(add) else {
                    log::warn!(
                        "Unable to find friendly name for zone member when adding {add:?}. Skipping"
                    );
                    continue;
                };
                z2mws.send_group_member_add(topic, friendly_name).await?;
            }

            for remove in known_existing.difference(&known_new) {
                let Some(friendly_name) = &self.rmap.get(remove) else {
                    log::warn!(
                        "Unable to find friendly name for zone member when removing {remove:?}. Skipping"
                    );
                    continue;
                };
                z2mws.send_group_member_remove(topic, friendly_name).await?;
            }
        }

        Ok(())
    }

    #[allow(clippy::ref_option)]
    async fn backend_delete(
        &self,
        z2mws: &mut Z2mWebSocket,
        link: &ResourceLink,
        reply: &RequestReply<DeleteReply>,
    ) -> ApiResult<()> {
        match link.rtype {
            RType::Scene => {
                let lock = self.state.lock().await;
                let group = lock.get::<Scene>(link)?.group;
                let index = lock
                    .aux_get(link)?
                    .index
                    .ok_or(HueError::NotFound(link.rid))?;
                drop(lock);

                if let Some(topic) = self.rmap.get(&group) {
                    if !reply_delete(reply, DeleteReply::Claimed).await {
                        return Ok(());
                    }
                    z2mws.send_scene_remove(topic, index).await?;
                }
            }

            RType::Device => {
                if let Some(dev) = self
                    .rmap
                    .get(link)
                    .and_then(|topic| self.network.get(topic))
                {
                    if !reply_delete(reply, DeleteReply::Claimed).await {
                        return Ok(());
                    }
                    let addr = dev.ieee_address.to_string();
                    log::info!(
                        "[{}] Requesting z2m removal of {} ({})",
                        self.name,
                        &addr,
                        dev.friendly_name
                    );

                    z2mws.send_device_remove(addr).await?;
                }
            }

            RType::Room | RType::Zone => {
                if let Some(topic) = self.rmap.get(link) {
                    if !reply_delete(reply, DeleteReply::Claimed).await {
                        return Ok(());
                    }
                    log::info!("[{}] Requesting z2m removal of {}", self.name, &topic);
                    z2mws.send_group_remove(topic.clone()).await?;
                }
            }

            rtype => {
                if !reply_delete(
                    reply,
                    DeleteReply::Failed(format!("Deleting type {rtype:?} is not supported")),
                )
                .await
                {
                    return Ok(());
                }
                log::warn!(
                    "[{}] Deleting objects of type {rtype:?} is not supported",
                    self.name
                );
            }
        }
        Ok(())
    }

    async fn backend_entertainment_start(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        ent_id: &Uuid,
    ) -> ApiResult<()> {
        log::trace!("[{}] Entertainment start", self.name);
        let lock = self.state.lock().await;

        let ent: &EntertainmentConfiguration = lock.get_id(*ent_id)?;

        let mut chans = ent.channels.clone();

        let mut addrs: BTreeMap<String, Vec<u16>> = BTreeMap::new();
        let mut channels: BTreeMap<u8, (u16, LightRecordMode)> = BTreeMap::new();
        let mut targets = vec![];
        chans.sort_by_key(|c| c.channel_id);

        log::trace!("[{}] Resolving entertainment channels", self.name);
        for chan in chans {
            for member in &chan.members {
                let ent: &Entertainment = lock.get(&member.service)?;
                let light_id = ent
                    .renderer_reference
                    .ok_or(HueError::NotFound(member.service.rid))?;
                let topic = self
                    .rmap
                    .get(&light_id)
                    .ok_or(HueError::NotFound(light_id.rid))?;
                let dev = self
                    .network
                    .get(topic)
                    .ok_or(HueError::NotFound(member.service.rid))?;

                let segment_addr = dev.network_address + member.index;

                addrs
                    .entry(dev.friendly_name.clone())
                    .or_default()
                    .push(segment_addr);

                let mode = if ent.segments.as_ref().map_or(0, |s| s.segments.len()) > 1 {
                    LightRecordMode::Segment
                } else {
                    LightRecordMode::Device
                };
                channels.insert(u8::try_from(chan.channel_id)?, (segment_addr, mode));

                targets.push(topic);
            }
        }
        log::debug!("Entertainment addresses: {addrs:04x?}");
        drop(lock);

        if let Some(target) = targets.first() {
            let mut es = EntStream::new(self.counter, target, addrs, channels);

            // hack: This diverges from the observed behavior of the Hue bridge.
            // The real fix is to persist the latest value of the entertainment counter,
            // but for now resetting the stream to the last known counter seems to work good enough.
            es.reset_stream(z2mws).await?;

            // Not even a real Philips Hue bridge uses this trick!
            //
            // We set the entertainment mode fade speed ("smoothing")
            // to fit the target frame rate, to ensure perfectly smooth
            // transitionss, even at low frame rates!
            es.stream.set_smoothing_duration(self.throttle.interval())?;

            es.start_stream(z2mws).await?;

            self.entstream = Some(es);
        }

        Ok(())
    }

    async fn backend_entertainment_frame(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        frame: &HueStreamLightsV2,
    ) -> ApiResult<()> {
        if let Some(es) = &mut self.entstream
            && self.throttle.tick()
        {
            es.frame(z2mws, frame).await?;
        }

        Ok(())
    }

    async fn backend_entertainment_stop(&mut self, z2mws: &mut Z2mWebSocket) -> ApiResult<()> {
        log::debug!("Stopping entertainment mode..");
        if let Some(es) = &mut self.entstream.take() {
            let mut lock = self.state.lock().await;

            es.reset_stream(z2mws).await?;

            self.counter = es.stream.counter();

            for id in lock.get_resource_ids_by_type(RType::Light) {
                let light: &Light = lock.get_id(id)?;
                if light.is_streaming() {
                    lock.update(&id, Light::stop_streaming)?;
                }
            }

            for id in lock.get_resource_ids_by_type(RType::EntertainmentConfiguration) {
                let ec: &EntertainmentConfiguration = lock.get_id(id)?;
                if ec.is_streaming() {
                    lock.update(&id, EntertainmentConfiguration::stop_streaming)?;
                }
            }
            drop(lock);
        }

        Ok(())
    }

    async fn backend_zigbee_device_discovery(
        &self,
        z2mws: &mut Z2mWebSocket,
        _rlink: &ResourceLink,
        _zbd: &ZigbeeDeviceDiscoveryUpdate,
    ) -> ApiResult<()> {
        z2mws.send_permit_join(60 * 4, None).await
    }

    pub async fn handle_backend_event(
        &mut self,
        z2mws: &mut Z2mWebSocket,
        req: Arc<BackendRequest>,
    ) -> ApiResult<()> {
        self.learner.cleanup();

        match &*req {
            BackendRequest::LightUpdate(link, upd) => {
                self.backend_light_update(z2mws, link, upd).await
            }

            BackendRequest::SceneCreate(link, sid, scene) => {
                self.backend_scene_create(z2mws, link, *sid, scene).await
            }

            BackendRequest::SceneUpdate(link, upd) => {
                self.backend_scene_update(z2mws, link, upd).await
            }

            BackendRequest::GroupedLightUpdate(link, upd) => {
                self.backend_grouped_light_update(z2mws, link, upd).await
            }

            BackendRequest::RoomCreate {
                backend,
                room_new,
                existing_link,
                link_reply,
            } => {
                if backend == &self.name {
                    self.backend_room_create(z2mws, room_new, existing_link, link_reply)
                        .await
                } else {
                    Ok(())
                }
            }

            BackendRequest::RoomUpdate(link, upd) => {
                self.backend_room_update(z2mws, link, upd).await
            }

            BackendRequest::ZoneCreate {
                backend,
                zone_new,
                existing_link,
                link_reply,
            } => {
                if backend == &self.name {
                    self.backend_zone_create(z2mws, zone_new, existing_link, link_reply)
                        .await
                } else {
                    Ok(())
                }
            }

            BackendRequest::ZoneUpdate(link, upd) => {
                self.backend_zone_update(z2mws, link, upd).await
            }

            BackendRequest::Delete { link, reply } => self.backend_delete(z2mws, link, reply).await,

            BackendRequest::EntertainmentStart(ent_id) => {
                self.backend_entertainment_start(z2mws, ent_id).await
            }

            BackendRequest::EntertainmentFrame(frame) => {
                self.backend_entertainment_frame(z2mws, frame).await
            }

            BackendRequest::EntertainmentStop() => self.backend_entertainment_stop(z2mws).await,

            BackendRequest::ZigbeeDeviceDiscovery(rlink, zbd) => {
                self.backend_zigbee_device_discovery(z2mws, rlink, zbd)
                    .await
            }
        }
    }
}

#[allow(clippy::ref_option)]
async fn reply_delete(claim: &RequestReply<DeleteReply>, claimed: DeleteReply) -> bool {
    if let Some(claim) = claim
        && let Some(claim) = claim.lock().await.take()
    {
        match claim.send(claimed) {
            Ok(()) => {
                return true;
            }
            Err(err) => {
                log::error!("Failed to send delete reply: {err:?}");
            }
        }
    }
    false
}
