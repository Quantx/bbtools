use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::bbtools::*;
use stg::Mission;
use xbe::XBE;

use glam::EulerRot;
use glam::Quat;
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::f32::Vec4;

const SB_MODEL_CONFIG_SIZE: usize = 10;
const LOC_MODEL_CONFIG_SIZE: usize = 8;
#[derive(Copy, Clone)]
pub struct ModelConfig {
    pub model: u16,
    pub animation: Option<u16>,
    pub sequence: Option<u16>,
    pub hitbox: Option<u16>,
    pub texture: Option<u16>,
}

impl ModelConfig {
    pub fn import_sb_a(buf: &[u8]) -> Self {
        assert!(buf.len() == SB_MODEL_CONFIG_SIZE);
        let model = LittleEndian::read_u16(&buf[0..2]);
        let animation = LittleEndian::read_u16(&buf[2..4]);
        let sequence = LittleEndian::read_u16(&buf[4..6]);
        let texture = LittleEndian::read_u16(&buf[6..8]);
        let hitbox = LittleEndian::read_u16(&buf[8..10]);

        assert!(model != u16::MAX);

        return ModelConfig {
            model: model,
            animation: if animation != u16::MAX {
                Some(animation)
            } else {
                None
            },
            sequence: if sequence != u16::MAX {
                Some(sequence)
            } else {
                None
            },
            texture: if texture != u16::MAX {
                Some(texture)
            } else {
                None
            },
            hitbox: if hitbox != u16::MAX {
                Some(hitbox)
            } else {
                None
            },
        };
    }

    pub fn import_sb_b(buf: &[u8]) -> Self {
        assert!(buf.len() == SB_MODEL_CONFIG_SIZE);
        let model = LittleEndian::read_u16(&buf[0..2]);
        let animation = LittleEndian::read_u16(&buf[2..4]);
        let texture = LittleEndian::read_u16(&buf[4..6]);
        let hitbox = LittleEndian::read_u16(&buf[6..8]);
        let sequence = LittleEndian::read_u16(&buf[8..10]);

        assert!(model != u16::MAX);

        return ModelConfig {
            model: model,
            animation: if animation != u16::MAX {
                Some(animation)
            } else {
                None
            },
            sequence: if sequence != u16::MAX {
                Some(sequence)
            } else {
                None
            },
            texture: if texture != u16::MAX {
                Some(texture)
            } else {
                None
            },
            hitbox: if hitbox != u16::MAX {
                Some(hitbox)
            } else {
                None
            },
        };
    }

    pub fn import_loc_a(buf: &[u8]) -> Self {
        assert!(buf.len() == LOC_MODEL_CONFIG_SIZE);
        let model = LittleEndian::read_u16(&buf[0..2]);
        let animation = LittleEndian::read_u16(&buf[2..4]);
        let sequence = LittleEndian::read_u16(&buf[4..6]);
        let hitbox = LittleEndian::read_u16(&buf[6..8]);

        assert!(model != u16::MAX);

        return ModelConfig {
            model: model,
            animation: if animation != u16::MAX {
                Some(animation)
            } else {
                None
            },
            sequence: if sequence != u16::MAX {
                Some(sequence)
            } else {
                None
            },
            texture: None,
            hitbox: if hitbox != u16::MAX {
                Some(hitbox)
            } else {
                None
            },
        };
    }

    pub fn import_loc_b(buf: &[u8]) -> Self {
        assert!(buf.len() == LOC_MODEL_CONFIG_SIZE);
        let model = LittleEndian::read_u16(&buf[0..2]);
        let animation = LittleEndian::read_u16(&buf[2..4]);
        let hitbox = LittleEndian::read_u16(&buf[4..6]);
        let sequence = LittleEndian::read_u16(&buf[6..8]);

        assert!(model != u16::MAX);

        return ModelConfig {
            model: model,
            animation: if animation != u16::MAX {
                Some(animation)
            } else {
                None
            },
            sequence: if sequence != u16::MAX {
                Some(sequence)
            } else {
                None
            },
            texture: None,
            hitbox: if hitbox != u16::MAX {
                Some(hitbox)
            } else {
                None
            },
        };
    }

    pub fn import_pointers(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
    ) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut pointers: Vec<u32> = vec![0; count];
        xbe.reader.read_u32_into::<LittleEndian>(&mut pointers)?;

