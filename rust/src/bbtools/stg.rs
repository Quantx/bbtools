use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use glam::USizeVec2;
use glam::f32::{Vec2, Vec3A as Vec3};
use std::f32;
use std::{
    fs::File,
    io::{BufReader, BufWriter, Read},
    path::Path,
};

use crate::bbtools::write_godot_path;
use crate::bbtools::{obj::OBJ, ppd::Surface, xpr::XPR};
use crate::titles::FnIndexToPath;

const STG_COUNT: usize = 4;
const STG_SIZE: usize = 264;

#[derive(Debug)]
pub struct STG {
    world_light: [f32; 4],
    world_specular: [f32; 4],
    world_ambient: [f32; 4],
    fog_color: [f32; 4],

    sun_color: [f32; 4],

    water_color: [f32; 4],

    sky_height: f32,
    sky_velocity: [Vec2; 2],

    water_height: f32,
    _water_speed: f32,

    tactics_time: u32,

    _do_predraw: bool,

    draw_rain: bool,
    draw_shadows: bool,
    draw_terrain: bool,
    draw_water: bool,

    _point_light_rate: f32,

    fog_start: f32,
    fog_end: f32,

    sky_fog_start: f32,
    sky_fog_end: f32,

    _shadow_offset: f32,
    _shadow_scale: f32,
    _shadow_angle: f32,
    shadow_start: f32,
    shadow_end: f32,
    shadow_yaw: f32,
    shadow_pitch: f32,

    sun_flash_power: f32,
    sun_back_size: f32,
    sun_front_size: f32,

    _water_texture: u8,

    _flash_shadow_rate: f32,

    ticket_a: u32,
    ticket_b: u32,

    _clip_ex: f32,
    _clip_sub_ex: f32,
    _clip_side_ex: f32,
}

impl STG {
    pub fn import_buf(buf: &[u8], scale: f32, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let mut world_light: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[0..16], &mut world_light);

