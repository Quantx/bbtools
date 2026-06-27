use std::array;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};

use glam::f32::Vec2;
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD

use crate::bbtools::*;
use eff::EffectPosition;
use obj::{MechModelConfig, ModelConfig};
use xbe::XBE;

const MECH_COUNT: usize = 35;

pub struct RpmTorque(f32, f32);
impl From<&[u8]> for RpmTorque {
    fn from(buf: &[u8]) -> Self {
        assert!(buf.len() == 8);
        RpmTorque(
            LittleEndian::read_f32(&buf[0..4]),
            LittleEndian::read_f32(&buf[4..8]),
        )
    }
}

impl RpmTorque {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.0)?;
        writer.write_f32::<LittleEndian>(self.1)?;
        return Ok(());
    }
}

const ENGINE_DATA_SIZE: usize = 192;
const MECH_GENERATIONS: [u8; 6] = [0, 1, 2, 1, 0, 1];
pub struct EngineData {
    id: u8,
    weight: f32,
    tier_r: f32,
    gears: [f32; 6], // 0=Reverse, 1=1st, ... , 5=5th
    gear_f: f32,
    brake: f32,
    rpm_min: f32,
    override_multiplier: RpmTorque,
    wheel_torque: f32,
    wheel_start_speed: f32,
    drag_coefficient: f32,
    rpm_rate: f32,
    drag_size: Vec2,
    internal_resistance: f32,
    engine_curve: [RpmTorque; 4],
    turn_speed: f32,
    balancer: f32,
    health_torso: u16,
    health_leg_l: u16,
    health_leg_r: u16,
    health_opt_armor: u16,
    tank_capacity_main: f32,
    tank_capacity_sub: f32,
    resistance_front: f32,
    resistance_side: f32,
    resistance_rear: f32,
    cockpit_type: u8,
    ticket_cost: u8,
    slope_gear_a: u8,
    slope_gear_b: u8,
    loadout_weight_max: u8,
    loadout_weight_standard: u8,
}

impl From<&[u8; ENGINE_DATA_SIZE]> for EngineData {
    fn from(buf: &[u8; ENGINE_DATA_SIZE]) -> Self {
        let id = LittleEndian::read_u32(&buf[0..4]);
        assert!(id <= u8::MAX as u32);

        let mut gears: [f32; 6] = [0.0; _];
        LittleEndian::read_f32_into(&buf[12..36], &mut gears);

        // sub_tank_number = &buf[132..136]

        let tank_capacity_sub = LittleEndian::read_f32(&buf[140..144]);

        // tank_consumption = &buf[144..148]
        // battery_capacity = &buf[152..156]

        let cockpit_type = LittleEndian::read_u32(&buf[156..160]);
        assert!((cockpit_type as usize) < MECH_GENERATIONS.len());

        let ticket_cost = LittleEndian::read_u32(&buf[160..164]);
        assert!(ticket_cost <= u8::MAX as u32);

        let slope_gear_a = LittleEndian::read_u32(&buf[164..168]);
        assert!(slope_gear_a <= u8::MAX as u32);

        let slope_gear_b = LittleEndian::read_u32(&buf[168..172]);
        assert!(slope_gear_b <= u8::MAX as u32);

        // price = &buf[172..176]

        let loadout_weight_max = LittleEndian::read_u32(&buf[176..180]);
        assert!(loadout_weight_max <= u8::MAX as u32);

        let loadout_weight_standard = LittleEndian::read_u32(&buf[180..184]);
        assert!(loadout_weight_standard <= u8::MAX as u32);

        let height = LittleEndian::read_f32(&buf[72..76]);
        let width = LittleEndian::read_f32(&buf[76..80]);

        return EngineData {
            id: id as u8,
            weight: LittleEndian::read_f32(&buf[4..8]),
            tier_r: LittleEndian::read_f32(&buf[8..12]),
            gears: gears,
            gear_f: LittleEndian::read_f32(&buf[36..40]),
            brake: LittleEndian::read_f32(&buf[40..44]),
            rpm_min: LittleEndian::read_f32(&buf[44..48]),
            override_multiplier: RpmTorque::from(&buf[48..56]),
            wheel_torque: LittleEndian::read_f32(&buf[56..60]),
            wheel_start_speed: LittleEndian::read_f32(&buf[60..64]),
            drag_coefficient: LittleEndian::read_f32(&buf[64..68]),
            rpm_rate: LittleEndian::read_f32(&buf[68..72]),
            drag_size: Vec2::new(width, height),
            internal_resistance: LittleEndian::read_f32(&buf[80..84]),
            engine_curve: [
                RpmTorque::from(&buf[84..92]),
                RpmTorque::from(&buf[92..100]),
                RpmTorque::from(&buf[100..108]),
                RpmTorque::from(&buf[108..116]),
            ],
            turn_speed: LittleEndian::read_f32(&buf[116..120]),
            balancer: LittleEndian::read_f32(&buf[120..124]),
            health_torso: LittleEndian::read_u16(&buf[124..126]),
            health_leg_l: LittleEndian::read_u16(&buf[126..128]),
            health_leg_r: LittleEndian::read_u16(&buf[128..130]),
            health_opt_armor: LittleEndian::read_u16(&buf[130..132]),
            tank_capacity_main: LittleEndian::read_f32(&buf[136..140]),
            tank_capacity_sub: if tank_capacity_sub > 0.0 {
                tank_capacity_sub
            } else {
                10000.0
            },
            resistance_front: LittleEndian::read_f32(&buf[148..152]),
            resistance_side: LittleEndian::read_f32(&buf[184..188]),
            resistance_rear: LittleEndian::read_f32(&buf[188..192]),
            cockpit_type: cockpit_type as u8,
            ticket_cost: ticket_cost as u8,
            slope_gear_a: slope_gear_a as u8,
            slope_gear_b: slope_gear_b as u8,
            loadout_weight_max: loadout_weight_max as u8,
            loadout_weight_standard: loadout_weight_standard as u8,
        };
    }
}