        let mut model_configs: Vec<ModelConfig> = Vec::with_capacity(count);
        let mut model_config_buf: [u8; LOC_MODEL_CONFIG_SIZE] = [0; _];
        for pointer in pointers {
            xbe.seek_pointer_offset(pointer)?;

            xbe.reader.read(&mut model_config_buf)?;

            let model_config = ModelConfig::import_loc_a(model_config_buf.as_slice());
            //println!("Imported model config {:04}", model_config.model);
            model_configs.push(model_config);
        }

        return Ok(model_configs);
    }
}

impl fmt::Display for ModelConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ModelConfig: {}", self.model)?;

        if let Some(animation) = self.animation {
            write!(f, ", Animation: {}", animation)?;
        }

        if let Some(sequence) = self.sequence {
            write!(f, ", Sequence: {}", sequence)?;
        }

        if let Some(texture) = self.texture {
            write!(f, ", Texture: {}", texture)?;
        }

        if let Some(hitbox) = self.hitbox {
            write!(f, ", Hitbox: {}", hitbox)?;
        }

        return Ok(());
    }
}

const MECH_MODEL_CONFIG_SIZE: usize = 20;
#[derive(Copy, Clone)]
pub struct MechModelConfig {
    pub emblem_model: Option<u16>,
    pub chassis: ModelConfig,
    pub hatch: ModelConfig,
}

impl From<&[u8]> for MechModelConfig {
    fn from(buf: &[u8]) -> Self {
        assert!(buf.len() == MECH_MODEL_CONFIG_SIZE);
        assert!(LittleEndian::read_u16(&buf[18..20]) == 0);

        let emblem_model = LittleEndian::read_u16(&buf[0..2]);
        return MechModelConfig {
            emblem_model: if emblem_model != u16::MAX {
                Some(emblem_model)
            } else {
                None
            },
            chassis: ModelConfig::import_loc_a(&buf[2..10]),
            hatch: ModelConfig::import_loc_a(&buf[10..18]),
        };
    }
}

impl MechModelConfig {
    pub fn import_pointers(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
    ) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut pointers: Vec<u32> = vec![0; count];
        xbe.reader.read_u32_into::<LittleEndian>(&mut pointers)?;

        let mut mech_model_configs: Vec<MechModelConfig> = Vec::with_capacity(count);
        let mut mech_model_config_buf: [u8; MECH_MODEL_CONFIG_SIZE] = [0; _];
        for pointer in pointers {
            xbe.seek_pointer_offset(pointer)?;

            xbe.reader.read(&mut mech_model_config_buf)?;

            let mech_model_config = MechModelConfig::from(mech_model_config_buf.as_slice());
            //println!("Imported mech model config: Chassis {:04}, Hatch {:04}", mech_model_config.chassis.model, mech_model_config.hatch.model);
            mech_model_configs.push(mech_model_config);
        }

        return Ok(mech_model_configs);
    }
}

const WEAPON_MODEL_CONFIG_SIZE: usize = 24;
#[derive(Copy, Clone)]
pub struct WeaponModelConfig {
    pub projectile: Option<ModelConfig>,
    pub weapon: Option<ModelConfig>,
    pub _attr0: u16,
    pub _attr1: u16,
}

impl WeaponModelConfig {
    pub fn import_sb(buf: &[u8]) -> Self {
        assert!(buf.len() == WEAPON_MODEL_CONFIG_SIZE);

        let projectile_model_id = LittleEndian::read_u16(&buf[0..2]);
        let projectile = if projectile_model_id == u16::MAX {
            None
        } else {
            Some(ModelConfig::import_sb_b(&buf[0..10]))
        };

        let weapon_model_id = LittleEndian::read_u16(&buf[10..12]);
        let weapon = if weapon_model_id == u16::MAX {
            None
        } else {
            Some(ModelConfig::import_sb_b(&buf[10..20]))
        };

        return WeaponModelConfig {
            projectile,
            weapon,
            _attr0: LittleEndian::read_u16(&buf[20..22]),
            _attr1: LittleEndian::read_u16(&buf[22..24]),
        };
    }
}

const MODEL_CONFIG_FLAGS_SIZE: usize = 12;
#[derive(Copy, Clone)]
pub struct ModelConfigFlags {
    pub modcfg: ModelConfig,
    pub flags: u32,
}

impl ModelConfigFlags {
    pub fn import_a(buf: &[u8]) -> Self {
        assert!(buf.len() == MODEL_CONFIG_FLAGS_SIZE);
        return ModelConfigFlags {
            modcfg: ModelConfig::import_loc_a(&buf[0..8]),
            flags: LittleEndian::read_u32(&buf[8..12]),
        };
    }

