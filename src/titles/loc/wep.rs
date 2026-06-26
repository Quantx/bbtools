use std::cmp;
use std::ffi::CString;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::u16;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};

use glam::Quat;
use glam::U8Vec4;
use glam::f32::Vec2;
use glam::f32::Vec3A as Vec3;
use glam::u8::U8Vec2;

use crate::bbtools::*;
use crate::titles::loc::GAME_FPS;
use eff::{SMOKE_TRAIL_SIZE, TRACER_TRAIL_SIZE, TrailEffect};
use obj::{ModelConfig, ModelConfigFlags};
use xbe::XBE;

#[derive(Clone, Copy)]
pub enum ProjectileCollider {
    Sphere {
        position: Vec3,

        radius: f32,
    },
    Capsule {
        position: Vec3,
        rotation: Quat,

        radius: f32,
        height: f32,
    },
    Box {
        position: Vec3,
        rotation: Quat,

        size: Vec3,
    },
}

impl ProjectileCollider {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Sphere { position, radius } => {
                writer.write_u8(0)?;

                writer.write_f32::<LittleEndian>(position.x)?;
                writer.write_f32::<LittleEndian>(position.y)?;
                writer.write_f32::<LittleEndian>(position.z)?;

                writer.write_f32::<LittleEndian>(*radius)?;
            }
            Self::Capsule {
                position,
                rotation,
                radius,
                height,
            } => {
                writer.write_u8(1)?;

                writer.write_f32::<LittleEndian>(position.x)?;
                writer.write_f32::<LittleEndian>(position.y)?;
                writer.write_f32::<LittleEndian>(position.z)?;

                writer.write_f32::<LittleEndian>(rotation.x)?;
                writer.write_f32::<LittleEndian>(rotation.y)?;
                writer.write_f32::<LittleEndian>(rotation.z)?;
                writer.write_f32::<LittleEndian>(rotation.w)?;

                writer.write_f32::<LittleEndian>(*radius)?;
                writer.write_f32::<LittleEndian>(*height)?;
            }
            Self::Box {
                position,
                rotation,
                size,
            } => {
                writer.write_u8(2)?;

                writer.write_f32::<LittleEndian>(position.x)?;
                writer.write_f32::<LittleEndian>(position.y)?;
                writer.write_f32::<LittleEndian>(position.z)?;

                writer.write_f32::<LittleEndian>(rotation.x)?;
                writer.write_f32::<LittleEndian>(rotation.y)?;
                writer.write_f32::<LittleEndian>(rotation.z)?;
                writer.write_f32::<LittleEndian>(rotation.w)?;

                writer.write_f32::<LittleEndian>(size.x)?;
                writer.write_f32::<LittleEndian>(size.y)?;
                writer.write_f32::<LittleEndian>(size.z)?;
            }
        }

        return Ok(());
    }
}

#[derive(Clone, Default)]
pub struct WeaponEffects {
    /*** Mech Effects ***/
    // Smoke effects spawned at the base of the mech
    pub mech: Option<u16>,

    /*** Weapon Effects ***/
    // Effect played while the weapon is charging before the shot is fired
    pub charging: Option<u16>,
    // Muzzle flash effects (all are played at once)
    pub firing: Vec<u16>,
    // Muzzle smoke (5 of these are spawned at 0.1 second intervals)
    pub smoke: Option<u16>,
    // A bullet casing that's spawned at the ejection port
    pub casing: Option<u16>,

    // A random light effect is selected from this list and shown at the offset
    pub lights: Vec<u8>,
    pub lights_offset: Vec3,

    /*** Projectile Effects ***/
    // A flare effect that follows the projectile as it flies
    pub flare: Option<u16>,
    // Stationary smoke effects that are spawned as the projectile flies through the air
    pub flying: Vec<u16>,
    // A small trail that follows the projectile as it flies
    pub thrust: Option<u16>,
    // Played when the shot hits a mech. Spawned from one of the following special bones [0-7, 10, 11, 13-16]
    pub mech_impact: Option<u16>,

    pub smoke_trail: Option<u16>,
    pub tracer_trail: Option<u16>,
}

