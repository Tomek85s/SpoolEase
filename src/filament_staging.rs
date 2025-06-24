use alloc::rc::Rc;

use crate::{bambu::TagInformation, store::Store};

pub struct FilamentStaging {
    tag_info: Option<TagInformation>,
    origin: StagingOrigin,
    store: Rc<Store>
}

#[derive(PartialEq)]
pub enum StagingOrigin {
    Empty,
    Scanned,
    Encoded,
    Unloaded,
}

impl FilamentStaging {
    pub fn new(store: Rc<Store>) -> Self {
        Self { tag_info: None, origin: StagingOrigin::Empty, store: store.clone() }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tag_info.is_none()
    }

    pub fn clear(&mut self) {
        self.tag_info = None;
        self.origin = StagingOrigin::Empty;
    }
    pub fn tag_info(&self) -> &Option<TagInformation> {
        &self.tag_info
    }
    pub fn tag_info_mut(&mut self) -> &mut Option<TagInformation> {
        &mut self.tag_info
    }

    pub fn set_tag_info(&mut self, mut tag_info: TagInformation, origin: StagingOrigin) {
        // if loaded in scanning scenario or unloading scenario, the store should reflect some of the fields
        if [StagingOrigin::Scanned, StagingOrigin::Unloaded].contains(&origin) {
            if let Some(tag_id) = &tag_info.tag_id {
                if let Some(spool_in_store) = self.store.get_spool_by_tag_id(tag_id) {
                    tag_info.note = Some(spool_in_store.note);
                }
            }
        }
        self.tag_info = Some(tag_info);
        self.origin = origin;
    }
    pub fn origin(&self) -> &StagingOrigin {
        &self.origin
    }
    
}