    pub fn import_b(buf: &[u8]) -> Self {
        assert!(buf.len() == MODEL_CONFIG_FLAGS_SIZE);
        return ModelConfigFlags {
            modcfg: ModelConfig::import_loc_b(&buf[0..8]),
            flags: LittleEndian::read_u32(&buf[8..12]),
        };
    }

    pub fn import_pointers_b(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
    ) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut pointers: Vec<u32> = vec![0; count];
        xbe.reader.read_u32_into::<LittleEndian>(&mut pointers)?;

        let mut model_configs: Vec<ModelConfigFlags> = Vec::with_capacity(count);
        let mut model_configs_buf: [u8; MODEL_CONFIG_FLAGS_SIZE] = [0; _];
        for pointer in pointers {
            xbe.seek_pointer_offset(pointer)?;

            xbe.reader.read(&mut model_configs_buf)?;

            let model_config = ModelConfigFlags::import_b(model_configs_buf.as_slice());
            //println!("Imported weapon model config: Model {:04}, Flags {:08X}", model_config.modcfg.model, model_config.flags);
            model_configs.push(model_config);
        }

        return Ok(model_configs);
    }

    pub fn import_multiple_a(buf: &[u8], count: usize) -> Vec<Self> {
        let buf_end = count * MODEL_CONFIG_FLAGS_SIZE;
        let (model_config_list, []) = buf[..buf_end].as_chunks::<MODEL_CONFIG_FLAGS_SIZE>() else {
            panic!("buf length not a multiple of MODEL_CONFIG_FLAGS_SIZE");
        };

        let mut model_configs: Vec<ModelConfigFlags> = Vec::with_capacity(count);
        for model_config_buf in model_config_list.iter() {
            model_configs.push(ModelConfigFlags::import_a(model_config_buf));
        }

        return model_configs;
    }
}

impl fmt::Display for ModelConfigFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, Flags: {:08X}", self.modcfg, self.flags)
    }
}

pub struct ModelConfigDB {
    configs: BTreeMap<u16, ModelConfig>,
}

impl ModelConfigDB {
    pub fn new() -> Self {
        ModelConfigDB {
            configs: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, mc: &ModelConfig) {
        if let Some(old) = self.configs.insert(mc.model, *mc) {
            // Ensure duplicate copies are identical
            assert!(
                mc.animation == old.animation,
                "Model {:04}, New Anim {:?} != Old Anim {:?}",
                mc.model,
                mc.animation,
                old.animation
            );
            assert!(
                mc.sequence == old.sequence,
                "Model {:04}, New Seq {:?} != Old Seq {:?}",
                mc.model,
                mc.sequence,
                old.sequence
            );
            assert!(
                mc.texture == old.texture,
                "Model {:04}, New Texture {:?} != Old Texture {:?}",
                mc.model,
                mc.texture,
                old.texture
            );
            assert!(
                mc.hitbox == old.hitbox,
                "Model {:04}, New Hitbox {:?} != Old Hitbox {:?}",
                mc.model,
                mc.hitbox,
                old.hitbox
            );
        }
    }

    pub fn register_multiple(&mut self, mcs: &[ModelConfig]) {
        for mc in mcs.iter() {
            self.register(mc)
        }
    }

    pub fn get_sequence_table(&self, animation_count: usize) -> Vec<Option<u16>> {
        let mut sequence_table: Vec<Option<u16>> = vec![None; animation_count];
        for (_, config) in self.configs.iter() {
            if let Some(animation) = config.animation {
                if !sequence_table[animation as usize].is_none() {
                    assert!(sequence_table[animation as usize] == config.sequence);
                }

                sequence_table[animation as usize] = config.sequence;
            }
        }

        return sequence_table;
    }

    pub fn get_model(&self, id: u16) -> Option<&ModelConfig> {
        self.configs.get(&id)
    }
}

pub const POS_ROT_SIZE: usize = 24;

#[derive(Clone, Copy, Default)]
pub struct PosRot(pub Vec3, pub Vec3);
impl PosRot {
    pub fn import(buf: &[u8]) -> Self {
        assert!(buf.len() == POS_ROT_SIZE);
        PosRot(
            Vec3::new(
                LittleEndian::read_f32(&buf[0..4]),
                LittleEndian::read_f32(&buf[4..8]),
                LittleEndian::read_f32(&buf[8..12]),
            ),
            Vec3::new(
                LittleEndian::read_f32(&buf[12..16]),
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ),
        )
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.0.x)?;
        writer.write_f32::<LittleEndian>(self.0.y)?;
        writer.write_f32::<LittleEndian>(self.0.z)?;

