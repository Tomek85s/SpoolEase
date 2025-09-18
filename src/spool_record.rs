use alloc::{format, string::{String, ToString}};
use serde::{Deserialize, Serialize};
use shared::utils::{
    deserialize_bool_yn_empty_n, deserialize_f32_base64, deserialize_optional, deserialize_optional_bool_yn, serialize_bool_yn, serialize_f32_base64,
    serialize_optional_bool_yn,
};
use crate::{
    bambu::{KInfo, KNozzleId},
    csvdb::CsvDbId,
};

// TODO: think if to change it to get the spoolRecord from store (and it will hold Rc to store)
#[derive(Debug, Clone, Default)]
pub struct FullSpoolRecord {
    pub spool_rec: SpoolRecord,
    pub spool_rec_ext: SpoolRecordExt,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub struct SpoolRecord {
    pub id: String,
    pub tag_id: String,           // 14 (7*2)
    pub material_type: String,    // 10
    pub material_subtype: String, // 10
    pub color_name: String,       // 10
    pub color_code: String,       // 8
    pub note: String,             // 40
    pub brand: String,            // 30
    #[serde(deserialize_with = "deserialize_optional")]
    pub weight_advertised: Option<i32>, // 4
    #[serde(deserialize_with = "deserialize_optional")]
    pub weight_core: Option<i32>, // 4
    #[serde(deserialize_with = "deserialize_optional")] pub weight_new: Option<i32>, // 4
    #[serde(deserialize_with = "deserialize_optional")]
    pub weight_current: Option<i32>, // 4
    #[serde(default)]
    pub slicer_filament: String,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub added_time: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub encode_time: Option<i32>,
    #[serde(default, serialize_with = "serialize_optional_bool_yn", deserialize_with = "deserialize_optional_bool_yn")]
    pub added_full: Option<bool>,
    #[serde(default, serialize_with = "serialize_f32_base64", deserialize_with = "deserialize_f32_base64")]
    pub consumed_since_add: f32,
    #[serde(default, serialize_with = "serialize_f32_base64", deserialize_with = "deserialize_f32_base64")]
    pub consumed_since_weight: f32,
    #[serde(default, serialize_with = "serialize_bool_yn", deserialize_with = "deserialize_bool_yn_empty_n")]
    pub ext_has_k: bool,
    // pub update_time
    // pub update_tag_fields_time
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub price: Option<()>,
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub grade/quality: Option<()>,
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub diameter: Option<()>,
    //
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub location: Option<()>,
    //
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub purchased: Option<()>,
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub opened: Option<()>,
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub dried: Option<()>,
    // #[serde(default,deserialize_with = "deserialize_optional_unit")]
    // pub last_used: Option<()>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SpoolRecordExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_info: Option<KInfo>,
}

impl SpoolRecordExt {
    pub fn get_calibrations(&self, printer: &str, extruder: i32, diameter: &str, nozzle_id: &str) -> Option<&KNozzleId> {
        let res = self
            .k_info
            .as_ref()?
            .printers
            .get(printer)?
            .extruders
            .get(&extruder)?
            .diameters
            .get(diameter)?
            .nozzles
            .get(nozzle_id);
        res
    }
}

impl CsvDbId for SpoolRecord {
    fn id(&self) -> &String {
        &self.id
    }
}

const TAG_URL_PREFIX_V2: &str = "https://info.filament3d.org/V2/";
        // Some(format!("{FILAMENT_URL_PREFIX}V1?ID={TAG_PLACEHOLDER}{encode_time_part}{material_part}
        // {filament_subtype_part}{color_part}{color_name_part}{brand_part}{advertised_weight_part}{weight_core_part}{weight_new_part}{nozzle_temp_min_part}{nozzle_temp_max_part}{note_part}{tray_info_idx_part}"))
// TODO:    
// 1. Add slicer_filament_name - derive from slicer,mfilament_code or from material_type if slicer not filled in, use get_filament_info for that
// 2. Add temperatures - use get_filament_info for that 
// 3. Add note - and fully url encode it
// 4. ? Add added time

impl SpoolRecord {
    pub fn to_tag_descriptor_v2(&self) -> Option<String> {
        if self.id.is_empty() || self.tag_id.is_empty() || self.material_type.is_empty() || self.color_code.is_empty() {
            return None;
        }
        let encode_time_part = part_opt("DE", &self.encode_time);
        let id = &self.id;
        let tag_id = &self.tag_id;
        let material = &self.material_type;
        let material_subtype_part = part_val("MS", &self.material_subtype);
        let brand_part = part_val("B", &self.brand);
        let weight_advertised_part = part_opt("WA", &self.weight_advertised);
        let weight_core_part = part_opt("WC", &self.weight_core);
        let weight_new_part = part_opt("WN", &self.weight_new);
        let slicer_filament_code_part = part_val("FI", &self.slicer_filament);
        Some(format!("{TAG_URL_PREFIX_V2}?TG={tag_id}&ID={id}{encode_time_part}&M={material}{material_subtype_part}{brand_part}{weight_advertised_part}{weight_core_part}{weight_new_part}{slicer_filament_code_part}"))
    }
}

pub fn part_opt<T: Default+PartialEq+core::fmt::Display>(prefix: &str, opt: &Option<T>) -> String {
    if matches!(opt, Some(v) if *v!= T::default()) { format!("&{prefix}={}", opt.as_ref().unwrap()) } else {"".to_string()}
}

pub fn part_val<T: Default+PartialEq+core::fmt::Display>(prefix: &str, val: &T) -> String {
    if *val != T::default() { format!("&{prefix}={}", val) } else {"".to_string()}
}
