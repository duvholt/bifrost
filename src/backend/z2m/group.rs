use hue::api::ResourceLink;

use crate::{backend::z2m::Z2mBackend, model::state::AuxData, resource::Resources};

impl Z2mBackend {
    pub fn set_group_aux(
        &self,
        res: &mut Resources,
        link_glight: ResourceLink,
        group_id: u32,
        topic: Option<&str>,
    ) {
        let mut aux = AuxData::new().with_index(group_id).with_backend(&self.name);
        if let Some(topic) = topic {
            aux = aux.with_topic(topic);
        }

        res.aux_set(&link_glight, aux);
    }
}