        writer.write_f32::<LittleEndian>(self.1.x)?;
        writer.write_f32::<LittleEndian>(self.1.y)?;
        writer.write_f32::<LittleEndian>(self.1.z)?;

        return Ok(());
    }
}

const RSTART_SIZE: usize = 32;
const SB_OBJ_SIZE: usize = 92;
const LOC_OBJ_SIZE: usize = 96;
pub struct OBJ {
    life: i16,
    pub modcfg: ModelConfig,
    _attr0: u16,
    position: Vec3,
    rotation: Vec3,
    _alpha_clip: i8,
    _shadow_flag: i8,
    npc_spawn_idx: u16,
    team_id: i8,
    ticket_value: i8,
    _attr1: u16,
    flags: u32,
}

impl From<&[u8; LOC_OBJ_SIZE]> for OBJ {
    fn from(buf: &[u8; LOC_OBJ_SIZE]) -> Self {
        let zero: [u8; 48] = [0; _];
        assert!(&buf[48..LOC_OBJ_SIZE] == zero);

        return OBJ {
            life: LittleEndian::read_i16(&buf[0..2]),
            modcfg: ModelConfig::import_loc_a(&buf[2..10]),
            _attr0: LittleEndian::read_u16(&buf[10..12]),
            position: Vec3::new(
                LittleEndian::read_f32(&buf[12..16]),
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ),
            rotation: Vec3::new(
                LittleEndian::read_f32(&buf[24..28]),
                LittleEndian::read_f32(&buf[28..32]),
                LittleEndian::read_f32(&buf[32..36]),
            ),
            _alpha_clip: buf[36] as i8,
            _shadow_flag: buf[37] as i8,
            npc_spawn_idx: LittleEndian::read_u16(&buf[38..40]),
            team_id: buf[40] as i8,
            ticket_value: buf[41] as i8,
            _attr1: LittleEndian::read_u16(&buf[42..44]),
            flags: LittleEndian::read_u32(&buf[44..48]),
        };
    }
}

impl From<&[u8; SB_OBJ_SIZE]> for OBJ {
    fn from(buf: &[u8; SB_OBJ_SIZE]) -> Self {
        let zero: [u8; 46] = [0; _];
        assert!(&buf[46..SB_OBJ_SIZE] == zero);

        return OBJ {
            life: LittleEndian::read_i16(&buf[0..2]),
            modcfg: ModelConfig::import_sb_a(&buf[2..12]),
            _attr0: 0,
            position: Vec3::new(
                LittleEndian::read_f32(&buf[12..16]),
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ),
            rotation: Vec3::new(
                LittleEndian::read_f32(&buf[24..28]),
                LittleEndian::read_f32(&buf[28..32]),
                LittleEndian::read_f32(&buf[32..36]),
            ),
            _alpha_clip: buf[37] as i8,
            _shadow_flag: buf[38] as i8,
            npc_spawn_idx: LittleEndian::read_u16(&buf[38..40]),
            team_id: buf[40] as i8,
            ticket_value: buf[41] as i8,
            _attr1: LittleEndian::read_u16(&buf[42..44]),
            flags: LittleEndian::read_u32(&buf[40..44]),
        };
    }
}

impl OBJ {
    pub fn import_loc_objects(buf: &[u8], scale: f32) -> Vec<OBJ> {
        let (obj_list, []) = buf.as_chunks::<LOC_OBJ_SIZE>() else {
            panic!("buf length not a multiple of LOC_OBJ_SIZE");
        };

        let mut objs: Vec<OBJ> = Vec::with_capacity(obj_list.len());
        for (i, obj_buf) in obj_list.iter().enumerate() {
            let model_id = LittleEndian::read_u16(&obj_buf[2..4]);
            if model_id == u16::MAX {
                assert!(i == obj_list.len() - 1);
                continue;
            }

            let mut obj = OBJ::from(obj_buf);
            obj.position *= scale;
            if obj.modcfg.model > 805 && obj.modcfg.model < 1236 {
                // Ignore debug and cockpit models
                objs.push(obj);
            }
        }

        return objs;
    }