const WEAPON_DATA_SIZE: usize = 84;
pub struct WeaponData {
    id: u8,
    initial_velocity: f32,
    boost_max: f32,
    boost_rate: f32,
    gravity_acceleration: f32,
    torso_turn_rate: f32,
    damage_range: f32,
    range_max: f32,
    range_min: f32,
    damage_max: u16,
    rapid_fire: u16,
    firing_interval: f32,
    reload_interval: f32,
    volley_count: u8,
    volley_interval: f32,
    impact_effect: u8,
    projectile_count: u16,
    magazine_count: u8,
    damage_min: u16,
    weight: u8,
    fire_probability: u8,
    fire_damage: u8,
    tracking: u8,
    volley_spread: f32,
    muzzle_offset: Vec2,
    recoil: f32,
    damage_type: u8,
    charge_delay: f32,
    pub category: u8,
    _flying_effect: u8,
    muzzle_count: U8Vec2,
    mech_recoil: bool,
}

impl WeaponData {
    pub fn import(buf: &[u8; WEAPON_DATA_SIZE], scale: f32, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let id = LittleEndian::read_u16(&buf[0..2]);
        assert!(id <= u8::MAX as u16);

        let life = LittleEndian::read_u16(&buf[2..4]);
        assert!(life == 1);

        // unknown0
        assert!(buf[48] == 0);

        // unknown1
        assert!(buf[60] == 0);

        let category = buf[80];

        let damage_max = LittleEndian::read_u16(&buf[36..38]);
        let damage_falloff = LittleEndian::read_u16(&buf[54..56]);
        let damage_min = cmp::max(damage_max as i32 - damage_falloff as i32, 0) as u16;

        let mut damage_range = LittleEndian::read_f32(&buf[24..28]);
        if category == 0 || category == 4 || category == 10 {
            // Artillery, Mortar, MLRS
            damage_range *= 0.5;
        }

        let volley_count = LittleEndian::read_u16(&buf[44..46]);
        assert!(volley_count <= u8::MAX as u16);

        let magazine_count = LittleEndian::read_u16(&buf[52..54]);
        assert!(magazine_count <= u8::MAX as u16);

        let damage_type = LittleEndian::read_u16(&buf[76..78]);
        assert!(damage_type <= u8::MAX as u16);

        let fire_damage = LittleEndian::read_u16(&buf[58..60]);
        assert!(fire_damage <= u8::MAX as u16);

        return WeaponData {
            id: id as u8,
            initial_velocity: LittleEndian::read_f32(&buf[4..8]),
            boost_max: LittleEndian::read_f32(&buf[8..12]),
            boost_rate: LittleEndian::read_f32(&buf[12..16]),
            gravity_acceleration: LittleEndian::read_f32(&buf[16..20]),
            torso_turn_rate: LittleEndian::read_f32(&buf[20..24]) * fps,
            damage_range,
            range_max: LittleEndian::read_f32(&buf[28..32]),
            range_min: LittleEndian::read_f32(&buf[32..36]),
            damage_max: damage_max,
            rapid_fire: cmp::max(LittleEndian::read_u16(&buf[38..40]), 1),
            firing_interval: LittleEndian::read_u16(&buf[40..42]) as f32 * spf,
            reload_interval: LittleEndian::read_u16(&buf[42..44]) as f32 * spf,
            volley_count: volley_count as u8,
            volley_interval: LittleEndian::read_u16(&buf[46..48]) as f32 * spf,
            impact_effect: buf[49],
            projectile_count: LittleEndian::read_u16(&buf[50..52]),
            magazine_count: magazine_count as u8,
            damage_min: damage_min,
            weight: buf[56],
            fire_probability: buf[57],
            fire_damage: fire_damage as u8,
            tracking: buf[61],
            volley_spread: (LittleEndian::read_u16(&buf[62..64]) as f32).to_radians(),
            muzzle_offset: Vec2::new(
                LittleEndian::read_f32(&buf[64..68]),
                LittleEndian::read_f32(&buf[68..72]),
            ) * -scale,
            recoil: LittleEndian::read_f32(&buf[72..76]),
            damage_type: damage_type as u8,
            charge_delay: LittleEndian::read_u16(&buf[78..80]) as f32 * spf,
            category,
            _flying_effect: buf[81],
            muzzle_count: U8Vec2::new(buf[82], buf[83]),
            mech_recoil: buf[81] == 20, // Flying type == Rail
        };
    }

    pub fn import_multiple(buf: &[u8], scale: f32, fps: f32) -> Vec<Self> {
        let weapon_count = LittleEndian::read_u32(&buf[0..4]) as usize;

        let (weapon_list, []) = buf[4..].as_chunks::<WEAPON_DATA_SIZE>() else {
            panic!("buf length not a multiple of WEAPON_DATA_SIZE");
        };

        let mut weapon_data_list: Vec<WeaponData> = Vec::with_capacity(weapon_count);
        for weapon_buf in weapon_list {
            weapon_data_list.push(WeaponData::import(weapon_buf, scale, fps));
        }

        return weapon_data_list;
    }

