#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <math.h>

#include "jWrite.h"

#ifdef __linux__
const char separator = '/';
#else
const char separator = '\\';
#endif

#define EFFECT_ENTRY_ATTR_REV_VELO 0x1
#define EFFECT_ENTRY_ATTR_NOT_FOG 0x2
#define EFFECT_ENTRY_ATTR_DIR_SPRT 0x4
#define EFFECT_ENTRY_ATTR_NOT_BILL 0x100
#define EFFECT_ENTRY_ATTR_FLG_COCKPIT 0x8000

struct file_entry {
    uint32_t offset;
    uint32_t length;
} __attribute__((packed));

struct vector2 {
    float x, y;
} __attribute__((packed));

struct vector3 {
    float x, y, z;
} __attribute__((packed));

struct color4 {
    float r, g, b, a;
} __attribute__((packed));

struct effect_entry {
    int16_t effect_type; // 0 = 2D_SIN, 1 = 2D_REP, 2 = 2D_PAR, 3 = LINPAR, 4 = GK, 5 = 3D_SIN, 6 = 3D_REP, 7 = 3D_PAR
    int8_t blend_type; // 0 = Alpha, 1 = Add, 2 = Subtract
    int8_t jnt;
    int16_t priority;
    int16_t texture;
    int16_t uv;
    int16_t sequence;
    int16_t model;
    uint16_t attrs; // Uses EFFECT_ENTRY_ATTR
    uint8_t vertex_color_b;
    uint8_t vertex_color_g;
    uint8_t vertex_color_r;
    uint8_t vertex_color_a;
    float color_damping_b;
    float color_damping_g;
    float color_damping_r;
    float color_damping_a;
    int32_t life;
    int32_t delay;
    struct vector3 position;
    struct vector3 rotation; // In radians, Brainbox2 shows degrees
    struct vector3 scale;
    struct vector3 position_velocity; // 0x58
    struct vector3 rotation_velocity; // 0x64
    struct vector3 scale_velocity; // 0x70
    struct vector3 position_acceleration; // 0x7C
    struct vector3 rotation_acceleration; // 0x88
    struct vector3 scale_acceleration; // 0x94
    struct vector3 gravity_acceleration; // 0xA0
    struct vector3 free0[3]; // 0xAC
    struct vector3 free1[3]; // 0xD0
    struct vector3 free2[3]; // 0xF4 (default: free2[1].y == -20.0f)
    struct vector3 free3[3]; // 0x118
};


/*
    free0[0].x = 00
    free0[0].y = 01
    free0[0].z = 02
    free0[1].x = 03
    free0[1].y = 04
    free0[1].z = 05
    free0[2].x = 06
    free0[2].y = 07
    free0[2].z = 08

    free1[0].x = 09
    free1[0].y = 10
    free1[0].z = 11
    free1[1].x = 12
    free1[1].y = 13
    free1[1].z = 14
    free1[2].x = 15
    free1[2].y = 16
    free1[2].z = 17

    free2[0].x = 18
    free2[0].y = 19
    free2[0].z = 20
    free2[1].x = 21
    free2[1].y = 22
    free2[1].z = 23
    free2[2].x = 24
    free2[2].y = 25
    free2[2].z = 26

    free3[0].x = 27
    free3[0].y = 28
    free3[0].z = 29
    free3[1].x = 30
    free3[1].y = 31
    free3[1].z = 32
    free3[2].x = 33
    free3[2].y = 34
    free3[2].z = 35
*/

struct x86_mov_float {
    uint32_t inst;
    float val;
};

struct mech_light {
    uint32_t type;
    float x;
    float y;
    float z;
};


char * progname;
char out_path[512];

char json_buffer[1<<20]; // 1MB

const float original_fps = 20.0f;