impl EngineData {
    pub fn import(path: &Path) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let file_count = reader.read_u32::<LittleEndian>()? as usize;
        assert!(file_count == MECH_COUNT);
        let mut file_sizes: Vec<u32> = vec![0; file_count];
        reader.read_u32_into::<LittleEndian>(&mut file_sizes)?;

        let mut engine_data_list: Vec<Self> = Vec::with_capacity(file_count);

        let mut file_buf: [u8; ENGINE_DATA_SIZE] = [0; _];
        for (i, file_size) in file_sizes.into_iter().enumerate() {
            if file_size as usize != ENGINE_DATA_SIZE {
                // Skip to next file
                println!(
                    "Skipping engine data entry {}, invalid size {}, expected {}",
                    i, file_size, ENGINE_DATA_SIZE
                );
                reader.seek_relative(file_size as i64)?;
                continue;
            }

            reader.read(&mut file_buf)?;
            engine_data_list.push(EngineData::from(&file_buf));
        }

        return Ok(engine_data_list);
    }

    pub fn get_generation(&self) -> u8 {
        MECH_GENERATIONS[self.cockpit_type as usize]
    }
}

const LOADOUT_SLOT_COUNT: usize = 9;
#[derive(Clone)]
pub struct Loadout {
    mwep_ids: Vec<u8>,
    mwep_presets: [u8; 3],
    mwep_fixeds: [bool; 3],
    swep_ids: Vec<u8>,
    swep_presets: [u8; 3],
    swep_fixeds: [bool; 3],
    tank_count_max: u8,
    tank_count_preset: u8,

    weight_type: u8, // 0=Light, 1=Medium, 2=Heavy
    class_type: u8,  // 0=Standard, 1=Support, 2=Scount, 3=Assult
    profile_description: u8,
    mounts: u8,
}

