use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    app_config::{BAMBU_COLOR_NAMES, BASE_FILAMENTS},
    spool_record::SpoolRecord,
};

use serde_with::serde_as;
use serde_with::hex::Hex;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BambuLabTag {
    tag_id: String,
    #[serde_as(as = "HashMap<_, Hex>")]
    blocks: HashMap<i32, Vec<u8>>,
}

pub const SPOOLEASE_V1_TAG_TYPE: &str = "SpoolEaseV1";
pub const BAMBULAB_TAG_TYPE: &str = "Bambu Lab";

// color_name
// https://raw.githubusercontent.com/bambulab/BambuStudio/refs/heads/master/resources/profiles/BBL/filament/filaments_color_codes.json
impl BambuLabTag {
    pub fn _material_variant_id(&self) -> String {
        // A00-G1 (Block 1, index 0..=7)
        self.get_block_cstr(1, 0, 8)
    }
    pub fn material_id(&self) -> String {
        // GFA00 (Block 1a (Block 1, index 8..=15)
        self.get_block_cstr(1, 8, 8)
    }
    pub fn _filament_type(&self) -> String {
        // PLA (Block 2) (Block 2, all)
        self.get_block_cstr(2, 0, 16)
    }
    pub fn _detailed_filament_type(&self) -> String {
        // PLA Basic (Block 4, all)
        self.get_block_cstr(4, 0, 16)
    }
    pub fn color_rgba(&self) -> String {
        // (Block 5, index 0..=3)
        if let Some(block) = self.blocks.get(&5) {
            let rgba = &block.as_slice()[0..=3];
            hex::encode_upper(rgba)
        } else {
            String::new()
        }
    }
    pub fn color_rgba2(&self) -> String {
        // (Block 16, index 4, reversed), Block 16, index 2 two bytes show how many colors
        if let Some(block) = self.blocks.get(&16) {
            let block = block.as_slice();
            let num_colors = i16::from_le_bytes([block[2], block[3]]) as i32;
            if num_colors > 1 {
                let rgba2 = [block[7], block[6], block[5], block[4]];
                hex::encode_upper(rgba2)
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } 
    pub fn spool_weight(&self) -> i32 {
        // 250g  (Block 5, index 4..=5)
        if let Some(block) = self.blocks.get(&5) {
            i16::from_le_bytes([block.as_slice()[4], block.as_slice()[5]]) as i32
        } else {
            0
        }
    } 

    pub fn new(tag_id_hex: &str, blocks: &HashMap<i32, Vec<u8>>) -> Self {
        BambuLabTag {
            tag_id: tag_id_hex.to_string(),
            blocks: blocks.clone(),
        }
    }
    fn get_block_cstr(&self, block: i32, start: usize, len: usize) -> String {
        if let Some(block) = self.blocks.get(&block) {
            Self::get_cstr(&block.as_slice()[start..start + len]).unwrap_or_default()
        } else {
            String::new()
        }
    }

    fn get_cstr(bytes: &[u8]) -> Result<String, core::str::Utf8Error> {
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        core::str::from_utf8(&bytes[..len]).map(|s| s.to_string())
    }

    pub fn to_spool_rec(&self) -> SpoolRecord {
        let material_id = self.material_id();
        let color_rgba = self.color_rgba();
        let color_rgba2 = self.color_rgba2();

        let (full_material, material_type) = BASE_FILAMENTS
            .lines()
            .find_map(|line| {
                let mut s = line.split(',');
                if s.next()? == material_id {
                    let full_material = s.next()?;
                    s.next()?;
                    s.next()?;
                    let material_type = s.next()?;
                    Some((full_material, material_type.to_string()))
                } else {
                    None
                }
            })
            .unwrap_or(("", String::new()));

        let color_name = BAMBU_COLOR_NAMES
            .lines()
            .find_map(|line| {
                let mut s = line.split(',');
                let id = s.next()?;
                if id != material_id {
                    return None;
                }

                let colors = s.next()?;
                let mut cs = colors.split('/');
                let c1 = cs.next()?;
                if c1 != color_rgba {
                    return None;
                }

                let c2 = cs.next().unwrap_or("");
                if c2 != color_rgba2 {
                    return None;
                }

                let name = s.next()?;
                let code = s.next()?;
                Some(format!("{name} ({code})"))
            })
            .unwrap_or_else(|| "(Fill Color-Name Manually)".to_string());

        let subtype_prefix = format!("Bambu {material_type} ");
        let material_subtype = if let Some(after_prefix) = full_material.strip_prefix(&subtype_prefix) {
            after_prefix.to_string()
        } else {
            String::new()
        };
        let spool_weight = self.spool_weight();

        SpoolRecord {
            material_type,
            material_subtype,
            color_name,
            color_code: color_rgba,
            brand: "Bambu".to_string(),
            weight_advertised: if spool_weight != 0 { Some(spool_weight) } else { None },
            weight_core: None,
            slicer_filament: material_id,
            data_origin: BAMBULAB_TAG_TYPE.to_string(),
            ..Default::default()
        }
    }
}