int unpackEFP(char * path) {
    FILE * efp = fopen(path, "rb");
    if (!efp) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        return 1;
    }

    char * ext = strrchr(path, '.');
    strcpy(ext + 1, "tbl");

    FILE * tbl = fopen(path, "rb");
    if (!tbl) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        fclose(efp);
        return 1;
    }

    char * sep = strrchr(path, separator);
    if (sep) sep[1] = '\0';
    else *path = '\0';

    uint32_t efp_file_count;
    fread(&efp_file_count, sizeof(uint32_t), 1, efp);

    struct file_entry efp_files[efp_file_count];
    fread(efp_files, sizeof(struct file_entry), efp_file_count, efp);

    /*
    EFP0 = EFE
    EFP1 = SEQ
    EFP2 = UV1 (Used with SEQ entries)
    EFP3 = UV2 (Mostly UI sprite)
    */

    const char * file_exts[] = {"efe", "seq", "uv", "uv2"};

    for (int i = 0; i < efp_file_count; i++) {
        fseek(efp, efp_files[i].offset, SEEK_SET);

        uint32_t file_count;
        fread(&file_count, sizeof(uint32_t), 1, efp);

        struct file_entry files[file_count];
        fread(files, sizeof(struct file_entry), file_count, efp);

        switch (i) {
        case 0: printf("Unpacking %d effect files\n", file_count); break;
        case 1: printf("Unpacking %d sequence files\n", file_count); break;
        case 2: printf("Unpacking %d uv-coord files\n", file_count); break;
        case 3: printf("Unpacking %d uv2-coord files\n", file_count); break;
        }

        for (int j = 0; j < file_count; j++) {
            if (!files[j].length) continue;

            uint32_t id = j;
            if (i == 0) {
                if (fread(&id, sizeof(uint32_t), 1, tbl) != 1) {
                    fprintf(stderr, "Unexpected end-of-file when reading EFE IDs\n");
                    fclose(efp);
                    fclose(tbl);
                    return 1;
                }
            }

            snprintf(out_path, sizeof(out_path), "%seff%0*d.%s", path, i ? 3 : 4, id, file_exts[i]);
            FILE * out = fopen(out_path, "wb");
            if (!out) {
                fprintf(stderr, "Failed to open output file: %s\n", out_path);
                fclose(efp);
                fclose(tbl);
                return 1;
            }

            fseek(efp, efp_files[i].offset + files[j].offset, SEEK_SET);
            for (int b = 0; b < files[j].length; b++) {
                int byte = fgetc(efp);
                if (byte == EOF) {
                    fprintf(stderr, "Unexpected end-of-file\n");
                    fclose(efp);
                    fclose(out);
                    return 1;
                }
                fputc(byte, out);
            }

            fclose(out);
        }
    }

    fclose(tbl);
    fclose(efp);
    return 0;
}

void jwObj_vector3(char * name, struct vector3 * v) {
    jwObj_array(name);
    jwArr_double(v->x);
    jwArr_double(v->y);
    jwArr_double(v->z);
    jwEnd();
}

void jwObj_color4(char * name, struct color4 * c) {
    jwObj_array(name);
    jwArr_double(c->r);
    jwArr_double(c->g);
    jwArr_double(c->b);
    jwArr_double(c->a);
    jwEnd();
}

void jwObj_vector3a(char * name, struct vector3 * va, size_t vs) {
    jwObj_array(name);
    for (size_t i = 0; i < vs; i++) {
        jwArr_array();
        jwArr_double(va[i].x);
        jwArr_double(va[i].y);
        jwArr_double(va[i].z);
        jwEnd();
    }
    jwEnd();
}

