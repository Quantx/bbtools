#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>
#include <unistd.h>
#include <string.h>
#include <math.h>

#include "jWrite.h"

#ifdef __linux__
#define SEPARATOR '/'
#else
#define SEPARATOR '\\'
#endif

char * progname;

char out_path[265];
char json_buffer[1<<20]; // 1MB

struct section_header {
    uint32_t flags;
    // Virtual Address
    uint32_t vaddr;
    uint32_t vsize;
    // File Address
    uint32_t faddr;
    uint32_t fsize;
    // Name Address
    uint32_t name_addr;
    uint32_t ref_count;
    // Shared Reference Count Address
    uint32_t head_ref_count_addr;
    uint32_t tail_ref_count_addr;
    // SHA-1 checksum
    uint8_t checksum[20];
};

struct __attribute__((__packed__)) model_config {
    int16_t model;
    int16_t motion;
    int16_t hitbox;
    int16_t lsq;
};

struct __attribute__((__packed__)) mech_model_config {
    struct model_config mech_cfg;
    int16_t emblem_model;
    struct model_config hatch_cfg;
    int16_t unknown;
};

struct __attribute__((__packed__)) weapon_model_config {
    struct model_config cfg;
    uint32_t flags;
};

struct __attribute__((__packed__)) weapon_data {
    uint16_t id;
    uint16_t life;
    float initial_velocity;
    float boost_max;
    float boost_rate;
    float gravity_acceleration;
    float torso_turn_rate;
    float damage_type_range;
    float max_range;
    float min_range;
    uint16_t offensive_power;
    uint16_t rapid_fire;
    uint16_t firing_interval;
    uint16_t reload_interval;
    uint16_t volley_count;
    uint16_t volley_interval;
    uint8_t unknown0;
    uint8_t impact_effect;
    uint16_t number_of_bullets;
    uint16_t number_of_mags;
    uint16_t offensive_power_falloff;
    uint8_t weight;
    uint8_t fire_probability;
    uint16_t fire_damage;
    uint8_t unknown1;
    uint8_t tracking;
    uint16_t volley_spread;
    float horizontal_muzzle_offset;
    float vertical_muzzle_offset;
    float recoil;
    uint16_t damage_type;
    uint16_t firing_delay;
    uint8_t category; // Also called "trajectory"
    uint8_t flying_effect;
    uint8_t horizontal_muzzle_count;
    uint8_t vertical_muzzle_count;
};

struct __attribute__((__packed__)) smoke_data {
    uint8_t color_start_a;
    uint8_t color_start_r;
    uint8_t color_start_g;
    uint8_t color_start_b;
    uint8_t color_max_a;
    uint8_t color_max_r;
    uint8_t color_max_g;
    uint8_t color_max_b;
    int32_t print_max;
    float start_size;
    float end_size;
    uint32_t flags;
    uint32_t padding[8];
};

struct __attribute__((__packed__)) tracer_data {
    uint8_t color_start_a;
    uint8_t color_start_r;
    uint8_t color_start_g;
    uint8_t color_start_b;
    int print_max;
    float start_size;
    uint32_t padding[8];
};

#define WEP_CLASS_COUNT 3
char * wep_classes[WEP_CLASS_COUNT] = {"mwep", "swep", "cwep"};

#define MWEP_DATA_LEN 26
uint32_t mwep_data_ptrs[MWEP_DATA_LEN];

#define SWEP_DATA_LEN 32
uint32_t swep_data_ptrs[SWEP_DATA_LEN];

#define TAMA_DATA_LEN 15
uint32_t tama_data_ptrs[TAMA_DATA_LEN];

#define MWEP_NAME_LEN 26
uint32_t mwep_name_ptrs[MWEP_NAME_LEN];