    pub fn get_thrust_effect(&self) -> Option<u16> {
        match self.category {
            3 => Some(560), // Missile
            8 => Some(561), // Cruise Missile
            _ => None,
        }
    }

    pub fn get_mech_impact_effect(&self) -> Option<u16> {
        match self.damage_type {
            // Proximity
            2 => match self.impact_effect {
                7 | 8 | 9 => Some(569), // Missile
                19 => Some(638),        // Marker
                23 => Some(627),        // Grenade
                _ => None,
            },
            // Artillery
            4 => {
                if self.impact_effect == 13 {
                    Some(618) // Napalm
                } else {
                    Some(545)
                }
            }
            7 => Some(545), // MLRS
            _ => None,
        }
    }
}

const SMOKE_TRAIL_COUNT: usize = 20;
const TRACER_TRAIL_COUNT: usize = 20;

pub struct WeaponFile {
    pub mweps: Vec<WeaponData>,
    pub sweps: Vec<WeaponData>,
    pub cweps: Vec<WeaponData>,
    pub smoke_trails: Vec<TrailEffect>,
    pub tracer_trails: Vec<TrailEffect>,
}

impl WeaponFile {
    pub fn import(path: &Path, scale: f32, fps: f32) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let file_count = reader.read_u32::<LittleEndian>()? as usize;
        assert!(file_count == 2);

        let mut file_sizes: Vec<u32> = vec![0; file_count];
        reader.read_u32_into::<LittleEndian>(&mut file_sizes)?;

        let mut weapons_buf: Vec<u8> = vec![0; file_sizes[0] as usize];
        reader.read_exact(&mut weapons_buf)?;

        let mut trails_buf: Vec<u8> = vec![0; file_sizes[1] as usize];
        reader.read_exact(&mut trails_buf)?;

        /*** Read Weapon Data ***/
        let weapon_type_count = LittleEndian::read_u32(&weapons_buf[0..4]) as usize;
        assert!(weapon_type_count == 3);

        let weapon_type_sizes_end = 4 + weapon_type_count * 4;
        let mut weapon_type_sizes: Vec<u32> = vec![0; weapon_type_count];
        LittleEndian::read_u32_into(
            &weapons_buf[4..weapon_type_sizes_end],
            &mut weapon_type_sizes,
        );

        let mwep_buf_end = weapon_type_sizes_end + weapon_type_sizes[0] as usize;
        let swep_buf_end = mwep_buf_end + weapon_type_sizes[1] as usize;
        let cwep_buf_end = swep_buf_end + weapon_type_sizes[2] as usize;

        let mwep_buf = &weapons_buf[weapon_type_sizes_end..mwep_buf_end];
        let swep_buf = &weapons_buf[mwep_buf_end..swep_buf_end];
        let cwep_buf = &weapons_buf[swep_buf_end..cwep_buf_end];

        let mut mweps = WeaponData::import_multiple(mwep_buf, scale, fps);
        let sweps = WeaponData::import_multiple(swep_buf, scale, fps);
        let cweps = WeaponData::import_multiple(cwep_buf, scale, fps);

        /* Weapon Fixups */
        mweps[0].muzzle_count = U8Vec2::ONE;
        mweps[0].muzzle_offset = Vec2::ZERO;

        mweps[3].muzzle_count = U8Vec2::ONE;
        mweps[3].muzzle_offset = Vec2::ZERO;

        mweps[19].muzzle_offset.y *= -1.0;

        mweps[20].muzzle_offset.y *= -1.0;

        /*** Read Trails Data ***/
        let trail_type_count = LittleEndian::read_u32(&trails_buf[0..4]) as usize;
        assert!(trail_type_count == 1);

        let trail_type_sizes_end = 4 + trail_type_count * 4;
        let mut trail_type_sizes: Vec<u32> = vec![0; trail_type_count];
        LittleEndian::read_u32_into(&trails_buf[4..trail_type_sizes_end], &mut trail_type_sizes);

        let smoke_buf_end = trail_type_sizes_end + (SMOKE_TRAIL_SIZE * SMOKE_TRAIL_COUNT);
        let tracer_buf_end = smoke_buf_end + (TRACER_TRAIL_SIZE * TRACER_TRAIL_COUNT);