int unpackEFE(char * path) {
    FILE * efe = fopen(path, "rb");
    if (!efe) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        return 1;
    }

    uint32_t entry_count;
    fread(&entry_count, sizeof(uint32_t), 1, efe);

    uint32_t entry_offsets[entry_count];
    fread(entry_offsets, sizeof(uint32_t), entry_count, efe);

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    printf("Converting effect with %d parts\n", entry_count);

    for (int i = 0; i < entry_count; i++) {
        fseek(efe, entry_offsets[i], SEEK_SET);

        if (i < entry_count - 1) {
            uint32_t length = entry_offsets[i + 1] - entry_offsets[i];
            //printf("Len %d, %d - %d\n", length, entry_offsets[i + 1], entry_offsets[i]);
            if (length != sizeof(struct effect_entry)) {
                fprintf(stderr, "Effect entry %d size missmatch, got %d expected %ld\n",
                    i, length, sizeof(struct effect_entry));
                fclose(efe);
                return 1;
            }
        }

        struct effect_entry effect;
        fread(&effect, sizeof(struct effect_entry), 1, efe);

        jwArr_object();

        jwObj_int("effect_type", effect.effect_type);
        jwObj_int("blend_type", effect.blend_type);
        jwObj_int("jnt", effect.jnt);
        jwObj_int("priority", effect.priority);
        jwObj_int("texture", effect.texture);
        jwObj_int("uv", effect.uv);
        jwObj_int("sequence", effect.sequence);
        jwObj_int("model", effect.model);

        jwObj_bool("attr_rev_velo", effect.attrs & EFFECT_ENTRY_ATTR_REV_VELO);
        jwObj_bool("attr_not_fog",  effect.attrs & EFFECT_ENTRY_ATTR_NOT_FOG);
        jwObj_bool("attr_dir_sprt", effect.attrs & EFFECT_ENTRY_ATTR_DIR_SPRT);
        jwObj_bool("attr_not_bill", effect.attrs & EFFECT_ENTRY_ATTR_NOT_BILL);
        jwObj_bool("attr_flg_cockpit", effect.attrs & EFFECT_ENTRY_ATTR_FLG_COCKPIT);

        if (effect.attrs & ~(EFFECT_ENTRY_ATTR_REV_VELO | EFFECT_ENTRY_ATTR_NOT_FOG | EFFECT_ENTRY_ATTR_DIR_SPRT | EFFECT_ENTRY_ATTR_NOT_BILL | EFFECT_ENTRY_ATTR_FLG_COCKPIT)) {
            fprintf(stderr, "Unknown attribute flag %04X in entry %02d\n", effect.attrs, i);
            fclose(efe);
            return 1;
        }

        jwObj_array("vertex_color");
            jwArr_int(effect.vertex_color_r);
            jwArr_int(effect.vertex_color_g);
            jwArr_int(effect.vertex_color_b);
            jwArr_int(effect.vertex_color_a);
        jwEnd();

        jwObj_array("damping_color");
            jwArr_double(effect.color_damping_r);
            jwArr_double(effect.color_damping_g);
            jwArr_double(effect.color_damping_b);
            jwArr_double(effect.color_damping_a);
        jwEnd();

        jwObj_int("life", effect.life);
        jwObj_int("delay", effect.delay);

        jwObj_vector3("position", &effect.position);
        jwObj_vector3("rotation", &effect.rotation);
        jwObj_vector3("scale", &effect.scale);

        jwObj_vector3("position_velocity", &effect.position_velocity);
        jwObj_vector3("rotation_velocity", &effect.rotation_velocity);
        jwObj_vector3("scale_velocity", &effect.scale_velocity);

        jwObj_vector3("position_acceleration", &effect.position_acceleration);
        jwObj_vector3("rotation_acceleration", &effect.rotation_acceleration);
        jwObj_vector3("scale_acceleration", &effect.scale_acceleration);

        jwObj_vector3("gravity_acceleration", &effect.gravity_acceleration);

        jwObj_vector3a("free0", effect.free0, 3);
        jwObj_vector3a("free1", effect.free1, 3);
        jwObj_vector3a("free2", effect.free2, 3);
        jwObj_vector3a("free3", effect.free3, 3);

        jwEnd();
    }

    fclose(efe);

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strncpy(out_path, path, sizeof(out_path));
    char * ext = strrchr(out_path, '.');

    strcpy(ext + 1, "json");
    FILE * out = fopen(out_path, "w");
    if (!out) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), out);

    fclose(out);

    return 0;
}

int unpackSEQ(char * path) {
    strncpy(out_path, path, sizeof(out_path));
    char * ext = strrchr(out_path, '.');

    FILE * seq = fopen(out_path, "rb");
    if (!seq) {
        fprintf(stderr, "Failed to open %s\n", out_path);
        return 2;
    }

    uint32_t seq_count;
    fread(&seq_count, sizeof(uint32_t), 1, seq);

    printf("Converting sequence file: %d frames\n", seq_count);

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    for (int i = 0; i < seq_count; i++) {
        int16_t seq_frame, seq_time;
        fread(&seq_frame, sizeof(int16_t), 1, seq);
        fread(&seq_time, sizeof(int16_t), 1, seq);

        jwArr_object();
        jwObj_int("frame", seq_frame);
        jwObj_int("time", seq_time);
        jwEnd();
    }

    fclose(seq);

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    // Rename it from eff000.seq to seq000.json
    memcpy(ext - 6, "seq", 3);
    strcpy(ext + 1, "json");
    FILE * out = fopen(out_path, "w");
    if (!out) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), out);

    fclose(out);
    return 0;
}

