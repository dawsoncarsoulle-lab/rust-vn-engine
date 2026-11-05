//! Typed Widget data shared by the existing evaluator and its portable adapter.
//!
//! Decoding a reference yields inert data. Only an Engine which has admitted
//! the class and exact resident target may bind it to its own runtime epoch.
//! These values never carry viewport, focus, input or native wrapper authority.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetClassReference {
    uuid: String,
    receipt: [u8; 32],
}
impl WidgetClassReference {
    pub fn new(uuid: String, receipt: [u8; 32]) -> Result<Self, String> {
        crate::localized_text::validate_text_asset_id(&uuid)?;
        Ok(Self { uuid, receipt })
    }
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
    pub fn receipt(&self) -> &[u8; 32] {
        &self.receipt
    }
    pub fn validate(&self) -> Result<(), String> {
        crate::localized_text::validate_text_asset_id(&self.uuid)
    }
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.uuid.len()
    }
}

/// A stable link inside one saved logical instance graph, not a live handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableWidgetReference {
    class: WidgetClassReference,
    alias: String,
    incarnation: u64,
}
impl PortableWidgetReference {
    pub fn new(
        class: WidgetClassReference,
        alias: String,
        incarnation: u64,
    ) -> Result<Self, String> {
        let value = Self {
            class,
            alias,
            incarnation,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn class(&self) -> &WidgetClassReference {
        &self.class
    }
    pub fn alias(&self) -> &str {
        &self.alias
    }
    pub fn incarnation(&self) -> u64 {
        self.incarnation
    }
    pub fn validate(&self) -> Result<(), String> {
        self.class.validate()?;
        if self.alias.is_empty()
            || self.alias.len() > 128
            || self.alias.chars().any(char::is_control)
            || self.incarnation == 0
        {
            return Err("Invalid portable Widget target identity".into());
        }
        Ok(())
    }
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.class.uuid.len() + self.alias.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeBinding {
    instance: u64,
    epoch: u64,
}

/// The runtime binding is deliberately absent from every serialized format.
/// Equality includes it: cold decoding cannot revive a pre-load live reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetReference {
    portable: Option<PortableWidgetReference>,
    binding: Option<RuntimeBinding>,
}
impl WidgetReference {
    pub fn portable(&self) -> Option<&PortableWidgetReference> {
        self.portable.as_ref()
    }
    pub fn null() -> Self {
        Self {
            portable: None,
            binding: None,
        }
    }
    pub fn is_null(&self) -> bool {
        self.portable.is_none()
    }
    pub fn is_bound_to(&self, instance: u64, epoch: u64) -> bool {
        self.binding == Some(RuntimeBinding { instance, epoch })
    }
    pub fn is_inert(&self) -> bool {
        self.portable.is_some() && self.binding.is_none()
    }
    pub fn was_bound_to_instance(&self, instance: u64) -> bool {
        self.binding
            .is_some_and(|binding| binding.instance == instance)
    }
    pub fn inert_data(&self) -> Self {
        Self {
            portable: self.portable.clone(),
            binding: None,
        }
    }
    /// An internal Rust bridge, not admission. Engines must validate the exact
    /// class/source and resident target before binding and on every use.
    pub fn bind_admitted(
        portable: PortableWidgetReference,
        instance: u64,
        epoch: u64,
    ) -> Result<Self, String> {
        portable.validate()?;
        if instance == 0 {
            return Err("Widget reference requires a real Engine lineage".into());
        }
        Ok(Self {
            portable: Some(portable),
            binding: Some(RuntimeBinding { instance, epoch }),
        })
    }
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.portable.as_ref().map_or(0, |portable| {
                portable.class.uuid.len() + portable.alias.len()
            })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "reference",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum WidgetReferenceWire {
    Null,
    Object(PortableWidgetReference),
}
impl Serialize for WidgetReference {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.portable {
            None => WidgetReferenceWire::Null.serialize(serializer),
            Some(portable) => {
                portable.validate().map_err(serde::ser::Error::custom)?;
                WidgetReferenceWire::Object(portable.clone()).serialize(serializer)
            }
        }
    }
}
impl<'de> Deserialize<'de> for WidgetReference {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match WidgetReferenceWire::deserialize(deserializer)? {
            WidgetReferenceWire::Null => Ok(Self::null()),
            WidgetReferenceWire::Object(portable) => {
                portable.validate().map_err(serde::de::Error::custom)?;
                Ok(Self {
                    portable: Some(portable),
                    binding: None,
                })
            }
        }
    }
}

#[cfg(test)]
#[path = "widget_reference_tests_0974.rs"]
mod tests;