        let smoke_buf = &trails_buf[trail_type_sizes_end..smoke_buf_end];
        let tracer_buf = &trails_buf[smoke_buf_end..tracer_buf_end];

        let smoke_trails = TrailEffect::import_smoke_multiple(smoke_buf, 15, scale, GAME_FPS);
        let mut tracer_trails =
            TrailEffect::import_tracer_multiple(tracer_buf, 33, scale, GAME_FPS);

        // Manually add the 19th entry which is used by MWEP 10
        tracer_trails.push(TrailEffect::new_tracer(
            33,
            U8Vec4::new(0xFF, 0xC0, 0x80, 0xFF),
            4,
            35.0,
            1.0 / GAME_FPS,
        ));

        return Ok(WeaponFile {
            mweps,
            sweps,
            cweps,
            smoke_trails,
            tracer_trails,
        });
    }
}

#[derive(Copy, Clone)]
pub enum WeaponType {
    MWEP = 0, // Main Weapons
    SWEP = 1, // Sub Weapons
    CWEP = 2, // Chaff Weapons
}

impl WeaponType {
    pub fn to_str(&self) -> &str {
        match self {
            Self::MWEP => "MWEP",
            Self::SWEP => "SWEP",
            Self::CWEP => "CWEP",
        }
    }
}

pub struct WeaponLookup(u32, u32);

pub struct Weapon {
    wep_type: WeaponType,
    pub data: WeaponData,
    pub effects: WeaponEffects,
    display_name: String,
    name_text: u16,
    description_text: u16,
    pub weapon_modcfg: ModelConfigFlags,
    projectile_modcfg: ModelConfig,
    projectile_collider: ProjectileCollider,
}

impl Weapon {
    pub fn read_weapon_names(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        xbe.seek_section_offset(section_name, offset)?;
        let mut pointers: Vec<u32> = vec![0; count];
        xbe.reader.read_u32_into::<LittleEndian>(&mut pointers)?;

        let mut strings: Vec<String> = Vec::with_capacity(count);
        for pointer in pointers {
            xbe.seek_pointer_offset(pointer)?;

            let mut cstr_buf: Vec<u8> = Vec::with_capacity(16);
            xbe.reader.read_until(0, &mut cstr_buf)?;

            let cstr = CString::from_vec_with_nul(cstr_buf)?;
            strings.push(cstr.into_string()?);
        }

        return Ok(strings);
    }

    pub fn read_weapon_lookups(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
    ) -> Result<Vec<WeaponLookup>, Box<dyn std::error::Error>> {
        xbe.seek_section_offset(section_name, offset)?;
        let mut pointers: Vec<u32> = vec![0; count];
        xbe.reader.read_u32_into::<LittleEndian>(&mut pointers)?;

        let mut lookups: Vec<WeaponLookup> = Vec::with_capacity(count);
        for pointer in pointers {
            lookups.push(if xbe.seek_pointer_offset(pointer).is_ok() {
                WeaponLookup(
                    xbe.reader.read_u32::<LittleEndian>()?,
                    xbe.reader.read_u32::<LittleEndian>()?,
                )
            } else {
                WeaponLookup(0, 0)
            });
        }

        return Ok(lookups);
    }

    pub fn import(
        xbe: &mut XBE,
        weapon_data: Vec<WeaponData>,
        weapon_type: WeaponType,
        weapon_effects: &[WeaponEffects],
        tracers: &[TrailEffect],
        projectile_colliders: &[ProjectileCollider],
    ) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let projectile_modcfgs = ModelConfig::import_pointers(xbe, ".data", 0x58A90, 15)?;

        let weapon_modcfgs = match weapon_type {
            WeaponType::MWEP => ModelConfigFlags::import_pointers_b(xbe, ".data", 0x58620, 26)?,
            WeaponType::SWEP | WeaponType::CWEP => {
                ModelConfigFlags::import_pointers_b(xbe, ".data", 0x58998, 32)?
            }
        };

        let lookups = match weapon_type {
            WeaponType::MWEP => Weapon::read_weapon_lookups(xbe, ".data", 0x58480, 26)?,
            WeaponType::SWEP | WeaponType::CWEP => {
                Weapon::read_weapon_lookups(xbe, ".data", 0x58790, 33)?
            }
        };

        let weapon_names = match weapon_type {
            WeaponType::MWEP => Weapon::read_weapon_names(xbe, ".data", 0x58AE0, 26)?,
            WeaponType::SWEP | WeaponType::CWEP => {
                Weapon::read_weapon_names(xbe, ".data", 0x58B48, 33)?
            }
        };

