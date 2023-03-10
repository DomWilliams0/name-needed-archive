use std::collections::HashMap;
use std::rc::Rc;

use common::*;

use crate::definitions::{DefinitionErrorKind, ValueImpl};
use crate::ecs;
use crate::ecs::{ComponentTemplate, ComponentTemplateEntry};
use crate::string::StringCache;

/// Holds all registered component template entries
pub struct TemplateLookup(HashMap<&'static str, ComponentTemplateEntry<ValueImpl>>);

impl TemplateLookup {
    pub fn init() -> Self {
        let mut templates = HashMap::new();

        for entry in inventory::iter::<ComponentTemplateEntry<ValueImpl>> {
            debug!("registering component template {key}", key = entry.key);
            templates.insert(entry.key, entry.clone());
        }
        Self(templates)
    }

    pub fn construct(
        &self,
        uid: &str,
        map: &mut ecs::Map<ValueImpl>,
        string_cache: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<ValueImpl>>, DefinitionErrorKind> {
        self.0
            .get(uid)
            .ok_or_else(|| DefinitionErrorKind::NoSuchComponent(uid.to_owned()))
            .and_then(|e| (e.construct_fn)(map, string_cache).map_err(DefinitionErrorKind::from))
    }
}