        let mut world_specular: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[16..32], &mut world_specular);

        let mut world_ambient: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[32..48], &mut world_ambient);

        let mut fog_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[48..64], &mut fog_color);

        let mut sun_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[64..80], &mut sun_color);

        // LittleEndian::read_u32(&buf[80..84]) // shadow draw mode

        let sky_height = LittleEndian::read_f32(&buf[84..88]) * scale;
        let sky_velocity = [
            Vec2::new(
                LittleEndian::read_f32(&buf[88..92]),
                LittleEndian::read_f32(&buf[92..96]),
            ),
            Vec2::new(
                LittleEndian::read_f32(&buf[96..100]),
                LittleEndian::read_f32(&buf[100..104]),
            ),
        ];

        let water_height = LittleEndian::read_f32(&buf[104..108]) * scale;
        let water_speed = LittleEndian::read_f32(&buf[108..112]) * spf;

        let draw_rain = LittleEndian::read_u32(&buf[112..116]) != 0;

        let tactics_time = LittleEndian::read_u32(&buf[116..120]);

        let do_predraw = LittleEndian::read_u32(&buf[120..124]) != 0;
        let draw_shadows = LittleEndian::read_u32(&buf[124..128]) != 0;
        let draw_sky = LittleEndian::read_u32(&buf[128..132]) != 0;
        let draw_terrain = LittleEndian::read_u32(&buf[132..136]) != 0;
        let draw_water = LittleEndian::read_u32(&buf[136..140]) != 0;

        assert!(draw_sky == draw_terrain);

        let point_light_rate = LittleEndian::read_f32(&buf[140..148]);

        //LittleEndian::read_f32(&buf[144..148]); // fog_color_argb

        let fog_start = LittleEndian::read_f32(&buf[148..152]) * scale;
        let fog_end = LittleEndian::read_f32(&buf[152..156]) * scale;

        let sky_fog_start = LittleEndian::read_f32(&buf[156..160]) * scale;
        let sky_fog_end = LittleEndian::read_f32(&buf[160..164]) * scale;

        let shadow_offset = LittleEndian::read_f32(&buf[164..168]);
        let shadow_scale = LittleEndian::read_f32(&buf[168..172]);
        let shadow_angle = LittleEndian::read_f32(&buf[172..176]);
        let shadow_start = LittleEndian::read_f32(&buf[176..180]);
        let shadow_end = LittleEndian::read_f32(&buf[180..184]);
        let shadow_yaw = LittleEndian::read_f32(&buf[184..188]);
        let shadow_pitch = LittleEndian::read_f32(&buf[188..192]);

        let sun_flash_power = LittleEndian::read_f32(&buf[192..196]);
        let sun_back_size = LittleEndian::read_f32(&buf[196..200]) * scale;
        let sun_front_size = LittleEndian::read_f32(&buf[200..204]) * scale;

        let water_texture = LittleEndian::read_u32(&buf[204..208]);
        assert!(water_texture <= u8::MAX as u32);

        let flash_shadow_rate = LittleEndian::read_f32(&buf[208..212]);

        let area_over_size = LittleEndian::read_u32(&buf[212..216]);
        assert!(area_over_size == 0);

        let ticket_a = LittleEndian::read_u32(&buf[216..220]);
        let ticket_b = LittleEndian::read_u32(&buf[220..224]);

        let clip_ex = LittleEndian::read_f32(&buf[224..228]);
        let clip_sub_ex = LittleEndian::read_f32(&buf[228..232]);
        let clip_side_ex = LittleEndian::read_f32(&buf[232..236]);

        //LittleEndian::read_f32(&buf[236..240]);
        //LittleEndian::read_u32(&buf[240..244]);
        //LittleEndian::read_u32(&buf[244..248]);

        let mut water_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[248..264], &mut water_color);

        return STG {
            world_light,
            world_specular,
            world_ambient,
            fog_color,
            sun_color,
            sky_height,
            sky_velocity,
            water_height,
            _water_speed: water_speed,
            draw_rain,

            tactics_time,
            _do_predraw: do_predraw,
            draw_shadows,
            draw_terrain,
            draw_water,

            _point_light_rate: point_light_rate,

            fog_start,
            fog_end,

            sky_fog_start,
            sky_fog_end,

            _shadow_offset: shadow_offset,
            _shadow_scale: shadow_scale,
            _shadow_angle: shadow_angle,
            shadow_start,
            shadow_end,
            shadow_yaw,
            shadow_pitch,

            sun_flash_power,
            sun_back_size,
            sun_front_size,

            _water_texture: water_texture as u8,

            _flash_shadow_rate: flash_shadow_rate,

            ticket_a,
            ticket_b,

            _clip_ex: clip_ex,
            _clip_sub_ex: clip_sub_ex,
            _clip_side_ex: clip_side_ex,

            water_color,
        };
    }

    pub fn import(path: &Path, scale: f32, fps: f32) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut buf: Vec<u8> = Vec::with_capacity(STG_SIZE);
        reader.read_to_end(&mut buf)?;

        if buf.len() != STG_SIZE {
            return Err(std::io::Error::other("STG file was incorrect size"));
        }

        return Ok(Self::import_buf(buf.as_slice(), scale, fps));
    }

    pub fn export(&self, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u8(self.draw_shadows as u8)?;
        writer.write_u8(self.draw_rain as u8)?;

        writer.write_u32::<LittleEndian>(self.tactics_time)?;
        writer.write_u32::<LittleEndian>(self.ticket_a)?;
        writer.write_u32::<LittleEndian>(self.ticket_b)?;

        /*** Write World Lighting Data ***/
        for c in self.world_light {
            writer.write_f32::<LittleEndian>(c)?;
        }

        for c in self.world_specular {
            writer.write_f32::<LittleEndian>(c)?;
        }

        for c in self.world_ambient {
            writer.write_f32::<LittleEndian>(c)?;
        }

        for c in self.fog_color {
            writer.write_f32::<LittleEndian>(c)?;
        }

        /*** Write Fog data ***/
        writer.write_f32::<LittleEndian>(self.fog_start)?;
        writer.write_f32::<LittleEndian>(self.fog_end)?;

        writer.write_f32::<LittleEndian>(self.sky_fog_start)?;
        writer.write_f32::<LittleEndian>(self.sky_fog_end)?;

        /*** Write Sun and Shadow data ***/
        for c in self.sun_color {
            writer.write_f32::<LittleEndian>(c)?;
        }

        writer.write_f32::<LittleEndian>(self.sun_flash_power)?;
        writer.write_f32::<LittleEndian>(self.sun_back_size)?;
        writer.write_f32::<LittleEndian>(self.sun_front_size)?;

        writer.write_f32::<LittleEndian>(self.shadow_start)?;
        writer.write_f32::<LittleEndian>(self.shadow_end)?;
        writer.write_f32::<LittleEndian>(self.shadow_yaw)?;
        writer.write_f32::<LittleEndian>(self.shadow_pitch)?;

        /*** Write Sky data ***/
        writer.write_f32::<LittleEndian>(self.sky_height)?;
        for scroll in self.sky_velocity {
            writer.write_f32::<LittleEndian>(scroll.x)?;
            writer.write_f32::<LittleEndian>(scroll.y)?;
        }

        /*** Write Water data ***/
        for c in self.water_color {
            writer.write_f32::<LittleEndian>(c)?;
        }

        /*** Unused variables ***
        water_texture: u8,

        do_predraw: bool,

        point_light_rate: f32,

        shadow_offset: f32,
        shadow_scale: f32,
        shadow_angle: f32,

        flash_shadow_rate: f32,

        clip_ex: f32,
        clip_sub_ex: f32,
        clip_side_ex: f32,

        water_speed: f32,
        */

        return Ok(());
    }
}