impl Loadout {
    pub fn import(xbe: &mut XBE) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(".data", 0x61D70)?;
        let mut preset_pointers: [u32; MECH_COUNT] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut preset_pointers)?;

        let mut presets: Vec<[u8; LOADOUT_SLOT_COUNT]> = Vec::with_capacity(MECH_COUNT);
        for pointer in preset_pointers {
            let mut preset: [u8; LOADOUT_SLOT_COUNT] = [0xFF; _];

            if pointer != 0 {
                xbe.seek_pointer_offset(pointer)?;
                xbe.reader.read(&mut preset)?;
            }

            presets.push(preset);
        }

        xbe.seek_section_offset(".data", 0x61E00)?;
        let mut fixed_pointers: [u32; MECH_COUNT] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut fixed_pointers)?;

        let mut fixeds: Vec<[bool; LOADOUT_SLOT_COUNT]> = Vec::with_capacity(MECH_COUNT);
        for pointer in fixed_pointers {
            let mut fixed: [u8; LOADOUT_SLOT_COUNT] = [0; _];

            if pointer != 0 && xbe.seek_pointer_offset(pointer).is_ok() {
                xbe.reader.read(&mut fixed)?;
            }

            fixeds.push(fixed.map(|f| f != 0));
        }

        xbe.seek_section_offset(".data", 0x668E0)?;
        let mut loadouts: Vec<Loadout> = Vec::with_capacity(MECH_COUNT);
        for i in 0..MECH_COUNT {
            let mweps_pointer = xbe.reader.read_u32::<LittleEndian>()?;
            let sweps_pointer = xbe.reader.read_u32::<LittleEndian>()?;
            let tanks_pointer = xbe.reader.read_u32::<LittleEndian>()?;
            xbe.reader.seek_relative(5)?;

            let weight_type = xbe.reader.read_u8()?;
            let class_type = xbe.reader.read_u8()?;
            let profile_description = xbe.reader.read_u8()?;
            let mounts = xbe.reader.read_u32::<LittleEndian>()?;
            assert!(mounts <= u8::MAX as u32, "Invalid mount {}", mounts);

            // Record current position so that we can return to it
            let pos = xbe.reader.stream_position()?;

            let mut mwep_ids: Vec<u8> = Vec::with_capacity(16);
            if mweps_pointer != 0 {
                xbe.seek_pointer_offset(mweps_pointer)?;
                xbe.reader.read_until(0xFE, &mut mwep_ids)?;
            }
            mwep_ids.retain(|&v| v < 0xFE);

            let mut swep_ids: Vec<u8> = Vec::with_capacity(16);
            if sweps_pointer != 0 {
                xbe.seek_pointer_offset(sweps_pointer)?;
                xbe.reader.read_until(0xFE, &mut swep_ids)?;
            }
            swep_ids.retain(|&v| v < 0xFE);

            let mut tanks: Vec<u8> = Vec::with_capacity(16);
            if tanks_pointer != 0 {
                xbe.seek_pointer_offset(tanks_pointer)?;
                xbe.reader.read_until(0xFE, &mut tanks)?;
            }
            tanks.retain(|&v| v < 0xFE);

            for tank in tanks.iter_mut() {
                *tank = tank.wrapping_add(1);
            }

            // Return to previous position
            xbe.reader.seek(SeekFrom::Start(pos))?;

            let preset = &presets[i];

            let mut mwep_presets: [u8; 3] = [0xFF; _];
            for j in 0..3 {
                mwep_presets[j] = mwep_ids
                    .iter()
                    .position(|id| preset[j] == *id)
                    .unwrap_or(0xFF) as u8
            }

            let mut swep_presets: [u8; 3] = [0xFF; _];
            for j in 3..6 {
                swep_presets[j - 3] = swep_ids
                    .iter()
                    .position(|id| preset[j] == *id)
                    .unwrap_or(0xFF) as u8
            }

            let tank_count_preset = if let Some(tank_count) = tanks.get(preset[7] as usize) {
                *tank_count
            } else {
                0
            };

            loadouts.push(Loadout {
                mwep_ids,
                mwep_presets,
                mwep_fixeds: fixeds[i][0..3].try_into().unwrap(),
                swep_ids,
                swep_presets,
                swep_fixeds: fixeds[i][3..6].try_into().unwrap(),
                tank_count_max: tanks.into_iter().max().unwrap_or(0),
                tank_count_preset,
                weight_type,
                class_type,
                profile_description,
                mounts: mounts as u8,
            });
        }

        return Ok(loadouts);
    }
}

pub struct MechParts {
    id: u8,
    collider_size: Vec3,
    collider_offset: Vec3,
    manipulator: u8,
    weapon_mount: u8,
    _unknown: u8,
}

impl MechParts {
    pub fn import(xbe: &mut XBE, scale: f32) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(".data", 0x57A40)?;

