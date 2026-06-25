use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use glam::f32::{Vec2, Vec3A as Vec3};
use glam::{U8Vec4, USizeVec2, Vec4Swizzles};
use std::cmp;
use std::f32;
use std::fs;
use std::path::PathBuf;
use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::bbtools::*;
use obj::OBJ;
use ppd::Surface;
use xpr::XPR;

const SB_STG_SIZE: usize = 316;
const LOC_STG_SIZE: usize = 264;
const LOC_STG_COUNT: usize = 4;

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

    tactics_time: u32,

    draw_rain: bool,
    draw_shadows: bool,
    draw_terrain: bool,
    draw_water: bool,
    draw_sun: bool,

    fog_start: f32,
    fog_end: f32,

    sky_fog_start: f32,
    sky_fog_end: f32,

    shadow_start: f32,
    shadow_end: f32,
    shadow_yaw: f32,
    shadow_pitch: f32,

    sun_flash_power: f32,
    sun_back_size: f32,
    sun_front_size: f32,

    ticket_a: u32,
    ticket_b: u32,

    terrain_scale: f32,

    start_pos: Vec3,
    start_yaw: f32,

    wave_speed: f32,
    wave_scale: f32,
}

impl STG {
    pub fn import_sb_buf(buf: &[u8], scale: f32, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let mut world_light: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[0..16], &mut world_light);

