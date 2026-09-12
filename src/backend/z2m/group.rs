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

    pub fn set_used_group_ids(&mut self, groups: &[z2m::api::Group]) {
        self.used_group_ids = groups.iter().map(|g| g.id).collect();
    }

    pub fn reserve_group_id(&mut self, group_id: u32) {
        self.used_group_ids.insert(group_id);
    }

    pub fn get_next_group_id(&self) -> u32 {
        for x in 1.. {
            if !self.used_group_ids.contains(&x) {
                return x;
            }
        }
        unreachable!()
    }
}