        let mut mech_parts: Vec<MechParts> = Vec::with_capacity(MECH_COUNT);
        for _ in 0..MECH_COUNT {
            let id = xbe.reader.read_u32::<LittleEndian>()?;
            assert!(id <= u8::MAX as u32);

            let collider_offset_z = xbe.reader.read_f32::<LittleEndian>()? * scale;

            let manipulator = xbe.reader.read_u32::<LittleEndian>()?;
            assert!(manipulator <= u8::MAX as u32);

            let collider_size = Vec3::new(
                xbe.reader.read_f32::<LittleEndian>()?,
                xbe.reader.read_f32::<LittleEndian>()?,
                xbe.reader.read_f32::<LittleEndian>()?,
            ) * scale;

            let weapon_mount = xbe.reader.read_u32::<LittleEndian>()?;
            assert!(weapon_mount <= u8::MAX as u32);

            let unknown = xbe.reader.read_u32::<LittleEndian>()?;
            assert!(unknown <= u8::MAX as u32);

            mech_parts.push(MechParts {
                id: id as u8,
                collider_size: collider_size,
                collider_offset: Vec3::new(0.0, collider_size.y * 0.5, collider_offset_z),
                manipulator: manipulator as u8,
                weapon_mount: weapon_mount as u8,
                _unknown: unknown as u8,
            });
        }

        return Ok(mech_parts);
    }
}

const MECH_CAMO_COUNT: usize = 4;
const MECH_PAINT_AREA_COUNT: usize = 11;
const MECH_PAINT_COLOR_COUNT: usize = MECH_CAMO_COUNT * MECH_PAINT_AREA_COUNT;
const MECH_FACTION_FLAGS: [u8; MECH_COUNT] = [
    0x9, 0x9, 0x1, 0x1, 0x0, 0x2, 0x2, 0x2, 0x1, 0x1, 0x2, 0x2, 0x2, 0x1, 0x2, 0x2, 0x8, 0x8, 0x8,
    0x1, 0x1, 0x1, 0x1, 0x4, 0x1, 0x4, 0x4, 0x4, 0x8, 0x4, 0x4, 0x2, 0x0, 0x0, 0x0,
];
pub struct Mech {
    name_text: u16,
    description_text: u16,
    modcfgs: [MechModelConfig; MECH_CAMO_COUNT],
    manipulator: ModelConfig,
    weapon_mount: ModelConfig,
    manufacturer: u8,
    faction_flags: u8,
    collider_size: Vec3,
    collider_offset: Vec3,
    data: EngineData,
    loadout: Loadout,
    eye: EffectPosition,
    paint_palettes: [[Vec3; MECH_PAINT_AREA_COUNT]; MECH_CAMO_COUNT],
}

impl Mech {
    pub fn import(
        xbe: &mut XBE,
        engdat_path: &Path,
        scale: f32,
    ) -> Result<Vec<Self>, std::io::Error> {
        let engine_data = EngineData::import(engdat_path)?;

        let mech_models = MechModelConfig::import_pointers(xbe, ".data", 0x581C0, MECH_COUNT)?;

        let weapon_mount_models = ModelConfig::import_pointers(xbe, ".data", 0x5824C, 6)?;

        let manipulator_models = ModelConfig::import_pointers(xbe, ".data", 0x58264, 3)?;

        let loadouts = Loadout::import(xbe)?;

        let parts = MechParts::import(xbe, scale)?;

        let eyes = EffectPosition::import(xbe, ".data", 0x56118, MECH_COUNT, scale)?;

        xbe.seek_section_offset(".data", 0x345C0)?;
        let mut manufacturers: [u32; MECH_COUNT] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut manufacturers)?;

        let rgb_colors: [Vec3; MECH_COUNT * MECH_PAINT_COLOR_COUNT] = {
            xbe.seek_section_offset(".data", 0x61E90)?;
            let mut hsl_color_components: [f32; MECH_COUNT * MECH_PAINT_COLOR_COUNT * 3] = [0.0; _];
            xbe.reader
                .read_f32_into::<LittleEndian>(&mut hsl_color_components)?;
            array::from_fn(|i| {
                let hsl = Vec3::from_slice(&hsl_color_components[i * 3..i * 3 + 3]);
                hsl_to_rgb(hsl)
            })
        };