pub struct GND {
    size: USizeVec2,
    heightmap: Vec<f32>,
    texture: XPR,
}

impl GND {
    pub fn import(
        path: &Path,
        size: USizeVec2,
        terrain_scale: f32,
    ) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let length = size.x * size.y;

        let mut heightmap: Vec<f32> = vec![0.0; length];
        reader.read_f32_into::<LittleEndian>(&mut heightmap)?;

        // Scale the heightmap
        heightmap.iter_mut().for_each(|h| *h *= terrain_scale);

        // Skip unused data
        reader.seek_relative(length as i64 * 4)?;

        let mut texture_data: Vec<u8> = vec![0; length * 4];
        reader.read_exact(&mut texture_data)?;

        let texture = XPR::new_argb_lin(size.y, size.x, texture_data);

        return Ok(GND {
            size,
            heightmap,
            texture,
        });
    }

    pub fn get_min_height(&self) -> f32 {
        self.heightmap.iter().cloned().fold(0.0 / 0.0, f32::min)
    }

    pub fn get_max_height(&self) -> f32 {
        self.heightmap.iter().cloned().fold(0.0 / 0.0, f32::max)
    }

    pub fn get_height(&self, x: usize, y: usize) -> f32 {
        return self.heightmap[y * self.size.x + x];
    }
}

pub struct GAD {
    size: USizeVec2,
    attributes: Vec<Surface>,
}

impl GAD {
    pub fn import(path: &Path) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let size = USizeVec2::new(
            reader.read_u16::<LittleEndian>()? as usize,
            reader.read_u16::<LittleEndian>()? as usize,
        );

        assert!(size == USizeVec2::new(280, 280));

        let length = size.x * size.y;

        let mut attributes: Vec<Surface> = Vec::with_capacity(length);
        for _ in 0..length {
            let surface_bit = reader.read_u48::<LittleEndian>()?;
            let layers = reader.read_u16::<LittleEndian>()?;

            attributes.push(Surface::new(layers, surface_bit));
        }

        return Ok(GAD { size, attributes });
    }
}

pub struct Mission {
    scale: f32,
    ground: GAD,
    terrain: Option<GND>,
    stages: [STG; STG_COUNT],
}

impl Mission {
    pub fn import(
        gad_path: &Path,
        gnd_path: &Path,
        stg_paths: [&Path; STG_COUNT],
        terrain_scale: f32,
        object_scale: f32,
        fps: f32,
    ) -> Result<Self, std::io::Error> {
        let ground = GAD::import(gad_path)?;

        let map_scale = object_scale / terrain_scale; // 50 == (0.01 / 0.0002) == (5000 / 100)

        let terrain = if gnd_path.is_file() {
            let gnd = GND::import(&gnd_path, ground.size, terrain_scale)?;
            println!(
                "Terrain height min: {}, max: {}",
                gnd.get_min_height() * map_scale,
                gnd.get_max_height() * map_scale,
            );
            Some(gnd)
        } else {
            None
        };

        let stages: [STG; 4] = {
            let mut stage_list: Vec<STG> = Vec::with_capacity(STG_COUNT);
            for p in stg_paths {
                let stg = STG::import(p, object_scale, fps)?;
                if let Some(fstg) = stage_list.get(0) {
                    // Check that draw flags are always consistent
                    assert!(fstg.draw_water == stg.draw_water);
                    assert!(fstg.draw_terrain == stg.draw_terrain);

                    if stg.draw_water {
                        assert!(fstg.water_height == stg.water_height);
                    }
                } else if stg.draw_water {
                    println!("Water height: {}", stg.water_height);
                }

                if stg.draw_terrain {
                    println!("Sky height: {}", stg.sky_height);
                }

                println!(
                    "Sun front: {}, back: {}, flash: {}",
                    stg.sun_front_size, stg.sun_back_size, stg.sun_flash_power
                );

                println!(
                    "Sky height: {:.03}, Velocity 0: {:.03}, Velocity 1: {:.03}",
                    stg.sky_height, stg.sky_velocity[0], stg.sky_velocity[1],
                );

                stage_list.push(stg);
            }

            assert!(stage_list.len() == STG_COUNT);

            stage_list
                .try_into()
                .expect("Failed to cast stage_list into array")
        };

        return Ok(Mission {
            scale: map_scale,
            ground,
            terrain,
            stages,
        });
    }

