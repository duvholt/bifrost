use std::ops::{AddAssign, Sub};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{
    ColorTemperatureUpdate, ColorUpdate, DimmingUpdate, LightGradientUpdate, On, ResourceLink,
};
use crate::date_format;

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneActive {
    Inactive,
    Static,
    DynamicPalette,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SceneStatus {
    pub active: SceneActive,
    #[serde(
        with = "date_format::utc_ms_opt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_recall: Option<DateTime<Utc>>,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneStatusEnum {
    Active,
    Static,
    DynamicPalette,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Scene {
    pub actions: Vec<SceneActionElement>,
    #[serde(default)]
    pub auto_dynamic: bool,
    pub group: ResourceLink,
    pub metadata: SceneMetadata,
    /* palette: { */
    /*     color: [], */
    /*     color_temperature: [ */
    /*         { */
    /*             color_temperature: { */
    /*                 mirek: u32 */
    /*             }, */
    /*             dimming: { */
    /*                 brightness: f64, */
    /*             } */
    /*         } */
    /*     ], */
    /*     dimming: [], */
    /*     effects: [] */
    /* }, */
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub palette: Value,
    #[serde(default)]
    pub speed: f64,
    pub status: Option<SceneStatus>,
    #[serde(default)]
    pub recall: SceneRecall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SceneAction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temperature: Option<ColorTemperatureUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimming: Option<DimmingUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<On>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<LightGradientUpdate>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub effects: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SceneActionElement {
    pub action: SceneAction,
    pub target: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SceneMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appdata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ResourceLink>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct SceneMetadataUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appdata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ResourceLink>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SceneUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<SceneActionElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recall: Option<SceneRecall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SceneMetadataUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_dynamic: Option<bool>,
}

impl SceneUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_actions(self, actions: Option<Vec<SceneActionElement>>) -> Self {
        Self { actions, ..self }
    }

    #[must_use]
    pub fn with_recall_action(self, action: Option<SceneStatus>) -> Self {
        Self {
            recall: Some(SceneRecall {
                action: match action.map(|a| a.active) {
                    Some(SceneActive::DynamicPalette) => Some(SceneStatusEnum::DynamicPalette),
                    Some(SceneActive::Static) => Some(SceneStatusEnum::Active),
                    Some(SceneActive::Inactive) | None => None,
                },
                duration: None,
                dimming: None,
            }),
            ..self
        }
    }
}

impl AddAssign<&SceneUpdate> for Scene {
    fn add_assign(&mut self, upd: &SceneUpdate) {
        if let Some(actions) = &upd.actions {
            self.actions.clone_from(actions);
        }
        if let Some(md) = &upd.metadata {
            self.metadata += md;
        }
        if let Some(palette) = &upd.palette {
            self.palette.clone_from(palette);
        }
        if let Some(speed) = upd.speed {
            self.speed = speed;
        }
        if let Some(auto_dynamic) = upd.auto_dynamic {
            self.auto_dynamic = auto_dynamic;
        }
    }
}

impl AddAssign<&SceneMetadataUpdate> for SceneMetadata {
    fn add_assign(&mut self, upd: &SceneMetadataUpdate) {
        if let Some(appdata) = &upd.appdata {
            self.appdata = Some(appdata.to_string());
        }
        if let Some(image) = &upd.image {
            self.image = Some(*image);
        }
        if let Some(name) = &upd.name {
            self.name.clone_from(name);
        }
    }
}

impl Sub<&SceneMetadata> for &SceneMetadata {
    type Output = SceneMetadataUpdate;

    fn sub(self, rhs: &SceneMetadata) -> Self::Output {
        let mut upd = Self::Output::default();

        if self != rhs {
            if self.appdata != rhs.appdata {
                upd.appdata.clone_from(&rhs.appdata);
            }
            if self.image != rhs.image {
                upd.image.clone_from(&rhs.image);
            }
            if self.name != rhs.name {
                upd.name = Some(rhs.name.clone());
            }
        }

        upd
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SceneRecall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<SceneStatusEnum>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimming: Option<DimmingUpdate>,
}