int unpackUV(char * path, bool isUI) {
    FILE * uv = fopen(path, "rb");
    if (!uv) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 2;
    }

    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

    uint32_t uv_count;
    fread(&uv_count, sizeof(uint32_t), 1, uv);

    uint32_t texture;
    fread(&texture, sizeof(uint32_t), 1, uv);
    jwObj_int("texture", texture);

    float offset[2];
    fread(offset, sizeof(float), 2, uv);
    jwObj_array("offset");
        jwArr_double(offset[0]);
        jwArr_double(offset[1]);
    jwEnd();

    printf("Converting UV file: %d frames, texture ID %d, offset (%.3f, %.3f)\n",
        uv_count, texture, offset[0], offset[1]);

    jwObj_array("frames");
    for (int i = 0; i < uv_count; i++) {
        jwArr_object();

        float vals[6];
        fread(vals, sizeof(float), 6, uv);

        jwObj_array("start");
            jwArr_double(vals[0]);
            jwArr_double(vals[1]);
        jwEnd();

        jwObj_array("end");
            jwArr_double(vals[2]);
            jwArr_double(vals[3]);
        jwEnd();

        jwObj_array("size");
            jwArr_double(vals[4]);
            jwArr_double(vals[5]);
        jwEnd();

        jwEnd();
    }
    jwEnd();

    fclose(uv);

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    // Rename it from eff000.seq to <out_ext>000.json
    strncpy(out_path, path, sizeof(out_path));
    char * ext = strrchr(out_path, '.');

    if (isUI) {
        memmove(ext - 4, ext - 3, 3); // Move ID back 1 character without overwriting self
        memcpy(ext - 6, "ui", 2); // Change name to ui
        strcpy(ext - 1, ".json"); // Add extension
    } else {
        memcpy(ext - 6, "spr", 3);
        strcpy(ext + 1, "json");
    }

    FILE * out = fopen(out_path, "w");
    if (!out) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), out);

    fclose(out);
    return 0;
}

int unpackLID(char * path) {
    FILE * lidf = fopen(path, "rb");
    if (!lidf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    uint32_t entry_count, entry_size;
    fread(&entry_count, sizeof(uint32_t), 1, lidf);
    fread(&entry_size, sizeof(uint32_t), 1, lidf);

    if (entry_size != 40) {
        fprintf(stderr, "Unrecognized entry size of %u\n", entry_size);
        fclose(lidf);
        return 1;
    }

    printf("Extracting %u lights\n", entry_count);

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    unsigned int valid_light_count = 0;

    for (unsigned int i = 0; i < entry_count; i++) {
        uint32_t flags;
        uint16_t colorAddCount, life;
        struct color4 colorAdd, colorBase;
        struct vector3 position;

        fread(&flags, sizeof(uint32_t), 1, lidf);
        fread(&colorAddCount, sizeof(uint16_t), 1, lidf);
        fread(&life, sizeof(uint16_t), 1, lidf);
        fread(&colorAdd, sizeof(struct color4), 1, lidf);
        fread(&colorBase, sizeof(struct color4), 1, lidf);

        if (flags != 1) continue;

        valid_light_count++;

        if (!colorAddCount) {
            colorAdd.r = 0.0f;
            colorAdd.g = 0.0f;
            colorAdd.b = 0.0f;
            colorAdd.a = 0.0f;
        }

        jwArr_object();
            jwObj_int("life", life);
            jwObj_int("colorAddCount", colorAddCount);

            jwObj_color4("colorBase", &colorBase);
            jwObj_color4("colorAdd", &colorAdd);
        jwEnd();
    }

    fclose(lidf);

    printf("Extracted %u valid lights\n", valid_light_count);

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strncpy(out_path, path, sizeof(out_path));
    char * ext = strrchr(out_path, '.');
    strcpy(ext + 1, "json");

    FILE * out = fopen(out_path, "w");
    if (!out) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), out);

    fclose(out);

    return 0;
}