        let mut world_ambient: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[16..32], &mut world_ambient);

        let mut _cockpit_light: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[32..48], &mut _cockpit_light);

        let mut _cockpit_ambient: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[48..64], &mut _cockpit_ambient);

        let mut _cockpit_ambient_a: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[64..80], &mut _cockpit_ambient_a);

        let fog_color = U8Vec4::from_slice(&buf[80..84]).zyxw().as_vec4() / u8::MAX as f32;

        let fog_start = LittleEndian::read_f32(&buf[84..88]) * scale;
        let fog_end = LittleEndian::read_f32(&buf[88..92]) * scale;

        let _fog_start_sub = LittleEndian::read_f32(&buf[92..96]) * scale;
        let _fog_end_sub = LittleEndian::read_f32(&buf[96..100]) * scale;

        let shadow_yaw = LittleEndian::read_f32(&buf[100..104]);
        let shadow_pitch = LittleEndian::read_f32(&buf[104..108]);

        let _shadow_offset = LittleEndian::read_f32(&buf[108..112]);
        let _shadow_scale = LittleEndian::read_f32(&buf[112..116]);
        let _shadow_distance = LittleEndian::read_f32(&buf[116..120]);
        let shadow_start = LittleEndian::read_f32(&buf[120..124]) * scale;
        let shadow_end = LittleEndian::read_f32(&buf[124..128]) * scale;

        let _mip_bias = LittleEndian::read_f32(&buf[128..132]);

        let wave_speed = LittleEndian::read_f32(&buf[132..136]) * spf;
        let wave_scale = LittleEndian::read_f32(&buf[136..140]) * scale;

        let _noise_scroll = LittleEndian::read_f32(&buf[140..144]);

        let mut noise_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[144..160], &mut noise_color);

        let _noise_size = LittleEndian::read_f32(&buf[160..164]);

        let mut rain_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[164..180], &mut rain_color);

        let _rain_life = LittleEndian::read_u32(&buf[180..184]);

        let _rain_speed = LittleEndian::read_f32(&buf[184..188]);

        let _rain_req = LittleEndian::read_u32(&buf[188..192]);

        let _rain_spot_life = LittleEndian::read_u32(&buf[192..196]);

        let _rain_spot_size = LittleEndian::read_f32(&buf[196..200]);
        let _rain_spot_zoom = LittleEndian::read_f32(&buf[200..204]);

        let draw_rain = LittleEndian::read_u32(&buf[204..208]) != 0;
        let draw_sun = LittleEndian::read_u32(&buf[208..212]) != 0;

        let sun_flash_power = LittleEndian::read_f32(&buf[212..216]);
        let _sun_flash_ambient = LittleEndian::read_f32(&buf[216..220]);

        let start_pos = Vec3::new(
            LittleEndian::read_f32(&buf[220..224]),
            LittleEndian::read_f32(&buf[224..228]),
            LittleEndian::read_f32(&buf[228..232]),
        ) * scale;

        let start_yaw = LittleEndian::read_f32(&buf[232..236]);

        let terrain_scale = LittleEndian::read_f32(&buf[240..244]);

        let _noise_enabled = LittleEndian::read_u16(&buf[250..252]) != 0;
        let _sky_enabled = LittleEndian::read_u16(&buf[252..254]) != 0;

        let _clip = LittleEndian::read_f32(&buf[276..280]);
        let _clip0 = LittleEndian::read_f32(&buf[280..284]);
        let _clip0 = LittleEndian::read_f32(&buf[284..288]);

        let mut _bg_light_color: [f32; 4] = [0.0; _];
        LittleEndian::read_f32_into(&buf[288..304], &mut _bg_light_color);

        let _light_offset_y = LittleEndian::read_f32(&buf[304..308]);
        let _light_scroll_1 = LittleEndian::read_f32(&buf[308..312]);
        let _light_scroll_2 = LittleEndian::read_f32(&buf[312..316]);

        return STG {
            world_light,
            world_specular: [0.0; _],
            world_ambient,
            fog_color: fog_color.to_array(),

            sun_color: [0.5; _],

            water_color: [0.5; _],

            sky_height: 0.0,
            sky_velocity: [Vec2::ZERO; 2],

            water_height: 0.0,

            tactics_time: 0,

            draw_rain,
            draw_shadows: true,
            draw_terrain: true,
            draw_water: true,
            draw_sun,

            fog_start,
            fog_end,

            sky_fog_start: 0.0,
            sky_fog_end: 0.0,

            shadow_start,
            shadow_end,
            shadow_yaw,
            shadow_pitch,

            sun_flash_power,
            sun_back_size: 0.0,
            sun_front_size: 0.0,

            ticket_a: 0,
            ticket_b: 0,

            terrain_scale,

            start_pos,
            start_yaw,

            wave_scale,
            wave_speed,
        };
    }

    pub fn import_loc_buf(buf: &[u8], scale: f32, fps: f32) -> Self {
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

        let water_height = LittleEndian::read_f32(&buf[104..108]); // * scale;
        let _water_speed = LittleEndian::read_f32(&buf[108..112]) * spf;

        let draw_rain = LittleEndian::read_u32(&buf[112..116]) != 0;

        let tactics_time = LittleEndian::read_u32(&buf[116..120]);

        let _do_predraw = LittleEndian::read_u32(&buf[120..124]) != 0;
        let draw_shadows = LittleEndian::read_u32(&buf[124..128]) != 0;
        let draw_sky = LittleEndian::read_u32(&buf[128..132]) != 0;
        let draw_terrain = LittleEndian::read_u32(&buf[132..136]) != 0;
        let draw_water = LittleEndian::read_u32(&buf[136..140]) != 0;

        assert!(draw_sky == draw_terrain);

        let _point_light_rate = LittleEndian::read_f32(&buf[140..148]);

        //LittleEndian::read_f32(&buf[144..148]); // fog_color_argb

        let fog_start = LittleEndian::read_f32(&buf[148..152]) * scale;
        let fog_end = LittleEndian::read_f32(&buf[152..156]) * scale;

        let sky_fog_start = LittleEndian::read_f32(&buf[156..160]) * scale;
        let sky_fog_end = LittleEndian::read_f32(&buf[160..164]) * scale;

        let _shadow_offset = LittleEndian::read_f32(&buf[164..168]);
        let _shadow_scale = LittleEndian::read_f32(&buf[168..172]);
        let _shadow_angle = LittleEndian::read_f32(&buf[172..176]);
        let shadow_start = LittleEndian::read_f32(&buf[176..180]) * scale;
        let shadow_end = LittleEndian::read_f32(&buf[180..184]) * scale;
        let shadow_yaw = LittleEndian::read_f32(&buf[184..188]);
        let shadow_pitch = LittleEndian::read_f32(&buf[188..192]);

        let sun_flash_power = LittleEndian::read_f32(&buf[192..196]);
        let sun_back_size = LittleEndian::read_f32(&buf[196..200]) * scale;
        let sun_front_size = LittleEndian::read_f32(&buf[200..204]) * scale;

        let water_texture = LittleEndian::read_u32(&buf[204..208]);
        assert!(water_texture <= u8::MAX as u32);

        let _flash_shadow_rate = LittleEndian::read_f32(&buf[208..212]);

        let area_over_size = LittleEndian::read_u32(&buf[212..216]);
        assert!(area_over_size == 0);

        let ticket_a = LittleEndian::read_u32(&buf[216..220]);
        let ticket_b = LittleEndian::read_u32(&buf[220..224]);

        let _clip_ex = LittleEndian::read_f32(&buf[224..228]);
        let _clip_sub_ex = LittleEndian::read_f32(&buf[228..232]);
        let _clip_side_ex = LittleEndian::read_f32(&buf[232..236]);

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
            water_height,

            sky_height,
            sky_velocity,

            draw_rain,

            tactics_time,

            draw_shadows,
            draw_terrain,
            draw_water,
            draw_sun: true,

            fog_start,
            fog_end,

            sky_fog_start,
            sky_fog_end,

            shadow_start,
            shadow_end,
            shadow_yaw,
            shadow_pitch,

            sun_flash_power,
            sun_back_size,
            sun_front_size,

            ticket_a,
            ticket_b,

            water_color,

            terrain_scale: 0.0,

            start_pos: Vec3::NAN,
            start_yaw: f32::NAN,

            wave_speed: 0.0,
            wave_scale: 0.0,
        };
    }

    pub fn import(path: &Path, scale: f32, fps: f32) -> Result<Self, std::io::Error> {
        let buf = fs::read(path)?;
        match buf.len() {
            SB_STG_SIZE => Ok(Self::import_sb_buf(buf.as_slice(), scale, fps)),
            LOC_STG_SIZE => Ok(Self::import_loc_buf(buf.as_slice(), scale, fps)),
            _ => Err(std::io::Error::other("STG file was incorrect size")),
        }
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
        for v in self.sky_velocity {
            writer.write_f32::<LittleEndian>(v.x)?;
            writer.write_f32::<LittleEndian>(v.y)?;
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

const SKYBOX_DUAL_FLAT_COUNT: usize = 2;
enum Skybox {
    DualFlat {
        texture_paths: [PathBuf; SKYBOX_DUAL_FLAT_COUNT],
    },
    Spherical {
        texture_path: PathBuf,
    },
}

impl Skybox {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::DualFlat { texture_paths } => {
                writer.write_u8(0)?;
                for p in texture_paths {
                    write_godot_path(p, writer)?;
                }
            }
            Self::Spherical { texture_path } => {
                writer.write_u8(1)?;
                write_godot_path(&texture_path, writer)?;
            }
        }
        return Ok(());
    }
}