    // https://gamedev.stackexchange.com/questions/24572/how-does-terrain-following-work-on-height-map
    pub fn sample_height(&self, mut pos: Vec3) -> Option<f32> {
        if self.terrain.is_none() {
            return None;
        }

        if pos.x < 0.0 || pos.z < 0.0 {
            return None;
        }

        pos /= self.scale;

        let p0 = USizeVec2::new(pos.x as usize, pos.z as usize);
        let p1 = p0 + USizeVec2::ONE;

        let terrain = self.terrain.as_ref().unwrap();
        let h0 = terrain.get_height(p0.x, p0.y);
        let h1 = terrain.get_height(p1.x, p0.y);
        let h2 = terrain.get_height(p0.x, p1.y);
        let h3 = terrain.get_height(p1.x, p1.y);

        pos -= pos.floor();

        let h = if pos.x + pos.z < 1.0 {
            let hx = (h1 - h0) * pos.x;
            let hz = (h2 - h0) * pos.z;
            h0 + hx + hz
        } else {
            let hx = (h2 - h3) * (1.0 - pos.x);
            let hz = (h1 - h3) * (1.0 - pos.z);
            h3 + hx + hz
        };

        return Some(h * self.scale);
    }

    pub fn export(
        &self,
        id: usize,
        path: &Path,
        objects: &[OBJ],
        model_path_fn: FnIndexToPath,
        dds_paths: [&Path; 6],
    ) -> Result<(), std::io::Error> {
        let mut mission_path = path.to_path_buf();

        let fstg = &self.stages[0];

        mission_path.push(format!("Mission_{:02}.mission_scene", id));
        {
            let file = File::create(&mission_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u8(fstg.draw_terrain as u8)?;
            write_godot_path(&dds_paths[2], &mut writer)?; // Map Big DDS
            write_godot_path(&dds_paths[3], &mut writer)?; // Map Small DDS

            write_godot_path(&dds_paths[0], &mut writer)?; // Object DDS
            let offset = Vec3::new(
                self.ground.size.x as f32 - 1.0,
                0.0,
                self.ground.size.y as f32 - 1.0,
            ) * self.scale
                * -0.5;

            OBJ::write_objects(objects, offset, model_path_fn, &mut writer)?;
        }
        mission_path.pop();

        if let Some(terrain) = &self.terrain {
            mission_path.push("terrain.dds");
            terrain.texture.write_to_dds(&mission_path)?;

            mission_path.set_extension("ground");
            let file = File::create(&mission_path)?;
            let mut writer = BufWriter::new(file);

            write_godot_path(&dds_paths[4], &mut writer)?; // Sky 0
            write_godot_path(&dds_paths[5], &mut writer)?; // Sky 1

            write_godot_path(&dds_paths[1], &mut writer)?; // Terrain DDS

            writer.write_u32::<LittleEndian>(self.ground.size.x as u32)?;
            writer.write_u32::<LittleEndian>(self.ground.size.y as u32)?;
            writer.write_f32::<LittleEndian>(self.scale)?;

            writer.write_f32::<LittleEndian>(if fstg.draw_water {
                fstg.water_height
            } else {
                f32::NEG_INFINITY
            })?;

            for &v in terrain.heightmap.iter() {
                writer.write_f32::<LittleEndian>(v)?;
            }

            for &Surface(layers, _) in self.ground.attributes.iter() {
                writer.write_u16::<LittleEndian>(layers)?;
            }

            for &Surface(_, surface_type) in self.ground.attributes.iter() {
                writer.write_u8(surface_type)?;
            }

            mission_path.pop();
        }

        for (i, stg) in self.stages.iter().enumerate() {
            mission_path.push(format!("tod{}.stage", i));
            stg.export(&mission_path)?;
            mission_path.pop();
        }

        return Ok(());
    }
}