        assert!(weapon_data.len() == weapon_effects.len());

        let mut weapons: Vec<Weapon> = Vec::with_capacity(weapon_data.len());
        for data in weapon_data {
            let id = data.id as usize;

            let WeaponLookup(projectile_idx, weapon_idx) = lookups[id];

            let weapon_modcfg = weapon_modcfgs[weapon_idx as usize];
            let projectile_modcfg = projectile_modcfgs[projectile_idx as usize];

            let effects = weapon_effects[id].clone();

            let projectile_collider_idx = (projectile_modcfg.model as usize - 1186) / 2;

            let mut projectile_collider = projectile_colliders[projectile_collider_idx];
            if projectile_collider_idx == 2 // Weapon uses the invisible projectile model
                && weapon_modcfg.flags & 0x0108_0000 == 0 // Weapon is neither melee, nor sheild
                && (id != 21 && matches!(weapon_type, WeaponType::MWEP) // Weapon is not the spear
            ) {
                match projectile_collider {
                    ProjectileCollider::Sphere { ref mut radius, .. } => {
                        let tracer_idx = effects.tracer_trail.expect(&format!(
                            "Weapon {} used empty model but did not specify a tracer trail",
                            id,
                        ));

                        // Set the collider radius to match the projectile effect width
                        let tracer = &tracers[tracer_idx as usize];
                        *radius = tracer.width_start * 0.25;
                    }
                    _ => unimplemented!("Projectile Collider Idx 2 was not a sphere"),
                }
            }

            let (name_text, description_text) = match weapon_type {
                WeaponType::MWEP => (id as u16 + 1449, id as u16 + 739),
                WeaponType::SWEP | WeaponType::CWEP => (id as u16 + 1472, id as u16 + 762),
            };

            weapons.push(Weapon {
                wep_type: weapon_type.clone(),
                data,
                effects,
                weapon_modcfg,
                projectile_modcfg,
                projectile_collider,
                display_name: weapon_names[id].clone(),
                name_text,
                description_text,
            });
        }