        let mut mechs: Vec<Mech> = Vec::with_capacity(MECH_COUNT);
        for data in engine_data {
            let id = data.id as usize;
            let manufacturer = manufacturers[id];
            assert!(manufacturer <= u8::MAX as u32);

            let part = &parts[id];
            assert!(data.id + 1 == part.id);

            let mut modcfgs: [MechModelConfig; MECH_CAMO_COUNT] = [mech_models[id]; _];
            modcfgs[1].chassis.model += 2;
            modcfgs[1].hatch.model += 2;
            modcfgs[2].chassis.model += 4;
            modcfgs[2].hatch.model += 4;
            modcfgs[3].chassis.model += 6;
            modcfgs[3].hatch.model += 6;

            //println!("Weapon Mount: {}", part.weapon_mount);

            let paint_colors = &rgb_colors
                [id * MECH_PAINT_COLOR_COUNT..id * MECH_PAINT_COLOR_COUNT + MECH_PAINT_COLOR_COUNT];
            let (paint_palettes, []) = paint_colors.as_chunks::<MECH_PAINT_AREA_COUNT>() else {
                panic!("paint_colors length not a multiple of MECH_PAINT_AREA_COUNT");
            };

            mechs.push(Mech {
                name_text: id as u16 + 1399,
                description_text: id as u16 + 688,
                modcfgs,
                manipulator: manipulator_models[part.manipulator as usize],
                weapon_mount: weapon_mount_models[part.weapon_mount as usize],
                manufacturer: manufacturer as u8,
                faction_flags: MECH_FACTION_FLAGS[id],
                collider_size: part.collider_size,
                collider_offset: part.collider_offset,
                data,
                loadout: loadouts[id].clone(),
                eye: eyes[id].expect("No eye effect for mech"),
                paint_palettes: paint_palettes
                    .try_into()
                    .expect("Failed to convert paint_palettes into an array"),
            });
        }