enum GNDTexture {
    Packed(XPR),
    External(PathBuf),
}

const BMP_MAGIC: [u8; 2] = *b"BM";
pub struct GND {
    gad: GAD,
    ground_heightmap: Vec<f32>,
    water_heightmap: Vec<f32>,
    skybox: Skybox,
    texture: GNDTexture,
    tilemap: Option<XPR>,
    tilemap_size: USizeVec2,
    tilemap_random_rotation: bool,
}

impl GND {
    fn import_gnd(
        gad: GAD,
        path: &Path,
        sky_paths: [&Path; SKYBOX_DUAL_FLAT_COUNT],
        terrain_scale: f32,
    ) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let size = gad.size;
        let length = size.element_product();

        let mut ground_heightmap: Vec<f32> = vec![0.0; length];
        reader.read_f32_into::<LittleEndian>(&mut ground_heightmap)?;

        // Scale the heightmap
        ground_heightmap
            .iter_mut()
            .for_each(|h| *h *= terrain_scale);

        // Skip unused data
        reader.seek_relative(length as i64 * 4)?;

        let mut texture_data: Vec<u8> = vec![0; length * 4];
        reader.read_exact(&mut texture_data)?;

        let texture = XPR::new_argb_lin(size.y, size.x, texture_data);

        return Ok(GND {
            gad,
            ground_heightmap,
            water_heightmap: vec![f32::NEG_INFINITY; length],
            skybox: Skybox::DualFlat {
                texture_paths: sky_paths.map(|p| p.to_path_buf()),
            },
            texture: GNDTexture::Packed(texture),
            tilemap: None,
            tilemap_size: size / 4,
            tilemap_random_rotation: true,
        });
    }

    fn import_bmp(
        gad: GAD,
        height_path: &Path,
        texture_path: &Path,
        sky_path: &Path,
        terrain_scale: f32,
    ) -> Result<Self, std::io::Error> {
        let file = File::open(height_path)?;
        let mut reader = BufReader::new(file);

        let size = gad.size;

        let mut magic: [u8; 2] = [0; _];
        reader.read_exact(&mut magic)?;

        if magic != BMP_MAGIC {
            return Err(std::io::Error::other("BadMagic"));
        }

        let _file_size = reader.read_u32::<LittleEndian>()?;

        reader.seek_relative(4)?; // Reserved

        let data_offset = reader.read_u32::<LittleEndian>()? as usize;

        let info_header_size = reader.read_u32::<LittleEndian>()?;
        if info_header_size != 40 {
            return Err(std::io::Error::other("InvalidInfoHeaderSize"));
        }

        let width = reader.read_u32::<LittleEndian>()? as usize;
        let height = reader.read_u32::<LittleEndian>()? as usize;

        if width != size.x {
            return Err(std::io::Error::other("WrongWidth"));
        }

        if height != size.y {
            return Err(std::io::Error::other("WrongHeight"));
        }

        let planes = reader.read_u16::<LittleEndian>()?;
        if planes != 1 {
            return Err(std::io::Error::other("InvalidPlanes"));
        }

        let bits_per_pixel = reader.read_u16::<LittleEndian>()?;
        if bits_per_pixel != 24 {
            return Err(std::io::Error::other("BitsPerPixel not 24"));
        }

        let bytes_per_pixel = cmp::max(bits_per_pixel as usize / 8, 1);

        let compression = reader.read_u32::<LittleEndian>()?;
        if compression != 0 {
            return Err(std::io::Error::other("Compressed"));
        }

        reader.seek(SeekFrom::Start(data_offset as u64))?;

        let length = size.element_product();

        let mut data = Vec::with_capacity(length * bytes_per_pixel);
        reader.read_to_end(&mut data)?;

        let mut ground_heightmap: Vec<f32> = Vec::with_capacity(length);
        let mut water_heightmap: Vec<f32> = Vec::with_capacity(length);

        // BMPs are stored upside-down, so parse the lines backwards
        for line in data.chunks_exact(size.x * bytes_per_pixel).rev() {
            for pixel in line.chunks_exact(bytes_per_pixel) {
                let r = pixel[2]; // Ground
                let _g = pixel[1]; // Trees
                let b = pixel[0]; // Water

                let gh = r as f32 * terrain_scale;
                let wh = if b == 0 {
                    f32::NEG_INFINITY
                } else {
                    b as f32 * terrain_scale - terrain_scale * 0.5
                };

                ground_heightmap.push(gh);
                water_heightmap.push(wh);
            }
        }

        return Ok(GND {
            gad,
            ground_heightmap,
            water_heightmap,
            skybox: Skybox::Spherical {
                texture_path: sky_path.to_path_buf(),
            },
            texture: GNDTexture::External(texture_path.to_path_buf()),
            tilemap: None,
            tilemap_size: size / 20,
            tilemap_random_rotation: true,
        });
    }

    pub fn create_tilemap(
        &mut self,
        tile_count: usize,
        reader: &mut impl Read,
    ) -> Result<(), std::io::Error> {
        let tilemap_length = self.tilemap_size.element_product();
        let mut values: Vec<u16> = vec![0; tilemap_length];
        if tilemap_length == tile_count {
            for i in 0..tilemap_length {
                values[i] = i as u16;
            }
            self.tilemap_random_rotation = false;
        } else {
            reader.read_u16_into::<LittleEndian>(&mut values)?;
            for &v in values.iter() {
                if (v as usize) >= tile_count {
                    return Err(std::io::Error::other("InvalidTileMapValue"));
                }
            }
        }

        self.tilemap = Some(XPR::new_l16_lin(
            self.tilemap_size.x,
            self.tilemap_size.y,
            values,
        ));

        return Ok(());
    }

    pub fn apply_heightmap_holes(&mut self) {
        for y in 0..self.gad.size.y {
            for x in 0..self.gad.size.x {
                let attribute = self.gad.get_attribute(x, y);
                if attribute.0 != 0 {
                    // This vertex has some collision, keep going
                    continue;
                }

                // Some holes need to be biased to higher terrain in the +X / +Y directions
                let height = self.get_height(x, y);
                if x < self.gad.size.x - 1 && self.get_height(x + 1, y) > height {
                    self.gad
                        .set_attribute(x, y, self.gad.get_attribute(x + 1, y));
                    self.gad.set_attribute(x + 1, y, attribute);
                    self.set_height(x + 1, y, f32::NAN);
                } else if y < self.gad.size.y - 1 && self.get_height(x, y + 1) > height {
                    self.gad
                        .set_attribute(x, y, self.gad.get_attribute(x, y + 1));
                    self.gad.set_attribute(x, y + 1, attribute);
                    self.set_height(x, y + 1, f32::NAN);
                } else {
                    self.set_height(x, y, f32::NAN);
                }
            }
        }
    }

    fn fill_water_heightmap(&mut self, height: f32) {
        self.water_heightmap.fill(height);
    }

    pub fn get_min_height(&self) -> f32 {
        self.ground_heightmap
            .iter()
            .cloned()
            .fold(0.0 / 0.0, f32::min)
    }

    pub fn get_max_height(&self) -> f32 {
        self.ground_heightmap
            .iter()
            .cloned()
            .fold(0.0 / 0.0, f32::max)
    }

    fn set_height(&mut self, x: usize, y: usize, height: f32) {
        self.ground_heightmap[y * self.gad.size.x + x] = height;
    }

    pub fn get_height(&self, x: usize, y: usize) -> f32 {
        return self.ground_heightmap[y * self.gad.size.x + x];
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

        let length = size.element_product();

        let mut attributes: Vec<Surface> = Vec::with_capacity(length);
        for _ in 0..length {
            let surface_bit = reader.read_u48::<LittleEndian>()?;
            let layers = reader.read_u16::<LittleEndian>()?;

            attributes.push(Surface::new(layers, surface_bit));
        }

        return Ok(GAD { size, attributes });
    }

    fn set_attribute(&mut self, x: usize, y: usize, attribute: Surface) {
        self.attributes[y * self.size.x + x] = attribute;
    }

    pub fn get_attribute(&self, x: usize, y: usize) -> Surface {
        return self.attributes[y * self.size.x + x];
    }
}