        return Ok(weapons);
    }

    pub fn get_id(&self) -> u8 {
        self.data.id
    }

    pub fn get_model_configs(&self) -> [ModelConfig; 2] {
        [self.weapon_modcfg.modcfg, self.projectile_modcfg]
    }

    pub fn export(
        &self,
        path: &Path,
        model_path_fn: FnIndexToPath,
        efg_idx_path_fn: FnIndexToPath,
        effect_lighting_path_fn: FnIndexToPath,
        tracer_trail_path_fn: FnIndexToPath,
        smoke_trail_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let mut weapon_path = path.to_owned();

        weapon_path.push(format!(
            "{}_{:02}.weapon_scene",
            self.wep_type.to_str(),
            self.get_id()
        ));
        {
            let file = File::create(&weapon_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u8(self.data.id)?;
            writer.write_u8(self.wep_type as u8)?;

            writer.write_u32::<LittleEndian>(self.weapon_modcfg.flags)?;

            let weapon_path = model_path_fn(self.weapon_modcfg.modcfg.model as usize);
            write_godot_path(&weapon_path, &mut writer)?;

            let mech_path = self.effects.mech.map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(mech_path.as_deref(), &mut writer)?;

            let charging_path = self
                .effects
                .charging
                .map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(charging_path.as_deref(), &mut writer)?;

            writer.write_u32::<LittleEndian>(self.effects.firing.len() as u32)?;
            for &firing_effect in self.effects.firing.iter() {
                let firing_path_str = efg_idx_path_fn(firing_effect as usize);
                write_godot_path(&firing_path_str, &mut writer)?;
            }

            let smoke_path = self.effects.smoke.map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(smoke_path.as_deref(), &mut writer)?;

            let casing_path = self.effects.casing.map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(casing_path.as_deref(), &mut writer)?;

            for v in self.effects.lights_offset.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }

            writer.write_u32::<LittleEndian>(self.effects.lights.len() as u32)?;
            for &light_idx in self.effects.lights.iter() {
                let light_path = effect_lighting_path_fn(light_idx as usize);
                write_godot_path(&light_path, &mut writer)?;
            }
        }
        weapon_path.pop();

        weapon_path.push(format!(
            "{}_{:02}.projectile_scene",
            self.wep_type.to_str(),
            self.get_id()
        ));
        {
            let file = File::create(&weapon_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u8(self.data.id)?;
            writer.write_u8(self.wep_type as u8)?;

            let projectile_path = model_path_fn(self.projectile_modcfg.model as usize);
            write_godot_path(&projectile_path, &mut writer)?;

            self.projectile_collider.write(&mut writer)?;

            let flare_path = self.effects.flare.map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(flare_path.as_deref(), &mut writer)?;

            writer.write_u32::<LittleEndian>(self.effects.flying.len() as u32)?;
            for &flying_effect in self.effects.flying.iter() {
                let flying_path_str = efg_idx_path_fn(flying_effect as usize);
                write_godot_path(&flying_path_str, &mut writer)?;
            }

            let thrust_path = self.effects.thrust.map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(thrust_path.as_deref(), &mut writer)?;

            let mech_impact_path = self
                .effects
                .mech_impact
                .map(|idx| efg_idx_path_fn(idx as usize));
            write_godot_option_path(mech_impact_path.as_deref(), &mut writer)?;

            let smoke_trail_path = self
                .effects
                .smoke_trail
                .map(|idx| smoke_trail_path_fn(idx as usize));
            write_godot_option_path(smoke_trail_path.as_deref(), &mut writer)?;

            let tracer_trail_path = self
                .effects
                .tracer_trail
                .map(|idx| tracer_trail_path_fn(idx as usize));
            write_godot_option_path(tracer_trail_path.as_deref(), &mut writer)?;
        }
        weapon_path.pop();

        weapon_path.push(format!("{:02}.weapon", self.get_id()));
        {
            let file = File::create(&weapon_path)?;
            let mut writer = BufWriter::new(file);

            // Weapon ID
            writer.write_u8(self.data.id)?;
            writer.write_u8(self.wep_type as u8)?;

            // Translation keys
            write_pascal_string(&format!("loc:{:04}", self.name_text), &mut writer)?;
            write_pascal_string(&format!("loc:{:04}", self.description_text), &mut writer)?;

            // Categories
            writer.write_u8(self.data.category)?;
            writer.write_u8(self.data.damage_type)?;
            writer.write_u8(self.data.tracking)?;
            writer.write_u8(self.data.impact_effect)?;
            //writer.write_u8(self.data.flying_effect)?; // Unused

            // Weapon Cockpit Name
            write_pascal_string(&self.display_name, &mut writer)?;

            // Mech Data
            writer.write_f32::<LittleEndian>(self.data.torso_turn_rate)?;
            writer.write_u8(self.data.weight)?;

            // Muzzle Data
            writer.write_u8(self.data.muzzle_count.x)?;
            writer.write_u8(self.data.muzzle_count.y)?;
            writer.write_f32::<LittleEndian>(self.data.muzzle_offset.x)?;
            writer.write_f32::<LittleEndian>(self.data.muzzle_offset.y)?;

            // Weapon Behaviour
            writer.write_f32::<LittleEndian>(self.data.firing_interval)?;
            writer.write_f32::<LittleEndian>(self.data.reload_interval)?;

            writer.write_u16::<LittleEndian>(self.data.rapid_fire)?;

            writer.write_u16::<LittleEndian>(self.data.projectile_count)?;
            writer.write_u8(self.data.magazine_count)?;

            writer.write_u8(self.data.volley_count)?;
            writer.write_f32::<LittleEndian>(self.data.volley_interval)?;
            writer.write_f32::<LittleEndian>(self.data.volley_spread)?;

            writer.write_f32::<LittleEndian>(self.data.recoil)?;
            writer.write_u8(self.data.mech_recoil as u8)?;
            writer.write_f32::<LittleEndian>(self.data.charge_delay)?;

            // Projectile Physics
            writer.write_f32::<LittleEndian>(self.data.initial_velocity)?;
            writer.write_f32::<LittleEndian>(self.data.boost_max)?;
            writer.write_f32::<LittleEndian>(self.data.boost_rate)?;
            writer.write_f32::<LittleEndian>(self.data.gravity_acceleration)?;

            // Projectile Data
            writer.write_f32::<LittleEndian>(self.data.range_min)?;
            writer.write_f32::<LittleEndian>(self.data.range_max)?;

            writer.write_f32::<LittleEndian>(self.data.damage_range)?;
            writer.write_u16::<LittleEndian>(self.data.damage_min)?;
            writer.write_u16::<LittleEndian>(self.data.damage_max)?;

            writer.write_u8(self.data.fire_probability)?;
            writer.write_u8(self.data.fire_damage)?;
        }
        weapon_path.pop();

        return Ok(());
    }
}