        return Ok(mechs);
    }

    pub fn get_model_configs(&self) -> [ModelConfig; 10] {
        [
            self.modcfgs[0].chassis,
            self.modcfgs[0].hatch,
            self.modcfgs[1].chassis,
            self.modcfgs[1].hatch,
            self.modcfgs[2].chassis,
            self.modcfgs[2].hatch,
            self.modcfgs[3].chassis,
            self.modcfgs[3].hatch,
            self.manipulator,
            self.weapon_mount,
        ]
    }

    pub fn get_id(&self) -> u8 {
        self.data.id
    }

    pub fn export(
        &self,
        path: &Path,
        model_path_fn: FnIndexToPath,
        efg_idx_path_fn: FnIndexToPath,
        mwep_path_fn: FnIndexToPath,
        swep_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let mut mech_path = path.to_owned();

        mech_path.push(format!("Mech_{:02}.mech_scene", self.get_id()));
        {
            let file = File::create(&mech_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u8(self.get_id())?;

            // Write Chassis and Hatch Models
            for modcfg in self.modcfgs.iter() {
                let chassis_path = model_path_fn(modcfg.chassis.model as usize);
                let hatch_path = model_path_fn(modcfg.hatch.model as usize);

                write_godot_path(&chassis_path, &mut writer)?;
                write_godot_path(&hatch_path, &mut writer)?;
            }

            let emblem_model_path = self.modcfgs[0]
                .emblem_model
                .map(|idx| model_path_fn(idx as usize));

            // Write Emblem Model
            write_godot_option_path(emblem_model_path.as_deref(), &mut writer)?;

            // Write Manipulator Model
            let manipulator_path = model_path_fn(self.manipulator.model as usize);
            write_godot_path(&manipulator_path, &mut writer)?;

            // Write Weapon Mount Model
            let weapon_mount_path = model_path_fn(self.weapon_mount.model as usize);
            write_godot_path(&weapon_mount_path, &mut writer)?;

            // Write Eye Effect
            let eye_effect_path = efg_idx_path_fn(self.eye.idx as usize);
            write_godot_path(&eye_effect_path, &mut writer)?;
            writer.write_f32::<LittleEndian>(self.eye.position.x)?;
            writer.write_f32::<LittleEndian>(self.eye.position.y)?;
            writer.write_f32::<LittleEndian>(self.eye.position.z)?;

            // Physics Collider
            writer.write_f32::<LittleEndian>(self.collider_size.x)?;
            writer.write_f32::<LittleEndian>(self.collider_size.y)?;
            writer.write_f32::<LittleEndian>(self.collider_size.z)?;

            writer.write_f32::<LittleEndian>(self.collider_offset.x)?;
            writer.write_f32::<LittleEndian>(self.collider_offset.y)?;
            writer.write_f32::<LittleEndian>(self.collider_offset.z)?;
        }
        mech_path.pop();

        mech_path.push("config.mech");
        {
            let file = File::create(&mech_path)?;
            let mut writer = BufWriter::new(file);

            // Mech ID
            writer.write_u8(self.data.id)?;

            // Translation keys
            write_pascal_string(&format!("loc:{:04}", self.name_text), &mut writer)?;
            write_pascal_string(&format!("loc:{:04}", self.description_text), &mut writer)?;

            // Mech Info
            writer.write_u8(self.data.cockpit_type)?;
            writer.write_u8(self.data.get_generation())?;
            writer.write_u8(self.manufacturer)?;
            writer.write_u8(self.faction_flags)?;
            writer.write_u8(self.loadout.weight_type)?;
            writer.write_u8(self.loadout.class_type)?;
            writer.write_u8(self.loadout.profile_description)?;
            writer.write_u8(self.loadout.mounts)?;
            writer.write_u8(self.data.ticket_cost)?;

            // Engine Characteristics
            writer.write_f32::<LittleEndian>(self.data.rpm_min)?;
            writer.write_f32::<LittleEndian>(self.data.rpm_rate)?;
            for rt in self.data.engine_curve.iter() {
                rt.write(&mut writer)?;
            }

            self.data.override_multiplier.write(&mut writer)?;

            // Transmission Characteristics
            writer.write_f32::<LittleEndian>(self.data.tier_r)?;
            for g in self.data.gears {
                writer.write_f32::<LittleEndian>(g)?;
            }

            writer.write_f32::<LittleEndian>(self.data.gear_f)?;

            writer.write_f32::<LittleEndian>(self.data.wheel_torque)?;
            writer.write_f32::<LittleEndian>(self.data.wheel_start_speed)?;

            writer.write_f32::<LittleEndian>(self.data.internal_resistance)?;

            writer.write_u8(self.data.slope_gear_a)?;
            writer.write_u8(self.data.slope_gear_b)?;

            // Movement Characteristics
            writer.write_f32::<LittleEndian>(self.data.weight)?;
            writer.write_f32::<LittleEndian>(self.data.brake)?;

            writer.write_f32::<LittleEndian>(self.data.drag_coefficient)?;
            writer.write_f32::<LittleEndian>(self.data.drag_size.x)?;
            writer.write_f32::<LittleEndian>(self.data.drag_size.y)?;

            writer.write_f32::<LittleEndian>(self.data.turn_speed)?;
            writer.write_f32::<LittleEndian>(self.data.balancer)?;

            // Mech Health
            writer.write_u16::<LittleEndian>(self.data.health_torso)?;
            writer.write_u16::<LittleEndian>(self.data.health_leg_r)?;
            writer.write_u16::<LittleEndian>(self.data.health_leg_l)?;
            writer.write_u16::<LittleEndian>(self.data.health_opt_armor)?;

            // Mech Damage Resistances
            writer.write_f32::<LittleEndian>(self.data.resistance_front)?;
            writer.write_f32::<LittleEndian>(self.data.resistance_side)?;
            writer.write_f32::<LittleEndian>(self.data.resistance_rear)?;

            // Loadout
            writer.write_f32::<LittleEndian>(self.data.tank_capacity_main)?;
            writer.write_f32::<LittleEndian>(self.data.tank_capacity_sub)?;

            writer.write_u8(self.data.loadout_weight_max)?;
            writer.write_u8(self.data.loadout_weight_standard)?;

            writer.write_u8(self.loadout.mwep_ids.len() as u8)?;
            for &id in self.loadout.mwep_ids.iter() {
                let mwep_path = mwep_path_fn(id as usize);
                write_godot_path(&mwep_path, &mut writer)?;
            }

            for idx in self.loadout.mwep_presets {
                writer.write_u8(idx)?;
            }

            for fixed in self.loadout.mwep_fixeds {
                writer.write_u8(fixed as u8)?;
            }

            writer.write_u8(self.loadout.swep_ids.len() as u8)?;
            for &id in self.loadout.swep_ids.iter() {
                let swep_path = swep_path_fn(id as usize);
                write_godot_path(&swep_path, &mut writer)?;
            }

            for idx in self.loadout.swep_presets {
                writer.write_u8(idx)?;
            }

            for fixed in self.loadout.swep_fixeds {
                writer.write_u8(fixed as u8)?;
            }

            writer.write_u8(self.loadout.tank_count_max)?;
            writer.write_u8(self.loadout.tank_count_preset)?;
        }
        mech_path.pop();

        for (i, palette) in self.paint_palettes.iter().enumerate() {
            mech_path.push(format!("{}.palette", i));

            let file = File::create(&mech_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u32::<LittleEndian>(palette.len() as u32)?;
            for color in palette {
                for c in color.to_array() {
                    writer.write_f32::<LittleEndian>(c)?;
                }
                writer.write_f32::<LittleEndian>(1.0)?; // Alpha is always 1
            }

            mech_path.pop();
        }

        return Ok(());
    }
}