#[derive(Default)]
struct MissionText {
    prefix: String,

    title: u16,
    attack_objective: Option<u16>,
    defense_objective: Option<u16>,
    symmetric_targets: bool,
    attack_targets: Option<u16>,
    defense_targets: Option<u16>,
}

impl MissionText {
    fn new_loc(id: usize) -> Self {
        let mut text = Self::default();
        text.prefix = "loc".to_owned();
        text.title = id as u16 + 1233;
        match id {
            3 => {
                text.attack_objective = Some(1265);
                text.defense_objective = Some(1266);
            }
            4 => {
                text.symmetric_targets = true;
                text.attack_targets = Some(1267);
                text.defense_targets = Some(1268);
            }
            5 => {
                text.attack_targets = Some(1269);
                text.defense_targets = Some(1270);
            }
            6 => {
                text.attack_objective = Some(1271);
                text.defense_objective = Some(1272);
            }
            9 => {
                text.attack_targets = Some(1273);
                text.defense_targets = Some(1274);
            }
            10 => {
                text.symmetric_targets = true;
                text.attack_targets = Some(1276);
                text.defense_targets = Some(1275);
            }
            15 => {
                text.attack_objective = Some(1277);
                text.defense_objective = Some(1278);
            }
            16 => {
                text.attack_objective = Some(1279);
                text.defense_objective = Some(1279);
            }
            17 => {
                text.attack_objective = Some(1280);
                text.defense_objective = Some(1280);
            }
            22 => {
                text.attack_targets = Some(1281);
                text.defense_targets = Some(1282);
            }
            23 => {
                text.attack_targets = Some(1283);
                text.defense_targets = Some(1284);
            }
            24 | 25 | 26 => {
                text.attack_targets = Some(1285);
                text.defense_targets = Some(1286);
            }
            _ => {}
        }
        return text;
    }