#define SWEP_NAME_LEN 33
uint32_t swep_name_ptrs[SWEP_NAME_LEN];

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Weapon Data Tool - By QuantX\n");

    if (!argc) {
        printf("Usage: %s wepdat.wcb\n", progname);
        return 1;
    }

    char * path = *argv++; argc--;

    FILE * wdat = fopen(path, "rb");
    if (!wdat) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 1;
    }

    strncpy(out_path, path, sizeof(out_path));
    char * sep = strrchr(out_path, SEPARATOR);
    if (sep) strcpy(sep + 1, "weapondata.json");
    else strcpy(out_path, "weapondata.json");

    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "failed to open output file: %s\n", out_path);
        return 1;
    }

    if (sep) strcpy(sep + 1, ".data.hdr");
    else strcpy(out_path, ".data.hdr");

    FILE * hdrf = fopen(out_path, "rb");
    if (!hdrf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    struct section_header hdr_data;
    fread(&hdr_data, sizeof(struct section_header), 1, hdrf);

    fclose(hdrf);

    if (sep) strcpy(sep + 1, ".rdata.hdr");
    else strcpy(out_path, ".rdata.hdr");

    hdrf = fopen(out_path, "rb");
    if (!hdrf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    struct section_header hdr_rdata;
    fread(&hdr_rdata, sizeof(struct section_header), 1, hdrf);

    fclose(hdrf);

    if (sep) strcpy(sep + 1, ".data.seg");
    else strcpy(out_path, ".data.seg");

    FILE * datf = fopen(out_path, "rb");
    if (!datf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    if (sep) strcpy(sep + 1, ".rdata.seg");
    else strcpy(out_path, ".rdata.seg");

    FILE * rdatf = fopen(out_path, "rb");
    if (!rdatf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    uint32_t file_count;
    fread(&file_count, sizeof(uint32_t), 1, wdat);

    uint32_t file_sizes[file_count];
    fread(file_sizes, sizeof(uint32_t), file_count, wdat);

    fseek(datf, 0x58A90, SEEK_SET);
    fread(tama_data_ptrs, sizeof(uint32_t), TAMA_DATA_LEN, datf);

    fseek(datf, 0x58620, SEEK_SET);
    fread(mwep_data_ptrs, sizeof(uint32_t), MWEP_DATA_LEN, datf);

    fseek(datf, 0x58998, SEEK_SET);
    fread(swep_data_ptrs, sizeof(uint32_t), SWEP_DATA_LEN, datf);

    fseek(datf, 0x58AE0, SEEK_SET);
    fread(mwep_name_ptrs, sizeof(uint32_t), MWEP_NAME_LEN, datf);

    fseek(datf, 0x58B48, SEEK_SET);
    fread(swep_name_ptrs, sizeof(uint32_t), SWEP_NAME_LEN, datf);

    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

    uint32_t weapon_type_count;
    fread(&weapon_type_count, sizeof(uint32_t), 1, wdat);

    if (weapon_type_count != WEP_CLASS_COUNT) {
        fprintf(stderr, "Weapon type count %u instead of %u\n", weapon_type_count, WEP_CLASS_COUNT);
        return 1;
    }

    uint32_t weapon_type_sizes[weapon_type_count];
    fread(weapon_type_sizes, sizeof(uint32_t), weapon_type_count, wdat);

    for (int we = 0; we < weapon_type_count; we++) {
        uint32_t weapon_count;
        fread(&weapon_count, sizeof(uint32_t), 1, wdat);

        uint32_t weapon_lookup_ptrs[weapon_count];
        fseek(datf, we ? 0x58790 : 0x58480, SEEK_SET);
        fread(weapon_lookup_ptrs, sizeof(uint32_t), weapon_count, datf);

        jwObj_array(wep_classes[we]);

        int weapons_processed = 0;
        for (int i = 0; i < weapon_count; i++) {
            struct weapon_data wep;
            fread(&wep, sizeof(struct weapon_data), 1, wdat);

            if (wep.id != i) {
                fprintf(stderr, "Weapon ID does not match! Got %d expected %d\n", wep.id, i);
                return 1;
            }

            uint32_t bullet_offset = 0;
            uint32_t weapon_offset = 0;

            if (weapon_lookup_ptrs[i] >= hdr_data.vaddr && weapon_lookup_ptrs[i] < hdr_data.vaddr + hdr_data.vsize) {
                fseek(datf, weapon_lookup_ptrs[i] - hdr_data.vaddr, SEEK_SET);
                fread(&bullet_offset, sizeof(uint32_t), 1, datf);
                fread(&weapon_offset, sizeof(uint32_t), 1, datf);
            }

            uint32_t * wep_data_ptrs = we ? swep_data_ptrs : mwep_data_ptrs;

            struct weapon_model_config wep_model_cfg;
            fseek(datf, wep_data_ptrs[weapon_offset] - hdr_data.vaddr, SEEK_SET);
            fread(&wep_model_cfg, sizeof(struct weapon_model_config), 1, datf);

            int16_t bullet_model;
            fseek(datf, tama_data_ptrs[bullet_offset] - hdr_data.vaddr, SEEK_SET);
            fread(&bullet_model, sizeof(int16_t), 1, datf);

            uint32_t * wep_name_ptrs = we ? swep_name_ptrs : mwep_name_ptrs;

            char name[16];
            fseek(rdatf, wep_name_ptrs[i] - hdr_rdata.vaddr, SEEK_SET);
            fread(name, sizeof(char), 16, rdatf);

            printf("WE %d | ID %02d: Weapon %04d, Bullet %04d, Flags: %08X, Name: \"%s\"\n", we, i, weapon_offset, bullet_offset, wep_model_cfg.flags, name);

            if (wep.unknown0 || wep.unknown1) {
                fprintf(stderr, "WEP unknown was non zero, id: %d\n", wep.id);
                return 1;
            }

            jwArr_object();

            jwObj_int("id", wep.id);
            jwObj_string("name", name);
            jwObj_int("life", wep.life);
            jwObj_double("initial_velocity", wep.initial_velocity);
            jwObj_double("boost_max", wep.boost_max);
            jwObj_double("boost_rate", wep.boost_rate);
            jwObj_double("gravity_acceleration", wep.gravity_acceleration);
            jwObj_double("torso_turn_rate", wep.torso_turn_rate);
            jwObj_double("damage_type_range", wep.damage_type_range);
            jwObj_double("max_range", wep.max_range);
            jwObj_double("min_range", wep.min_range);
            jwObj_int("offensive_power", wep.offensive_power);
            jwObj_int("rapid_fire", wep.rapid_fire);
            jwObj_int("firing_interval", wep.firing_interval);
            jwObj_int("reload_interval", wep.reload_interval);
            jwObj_int("volley_count", wep.volley_count);
            jwObj_int("volley_interval", wep.volley_interval);
//            jwObj_int("unknown0", wep.unknown0);
            jwObj_int("impact_effect", wep.impact_effect);
            jwObj_int("number_of_bullets", wep.number_of_bullets);
            jwObj_int("number_of_mags", wep.number_of_mags);
            jwObj_int("offensive_power_falloff", wep.offensive_power_falloff);
            jwObj_int("weight", wep.weight);
            jwObj_int("fire_probability", wep.fire_probability);
            jwObj_int("fire_damage", wep.fire_damage);
//            jwObj_int("unknown1", wep.unknown1);
            jwObj_int("tracking", wep.tracking);
            jwObj_int("volley_spread", wep.volley_spread);
            jwObj_double("horizontal_muzzle_offset", wep.horizontal_muzzle_offset);
            jwObj_double("vertical_muzzle_offset", wep.vertical_muzzle_offset);
            jwObj_double("recoil", wep.recoil);
            jwObj_int("damage_type", wep.damage_type);
            jwObj_int("firing_delay", wep.firing_delay);
            jwObj_int("category", wep.category);
            jwObj_int("flying_effect", wep.flying_effect);
            jwObj_int("horizontal_muzzle_count", wep.horizontal_muzzle_count);
            jwObj_int("vertical_muzzle_count", wep.vertical_muzzle_count);

            jwObj_int("bullet_model", bullet_model);
            jwObj_int("weapon_model", wep_model_cfg.cfg.model);
            jwObj_int("weapon_motion", wep_model_cfg.cfg.motion);
            jwObj_int("weapon_hitbox", wep_model_cfg.cfg.hitbox);
            jwObj_int("weapon_lsq", wep_model_cfg.cfg.lsq);

            jwObj_bool("shoulder", wep_model_cfg.flags & 0x00010000); // Shoulder weapon or SWEP BOX weapon
            jwObj_bool("shield",   wep_model_cfg.flags & 0x00040000);
            jwObj_bool("melee",    wep_model_cfg.flags & 0x00080000);
            jwObj_bool("mounted",  wep_model_cfg.flags & 0x08000000); // Whether or not this contributes to "MOUNT OVER"
            jwObj_bool("fixed",    wep_model_cfg.flags & 0x10000000); // Whether or not this weapon can be unequipped

            jwEnd();

            weapons_processed++;
        }

        printf("Weapon entry %d: Processed %d weapons\n", we, weapons_processed);

        jwEnd();
    }

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        fclose(outf);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    if (sep) strcpy(sep + 1, "trail_data.json");
    else strcpy(out_path, "trail_data.json");

    outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "failed to open output file: %s\n", out_path);
        return 1;
    }

    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

    uint32_t smoke_type_count;
    fread(&smoke_type_count, sizeof(uint32_t), 1, wdat);

    uint32_t smoke_type_sizes[smoke_type_count];
    fread(smoke_type_sizes, sizeof(uint32_t), smoke_type_count, wdat);

    jwObj_array("smoke");
    for (uint32_t sm = 0; sm < 20; sm++) {
        struct smoke_data smk;
        fread(&smk, sizeof(struct smoke_data), 1, wdat);

        if (!smk.print_max) continue; // Skip empty entries

        jwArr_object();
        jwObj_array("color_start");
            jwArr_int(smk.color_start_r);
            jwArr_int(smk.color_start_g);
            jwArr_int(smk.color_start_b);
            jwArr_int(smk.color_start_a);
        jwEnd();

        jwObj_array("color_max");
            jwArr_int(smk.color_max_r);
            jwArr_int(smk.color_max_g);
            jwArr_int(smk.color_max_b);
            jwArr_int(smk.color_max_a);
        jwEnd();

        jwObj_int("print_max", smk.print_max);
        jwObj_double("start_size", smk.start_size);
        jwObj_double("end_size", smk.end_size);
        jwObj_int("flags", smk.flags);

        for (int i = 0; i < 8; i++) {
            if (smk.padding[i]) {
                fprintf(stderr, "PADDING AT %d:%d WAS NON ZERO: %08X\n", sm, i, smk.padding[i]);
                return 1;
            }
        }
        jwEnd();
    }
    jwEnd();

    jwObj_array("tracer");
    for (uint32_t tr = 0; tr < 20; tr++) {
        struct tracer_data tracer;
        fread(&tracer, sizeof(struct tracer_data), 1, wdat);

        if (!tracer.print_max) continue; // Skip empty entries

        jwArr_object();
        jwObj_array("color_start");
            jwArr_int(tracer.color_start_r);
            jwArr_int(tracer.color_start_g);
            jwArr_int(tracer.color_start_b);
            jwArr_int(tracer.color_start_a);
        jwEnd();

        jwObj_int("print_max", tracer.print_max);
        jwObj_double("start_size", tracer.start_size);

        for (int i = 0; i < 8; i++) {
            if (tracer.padding[i]) {
                fprintf(stderr, "PADDING AT %d:%d WAS NON ZERO: %08X\n", tr, i, tracer.padding[i]);
                return 1;
            }
        }
        jwEnd();
    }

    // Manually add the last entry
    jwArr_object();
      jwObj_array("color_start");
          jwArr_int(0xFF);
          jwArr_int(0xC0);
          jwArr_int(0x80);
          jwArr_int(0xFF);
      jwEnd();

      jwObj_int("print_max", 4);
      jwObj_double("start_size", 35.0);
    jwEnd();

    jwEnd();

    fclose(wdat);
    fclose(datf);
    fclose(rdatf);

    jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        fclose(outf);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    return 0;
}