int unpackSEG(char * path) {
    strncpy(out_path, path, sizeof(out_path));
    char * sep = strrchr(out_path, separator);

    if (sep) strcpy(sep + 1, ".data.seg");
    else strcpy(out_path, ".data.seg");

    FILE * datf = fopen(out_path, "rb");
    if (!datf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    printf("Unpacking engine data\n");

    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

    static uint32_t cockpitLightTypes[54];
    fseek(datf, 0x36F78, SEEK_SET);
    fread(cockpitLightTypes, sizeof(uint32_t), 54, datf);

    static float cockpitLightPositions[162];
    fseek(datf, 0x37050, SEEK_SET);
    fread(cockpitLightPositions, sizeof(float), 162, datf);

    static struct mech_light mechLights[32];
    fseek(datf, 0x56118, SEEK_SET);
    fread(mechLights, sizeof(struct mech_light), 32, datf);

    static int32_t mechCollisionEffects[34][28][9];
    fseek(datf, 0x38D80, SEEK_SET);
    fread(mechCollisionEffects, sizeof(int32_t), 34 * 28 * 9, datf);

    static int32_t collisionEffects[28][32];
    fseek(datf, 0x37F5C, SEEK_SET);
    fread(collisionEffects, sizeof(int32_t), 28 * 32, datf);

    // Fixup the overlapping data
    for (int i = 0; i < 9; i++) collisionEffects[0][i] = 0;

    static struct vector2 distortionEffects[8][18];
    fseek(datf, 0x109C0, SEEK_SET);
    fread(distortionEffects, sizeof(struct vector2), 8 * 18, datf);

    fclose(datf);

    jwObj_array("mechs");
    for (int i = 0 ; i < 32; i++) {
        struct mech_light * ml = mechLights + i;
        jwArr_object();
            jwObj_int("type", ml->type);
            jwObj_array("position");
                jwArr_double(ml->x);
                jwArr_double(ml->y);
                jwArr_double(ml->z);
            jwEnd();
        jwEnd();
    }
    jwEnd();

    jwObj_array("mech_collisions");
    for (int m = 0; m < 34; m++) {
        jwArr_array();
        for (int c = 0; c < 28; c++) {
            jwArr_array();
            for (int e = 0; e < 9; e++) {
                jwArr_int(mechCollisionEffects[m][c][e]);
            }
            jwEnd();
        }
        jwEnd();
    }
    jwEnd();

    jwObj_array("collisions");
    for (int c = 0; c < 28; c++) {
        jwArr_array();
        for (int e = 9; e < 32; e++) {
            // Skip the first 9 entries as they're all zeros
            jwArr_int(collisionEffects[c][e]);
        }
        jwEnd();
    }
    jwEnd();

    if (sep) strcpy(sep + 1, ".text.seg");
    else strcpy(out_path, ".text.seg");

    FILE * txtf = fopen(out_path, "rb");
    if (!txtf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    jwObj_array("cockpits");
    for (int c = 0; c < 6; c++) {
        jwArr_object();

        jwObj_array("status");
        for (int i = 0; i < 9; i++) {
            int l = c * 9 + i;
            jwArr_object();
                jwObj_int("type", cockpitLightTypes[l]);
                jwObj_array("position");
                    jwArr_double(cockpitLightPositions[l * 3]);
                    jwArr_double(cockpitLightPositions[l * 3 + 1]);
                    jwArr_double(cockpitLightPositions[l * 3 + 2]);
                jwEnd();
            jwEnd();
        }
        jwEnd();

        struct x86_mov_float mov;
        fseek(txtf, 0x5C6EE + sizeof(struct x86_mov_float) * 3 * c, SEEK_SET);
        jwObj_object("eject");
            jwObj_int("type", c + 273);
            jwObj_array("position");
            for (int i = 0; i < 3; i++) {
                fread(&mov, sizeof(struct x86_mov_float), 1, txtf);
                jwArr_double(mov.val);
            };
            jwEnd();
        jwEnd();

        if (c == 1) {
            fseek(txtf, 0x6A2FC, SEEK_SET);

            jwObj_array("comms");
            for (int i = 0; i < 5; i++) {
                jwArr_array();
                for (int j = 0; j < 3; j++) {
                    fread(&mov, sizeof(struct x86_mov_float), 1, txtf);
                    jwArr_double(mov.val);
                }
                jwEnd();
            }
            jwEnd();
        }
        jwEnd();
    }
    jwEnd();

    fclose(txtf);

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    if (sep) strcpy(sep + 1, "effect_cues.json");
    else strcpy(out_path, "effect_cues.json");

    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);

    fclose(outf);

    if (sep) strcpy(sep + 1, "distortion.data");
    else strcpy(out_path, "distortion.data");

    outf = fopen(out_path, "wb");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(distortionEffects, sizeof(struct vector2), 8 * 18, outf);

    fclose(outf);
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Effect Tool - By QuantX\n");

    if (!argc) {
        fprintf(stderr, "Please specify an effect file: %s <path/effect.efe>\n", progname);
        return 1;
    }

    char * path = *argv++; argc--;

    char * ext = strrchr(path, '.');
    if (!ext) {
        fprintf(stderr, "No extension present on path: %s\n", path);
        return 1;
    }

    ext++;
    if (!strcmp(ext, "efp")) return unpackEFP(path);
    else if (!strcmp(ext, "efe")) return unpackEFE(path);
    else if (!strcmp(ext, "seq")) return unpackSEQ(path);
    else if (!strcmp(ext,  "uv")) return unpackUV(path, false);
    else if (!strcmp(ext, "uv2")) return unpackUV(path, true);
    else if (!strcmp(ext, "seg")) return unpackSEG(path);
    else if (!strcmp(ext, "lid")) return unpackLID(path);
    else {
        fprintf(stderr, "Unknown file extension: %s\n", path);
        return 1;
    }
    return 0;
}