    fn new_sb(id: usize) -> Self {
        let mut text = Self::default();
        text.prefix = "sb".to_owned();
        text.title = id as u16;
        return text;
    }

    fn write_prefixed(
        &self,
        text: Option<u16>,
        writer: &mut impl Write,
    ) -> Result<(), std::io::Error> {
        write_pascal_option_string(
            text.and_then(|t| Some(format!("{}:{:04}", self.prefix, t)))
                .as_deref(),
            writer,
        )
    }

    fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.write_prefixed(Some(self.title), writer)?;
        self.write_prefixed(self.attack_objective, writer)?;
        self.write_prefixed(self.defense_objective, writer)?;
        writer.write_u8(self.symmetric_targets as u8)?;
        self.write_prefixed(self.attack_targets, writer)?;
        self.write_prefixed(self.defense_targets, writer)?;

        return Ok(());
    }
}

pub struct Mission {
    id: usize,
    offset: Vec3,
    scale: f32,
    pub ground: Option<GND>,
    pub stages: Vec<STG>,
    text: MissionText,
}

impl Mission {
    pub fn import_loc(
        id: usize,
        gad_path: &Path,
        gnd_path: &Path,
        stg_paths: [&Path; LOC_STG_COUNT],
        sky_paths: [&Path; SKYBOX_DUAL_FLAT_COUNT],
        terrain_scale: f32,
        object_scale: f32,
        fps: f32,
    ) -> Result<Self, std::io::Error> {
        let gad = GAD::import(gad_path)?;

        let scale = object_scale / terrain_scale; // 50 == (0.01 / 0.0002) == (5000 / 100)

        let offset =
            Vec3::new(gad.size.x as f32 - 1.0, 0.0, gad.size.y as f32 - 1.0) * scale * -0.5;

        let mut ground = if gnd_path.is_file() {
            let gnd = GND::import_gnd(gad, &gnd_path, sky_paths, terrain_scale)?;
            println!(
                "Terrain height min: {}, max: {}",
                gnd.get_min_height() * scale,
                gnd.get_max_height() * scale,
            );
            Some(gnd)
        } else {
            None
        };

        let mut stages: Vec<STG> = Vec::with_capacity(LOC_STG_COUNT);
        for p in stg_paths {
            let stg = STG::import(p, object_scale, fps)?;
            if let Some(fstg) = stages.get(0) {
                // Check that draw flags are always consistent
                assert!(fstg.draw_water == stg.draw_water);
                assert!(fstg.draw_terrain == stg.draw_terrain);

                if stg.draw_water {
                    assert!(fstg.water_height == stg.water_height);
                }
            } else if stg.draw_water {
                println!("Water height: {}", stg.water_height);
            }

            println!(
                "Sun front: {}, back: {}, flash: {}",
                stg.sun_front_size, stg.sun_back_size, stg.sun_flash_power
            );

            if stg.draw_terrain {
                println!(
                    "Sky height: {:.03}, Velocity 0: {:.03}, Velocity 1: {:.03}",
                    stg.sky_height, stg.sky_velocity[0], stg.sky_velocity[1],
                );
            }

            stages.push(stg);
        }
        assert!(stages.len() == LOC_STG_COUNT);

        let fstg = &stages[0];
        assert!(fstg.draw_terrain == ground.is_some());

        if let Some(gnd) = ground.as_mut() {
            if fstg.draw_water {
                gnd.fill_water_heightmap(fstg.water_height * terrain_scale);
            }
        }

        return Ok(Mission {
            id,
            offset,
            scale,
            ground,
            stages,
            text: MissionText::new_loc(id),
        });
    }

