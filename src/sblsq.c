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

struct vector3 {
    float x, y, z;
} __attribute__((packed));

struct anim_entry {
    uint32_t offset;
    uint32_t size;
    uint32_t anim; // Animation ID
} __attribute__((packed));

struct frame_entry {
    uint16_t frame; // Frame ID
    uint16_t unknown;
    uint32_t offset;
} __attribute__((packed));

enum LSQ_TYPE {
    LSQ_TYPE_EFF,
    LSQ_TYPE_SE,
    LSQ_TYPE_UNK0,
    LSQ_TYPE_FLAGS,
    LSQ_TYPE_UNK1,
    
    LSQ_TYPE_COUNT
};

const float scale_factor = 100.0f;
const float original_fps = 20.0f;

char * lsq_type_names[LSQ_TYPE_COUNT] = {
    "effects", "sounds", "unknown0", "flags", "unknown1",
};

uint32_t offsets[LSQ_TYPE_COUNT];

void emit_vector3(struct vector3 * v) {
    jwArr_double(v->x);
    jwArr_double(v->y);
    jwArr_double(v->z);
}

int unpackEFF(FILE * lsq) {
    uint16_t frame_idx, effect_count;
    fread(&frame_idx, sizeof(uint16_t), 1, lsq);
    fread(&effect_count, sizeof(uint16_t), 1, lsq);
    
    float time = (float)(frame_idx) / original_fps;
    printf("  Frame %u (%.2fs): Processing %u effects\n", frame_idx, time, effect_count);
    
    jwObj_array("effects");
    
    for (uint16_t ei = 0; ei < effect_count; ei++) {
        uint16_t zero0;
    
        uint16_t effect_id;
        fread(&effect_id, sizeof(uint16_t), 1, lsq); // 2
        
        fread(&zero0, sizeof(uint16_t), 1, lsq); // 4
        if (zero0) {
            fprintf(stderr, "zero0 was non-zero (0): %04X\n", zero0);
            return 1;
        }
        
        uint8_t jnt_idx_a, jnt_idx_b;
        fread(&jnt_idx_a, sizeof(uint8_t), 1, lsq); // 5
        fread(&jnt_idx_b, sizeof(uint8_t), 1, lsq); // 6
        
        fread(&zero0, sizeof(uint16_t), 1, lsq); // 8
        if (zero0) {
            fprintf(stderr, "zero0 was non-zero (1): %04X\n", zero0);
            return 1;
        }
        
        uint32_t flags;
        fread(&flags, sizeof(uint32_t), 1, lsq); // 12
        
        uint32_t zero1;
        fread(&zero1, sizeof(uint32_t), 1, lsq); // 16
        if (zero1) {
            fprintf(stderr, "zero1 was non-zero: %08X\n", zero1);
            return 1;
        }
        
        uint32_t jnt_delay;
        fread(&jnt_delay, sizeof(uint32_t), 1, lsq); // 20
        
        struct vector3 pos, rot, t_rot;
        fread(&pos, sizeof(struct vector3), 1, lsq); // 32
        fread(&rot, sizeof(struct vector3), 1, lsq); // 44
        fread(&t_rot, sizeof(struct vector3), 1, lsq); // 56
        
        pos.x /= scale_factor;
        pos.y /= scale_factor;
        pos.z /= scale_factor;
        
        printf("    Effect ID: %02u, JNT %03u %03u, Flags %08X, JNT %u\n",
            effect_id, jnt_idx_a, jnt_idx_b, flags, jnt_delay);
        
        printf("    Position (%.2f, %.2f, %.2f), Rotation (%.2f, %.2f, %.2f), Trans-Rotation (%.2f, %.2f, %.2f)\n",
            pos.x, pos.y, pos.z,
            rot.x, rot.y, rot.z,
            t_rot.x, t_rot.y, t_rot.z);
        
        jwArr_object();
        
        jwObj_int("id", effect_id);
        jwObj_int("jnt_a", jnt_idx_a);
        jwObj_int("jnt_b", jnt_idx_b);
        jwObj_int("flags", flags);
        jwObj_double("jnt_delay", (float)(jnt_delay) / original_fps);
        
        jwObj_array("position");
        emit_vector3(&pos);
        jwEnd();
        
        jwObj_array("rotation");
        emit_vector3(&rot);
        jwEnd();
        
        jwObj_array("trans_rotation");
        emit_vector3(&t_rot);
        jwEnd();
        
        jwEnd();
        
        for (int i = 0; i < 6; i++) {
            fread(&zero1, sizeof(uint32_t), 1, lsq);
            if (zero1) {
                fprintf(stderr, "zero1 was non-zero for float index %d: %08X\n", zero1, i);
                return 1;
            }
        }
    }
    
    jwEnd();
    
    return 0;
}

int unpackSE(FILE * lsq) {
    uint16_t frame_idx, sound_count;
    fread(&frame_idx, sizeof(uint16_t), 1, lsq);
    fread(&sound_count, sizeof(uint16_t), 1, lsq);
    
    float time = (float)(frame_idx) / original_fps;
    printf("  Frame %u (%.2fs): Processing %u sounds\n", frame_idx, time, sound_count);

    jwObj_array("sounds");

    for (uint16_t si = 0; si < sound_count; si++) {
        uint32_t sound_id;
        fread(&sound_id, sizeof(uint32_t), 1, lsq);
        
        uint32_t jnt;
        fread(&jnt, sizeof(uint32_t), 1, lsq);
        
        uint32_t flags;
        fread(&flags, sizeof(uint32_t), 1, lsq);
        
        printf("    Sound ID %u, JNT %u, Flags %08X\n", sound_id, jnt, flags);
        
        jwArr_object();
        
        jwObj_int("id", sound_id);
        jwObj_int("jnt", jnt);
        jwObj_int("flags", flags);
        
        jwEnd();
        
        for (int i = 0; i < 7; i++) {
            uint32_t zero;
            fread(&zero, sizeof(uint32_t), 1, lsq);
            
            if (zero) {
                fprintf(stderr, "Flags zero was non-zero: %08X\n", zero);
                return 1;
            }
        }
    }
    
    jwEnd();

    return 0;
}