    pub fn import_sb_objects(buf: &[u8], scale: f32) -> Vec<OBJ> {
        let (obj_list, _) = buf.as_chunks::<SB_OBJ_SIZE>();

        let mut objs: Vec<OBJ> = Vec::with_capacity(obj_list.len());
        for obj_buf in obj_list.iter() {
            let model_id = LittleEndian::read_u16(&obj_buf[2..4]);
            if model_id == u16::MAX {
                break;
            }

            if model_id >= 833 && model_id <= 856 {
                // These models are skyboxes skip them
                continue;
            }

            // 277 = Destroyed radio tower
            // 542 = Empty model, just a skeleton
            // 384 = Resupply Helicopter with Continer
            // 357 = B-52 Bomber
            if model_id == 277 || model_id == 542 || model_id == 384 || model_id == 357 {
                // TODO: Figure out why these models have missmatched components
                continue;
            }

            let mut obj = OBJ::from(obj_buf);
            obj.position *= scale;

            if obj.modcfg.model == 357 && obj.modcfg.texture == Some(140) {
                obj.modcfg.texture = Some(139);
            }

            objs.push(obj);
        }

        return objs;
    }

    pub fn get_sb_skybox_texture(buf: &[u8]) -> Option<u16> {
        let (obj_list, _) = buf.as_chunks::<SB_OBJ_SIZE>();

        for obj_buf in obj_list.iter() {
            let model_id = LittleEndian::read_u16(&obj_buf[2..4]);
            if model_id == u16::MAX {
                break;
            }

            if model_id < 833 || model_id > 856 {
                // These models are NOT skyboxes skip them
                continue;
            }

            let obj = OBJ::from(obj_buf);
            return obj.modcfg.texture;
        }

        return None;
    }

    pub fn apply_rstart(
        objs: &mut Vec<OBJ>,
        path: &Path,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let buf = std::fs::read(path)?;

        if buf.len() != 0x144 {
            return Err("BadLength".into());
        }

        let magic = LittleEndian::read_u32(&buf[0..4]);
        if magic != 1 {
            return Err("BadMagic".into());
        }

        let (rstart_list, []) = buf[4..].as_chunks::<RSTART_SIZE>() else {
            panic!("buf length not a multiple of RSTART_SIZE");
        };

        let mut rstarts: Vec<Vec4> = Vec::with_capacity(10);
        for rstart_buf in rstart_list {
            rstarts.push(Vec4::new(
                LittleEndian::read_f32(&rstart_buf[0..4]),
                LittleEndian::read_f32(&rstart_buf[4..8]),
                LittleEndian::read_f32(&rstart_buf[8..12]),
                LittleEndian::read_f32(&rstart_buf[12..16]),
            ));
        }

        let mut rstart_idx: usize = 0;
        for obj in objs.iter_mut() {
            if obj.modcfg.model == 1184 {
                if rstart_idx < rstarts.len() {
                    obj.position.x = rstarts[rstart_idx].x;
                    obj.position.y = rstarts[rstart_idx].y;
                    obj.position.z = rstarts[rstart_idx].z;
                    obj.position *= scale;

                    obj.rotation.y = rstarts[rstart_idx].w;
                }
                rstart_idx += 1;
            }
        }

        if rstart_idx != 10 {
            return Err("BadRStartCount".into());
        }

        return Ok(());
    }

    pub fn apply_heightmap(objs: &mut [OBJ], mission: &Mission, flags: u32) {
        for obj in objs {
            if obj.flags & flags == 0 // Don't place object on ground if this flag is set
                && let Some(height) = mission.sample_height(obj.position)
            {
                obj.position.y = height;
            }
        }
    }

    pub fn write_objects(
        objs: &[OBJ],
        offset: Vec3,
        model_path_fn: FnIndexToPath,
        writer: &mut impl Write,
    ) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(objs.len() as u32)?;
        for obj in objs {
            // Write model name
            let model_path = model_path_fn(obj.modcfg.model as usize);
            write_godot_path(&model_path, writer)?;

            writer.write_i16::<LittleEndian>(obj.life)?;
            writer.write_u16::<LittleEndian>(obj.modcfg.model)?;

            writer.write_u32::<LittleEndian>(obj.flags)?;

            writer.write_i8(obj.team_id)?;
            writer.write_i8(obj.ticket_value)?;
            writer.write_u16::<LittleEndian>(obj.npc_spawn_idx)?;

            let position = obj.position + offset;
            for v in position.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }

            let quat = Quat::from_euler(
                EulerRot::XYZEx,
                obj.rotation.x,
                obj.rotation.y,
                obj.rotation.z,
            );
            for v in quat.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }

        return Ok(());
    }
}