    pub fn import_sb(
        id: usize,
        gad_path: &Path,
        height_path: &Path,
        texture_path: &Path,
        stg_path: &Path,
        sky_path: &Path,
        object_scale: f32,
        fps: f32,
    ) -> Result<Self, std::io::Error> {
        let gad = GAD::import(gad_path)?;

        let stage = STG::import(stg_path, object_scale, fps)?;

        let scale = 5.0 * object_scale;

        let offset =
            Vec3::new(gad.size.x as f32 - 1.0, 0.0, gad.size.y as f32 - 1.0) * scale * -0.5;

        let ground = GND::import_bmp(
            gad,
            height_path,
            texture_path,
            sky_path,
            stage.terrain_scale / 5.0,
        )?;

        return Ok(Mission {
            id,
            offset,
            scale,
            ground: Some(ground),
            stages: vec![stage],
            text: MissionText::new_sb(id),
        });
    }

    // https://gamedev.stackexchange.com/questions/24572/how-does-terrain-following-work-on-height-map
    pub fn sample_height(&self, mut pos: Vec3) -> Option<f32> {
        if self.ground.is_none() {
            return None;
        }

        if pos.x < 0.0 || pos.z < 0.0 {
            return None;
        }

        pos /= self.scale;

        let p0 = USizeVec2::new(pos.x as usize, pos.z as usize);
        let p1 = p0 + USizeVec2::ONE;

        let ground = self.ground.as_ref().unwrap();
        let h0 = ground.get_height(p0.x, p0.y);
        let h1 = ground.get_height(p1.x, p0.y);
        let h2 = ground.get_height(p0.x, p1.y);
        let h3 = ground.get_height(p1.x, p1.y);

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
        &mut self,
        path: &Path,
        objects: &[OBJ],
        model_path_fn: FnIndexToPath,
        dds_paths: [&Path; 4],
    ) -> Result<(), std::io::Error> {
        let mut mission_path = path.to_path_buf();

        let fstg = &self.stages[0];

        mission_path.push(format!("Mission_{:02}.mission_scene", self.id));
        {
            let file = File::create(&mission_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u32::<LittleEndian>(self.stages.len() as u32)?;
            writer.write_u8(fstg.draw_terrain as u8)?;

            self.text.write(&mut writer)?;

            write_godot_path(&dds_paths[2], &mut writer)?; // Map Big DDS
            write_godot_path(&dds_paths[3], &mut writer)?; // Map Small DDS

            write_godot_path(&dds_paths[0], &mut writer)?; // Object DDS
            OBJ::write_objects(objects, self.offset, model_path_fn, &mut writer)?;

            let start_pos = fstg.start_pos + self.offset;
            for v in start_pos.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }
            writer.write_f32::<LittleEndian>(fstg.start_yaw)?;
        }
        mission_path.pop();

        if let Some(ground) = self.ground.as_mut() {
            mission_path.push("terrain.dds");
            match &mut ground.texture {
                GNDTexture::Packed(texture) => {
                    texture.write_to_dds(&mission_path)?;
                }
                GNDTexture::External(path) => {
                    if let Some(ext) = path.extension() {
                        mission_path.set_extension(ext.to_ascii_lowercase());
                    }
                    fs::copy(path, &mission_path)?;
                }
            };
            let texture_path = mission_path.to_path_buf();
            mission_path.pop();

            if let Some(tilemap) = &mut ground.tilemap {
                mission_path.push("tilemap.dds");
                tilemap.write_to_dds(&mission_path)?;
                mission_path.pop();
            }

            mission_path.push("terrain.ground");
            let file = File::create(&mission_path)?;
            let mut writer = BufWriter::new(file);
            mission_path.pop();

            ground.skybox.write(&mut writer)?;

            let texture_file_name = texture_path.file_name().and_then(|s| s.to_str()).unwrap();
            write_pascal_string(texture_file_name, &mut writer)?;

            write_godot_path(&dds_paths[1], &mut writer)?; // Ground Tiles DDS

            writer.write_u32::<LittleEndian>(ground.gad.size.x as u32)?;
            writer.write_u32::<LittleEndian>(ground.gad.size.y as u32)?;
            writer.write_f32::<LittleEndian>(self.scale)?;

            writer.write_u32::<LittleEndian>(ground.tilemap_size.x as u32)?;
            writer.write_u32::<LittleEndian>(ground.tilemap_size.y as u32)?;
            writer.write_u8(ground.tilemap_random_rotation as u8)?;

            for &v in ground.ground_heightmap.iter() {
                writer.write_f32::<LittleEndian>(v)?;
            }

            for &Surface(layers, _) in ground.gad.attributes.iter() {
                writer.write_u16::<LittleEndian>(layers)?;
            }

            for &Surface(_, surface_type) in ground.gad.attributes.iter() {
                writer.write_u8(surface_type)?;
            }

            for &v in ground.water_heightmap.iter() {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }

        for (i, stg) in self.stages.iter().enumerate() {
            mission_path.push(format!("tod{}.stage", i));
            stg.export(&mission_path)?;
            mission_path.pop();
        }

        return Ok(());
    }
}