int unpackUNK0(FILE * lsq) {
    fprintf(stderr, "Unknown 0 contained data!\n");
    return 1;
}

int unpackFLAGS(FILE * lsq) {
    uint16_t frame_idx, flag_count;
    fread(&frame_idx, sizeof(uint16_t), 1, lsq);
    fread(&flag_count, sizeof(uint16_t), 1, lsq);
    
    float time = (float)(frame_idx) / original_fps;
    printf("  Frame %u (%.2fs): Processing %u flags\n", frame_idx, time, flag_count);
    
    if (flag_count != 1) {
        fprintf(stderr, "More than 1 flags in this frame\n");
        return 1;
    }
    
    for (uint16_t fi = 0; fi < flag_count; fi++) {
        uint16_t flags;
        fread(&flags, sizeof(uint16_t), 1, lsq);
        
        printf("    Flags: %04X\n", flags);
        
        // Only output the very last flags
        if (fi == flag_count - 1) jwObj_int("flags", flags);
        
        for (int i = 0; i < 13; i++) {
            uint16_t zero;
            fread(&zero, sizeof(uint16_t), 1, lsq);
            
            if (zero) {
                fprintf(stderr, "Flags zero was non-zero: %04X\n", zero);
                return 1;
            }
        }
    }
    
    return 0;
}

int unpackUNK1(FILE * lsq) {
    fprintf(stderr, "Unknown 1 contained data!\n");
    return 1;
}

int (*unpackFuncs[LSQ_TYPE_COUNT])(FILE * lsq) = {
    unpackEFF, unpackSE, unpackUNK0, unpackFLAGS, unpackUNK1,
};

int unpack(FILE * lsq, enum LSQ_TYPE type) {
    fseek(lsq, offsets[type], SEEK_SET);
    
    printf("*** Unpacking %s ***\n", lsq_type_names[type]);
    
    uint8_t total_anim_count, anim_count;
    fread(&total_anim_count, sizeof(uint8_t), 1, lsq);
    fread(&anim_count, sizeof(uint8_t), 1, lsq);
    
    printf("Processing %u animation entries, %u total animations\n", anim_count, total_anim_count);
    
    fseek(lsq, 2, SEEK_CUR);
    
    struct anim_entry anims[anim_count];
    fread(anims, sizeof(struct anim_entry), anim_count, lsq);
    
    jwObj_array(lsq_type_names[type]);
    
    for (uint32_t ai = 0; ai < anim_count; ai++) {
        struct anim_entry * anim = anims + ai;
        fseek(lsq, offsets[type] + anim->offset, SEEK_SET);
        
        uint32_t frame_count;
        fread(&frame_count, sizeof(uint32_t), 1, lsq);
        
        printf("Animation %u: Processing %u frames\n", anim->anim, frame_count);
        
        struct frame_entry frames[frame_count];
        fread(frames, sizeof(struct frame_entry), frame_count, lsq);
        
        jwArr_object();
        
        jwObj_int("animation", anim->anim);
        jwObj_array("frames");
        
        for (uint32_t fi = 0; fi < frame_count; fi++) {
            struct frame_entry * frame = frames + fi;
            fseek(lsq, offsets[type] + anim->offset + frame->offset, SEEK_SET);
            
            jwArr_object();
            jwObj_double("time", (float)(frame->frame) / original_fps);
            
            if (unpackFuncs[type](lsq)) return 1;
            
            jwEnd();
        }
        
        jwEnd();
        jwEnd();
    }
    
    jwEnd();
    
    return 0;
}

char * progname;
char out_path[256];
char json_buffer[1<<20]; // 1MB

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB LSQ Tool - By QuantX\n");

    if (!argc) {
        fprintf(stderr, "Please specify an LSQ file: %s <path/0000.lsq>\n", progname);
        return 1;
    }
    
    char * path = *argv++; argc--;
    
    strncpy(out_path, path, sizeof(out_path));
    
    char * ext = strrchr(out_path, '.');
    if (!ext) {
        fprintf(stderr, "No extension present on path: %s\n", path);
        return 1;
    }
    ext++;
    
    if (strcmp(ext, "lsq")) {
        fprintf(stderr, "Not a \".lsq\" file extension: %s\n", path);
        return 1;
    }
    
    FILE * lsq = fopen(path, "rb");
    if (!lsq) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        return 1;
    }
    
    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);
    
    fread(offsets, sizeof(uint32_t), LSQ_TYPE_COUNT, lsq);
    
    for (enum LSQ_TYPE type = 0; type < LSQ_TYPE_COUNT; type++) {
        if (!offsets[type]) continue;
        
        if (unpack(lsq, type)) return 1;
    }
    
    fclose(lsq);
    
    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }
    
    strcpy(ext, "json");
    
    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }
    
    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);
    return 0;
}
