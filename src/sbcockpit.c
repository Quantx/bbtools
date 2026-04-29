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

struct file_entry {
    uint32_t offset;
    uint32_t length;
};

struct light_entry {
    int16_t arg0;
    int16_t arg1;
    float x;
    float y;
    float z;
    float r;
    float g;
    float b;
    float energy;
};

struct ambient_entry {
    float amb_r;
    float amb_g;
    float amb_b;
    float amb_energy;
    float lit_r;
    float lit_g;
    float lit_b;
    float lit_energy;
    float yaw;
    float pitch;
};

#define BOOT_AMBIENT_COUNT 50

struct boot_ambient_entry { // 44 bytes
    int32_t frame;
    struct ambient_entry ambient;
};

struct boot_plight_entry { // 36 bytes (9 values)
    int32_t frame;
    float unknown;
    float x;
    float y;
    float z;
    float r;
    float g;
    float b;
    float energy;
};

struct vector2 {
    float x, y;
};

struct vector3 {
    float x, y, z;
};

struct os_lines_pointer {
    uint32_t count;
    uint32_t offset;
};

struct os_sprite {
    int32_t sprite_idx;
    struct vector2 pos;
    struct vector2 size;
    uint32_t color;
};

struct os_sprite2 {
    int32_t ui_idx;
    int32_t frame_idx;
    struct vector2 pos;
    struct vector2 origin;
    struct vector2 size;
    float rotation;
    float scale;
    float fade;
    float fade_add;
    float fade_sub;
    uint32_t color_ptr;
    uint32_t fade_target_ptr;
    int32_t fade_target_value;
};

struct os_draw_text {
    struct vector2 pos;
    uint32_t id;
    uint32_t arg;
    uint32_t padding[9]; // All 0s?
    uint32_t color; // Always 0x0000FFAA
};

struct os_draw_quad { // ID: 0x2
    float points[8];
    uint32_t colors[4]; // BGRA
    uint32_t padding[2];
};

struct os_draw_line { // ID: 0x3
    struct vector2 start;
    struct vector2 end;
    uint32_t color; // BGRA
    uint32_t padding[9]; // All 0s?
};

struct os_draw_sprite { // ID: 0x4
    struct vector2 start;
    struct vector2 end;
    struct vector2 start2;
    struct vector2 end2;
    uint32_t color; // BGRA
    uint32_t id;
    uint32_t padding[4]; // All 0s?
};

struct os_draw_lines { // ID: 0x5
    struct vector2 pos;
    uint32_t color; // BGRA
    uint32_t padding0;
    float scale;
    uint32_t id;
    uint32_t padding1[8];
};

struct os_draw { // 64 bytes (16 x 4)
    uint32_t type;
    int32_t anims;
    union {
        struct os_draw_text text; // 0x1
        struct os_draw_quad quad; // 0x2
        struct os_draw_line line; // 0x3
        struct os_draw_sprite sprite; // 0x4
        struct os_draw_lines lines; // 0x5
    };
};

struct os_anim_1 { // ID: 0x1
    uint32_t padding[13];
};

struct os_anim_points { // ID: 0x2
    float points[8];
    uint32_t padding[5];
};

struct os_anim_rotate { // ID: 0x4
    uint32_t padding0;
    uint32_t flag;
    uint32_t padding1[11];
};

struct os_anim_color { // ID: 0x8
    uint32_t color;
    uint32_t padding[12]; // All 0s?
};

struct os_anim_colors { // ID: 0x10
    uint32_t colors[4];
    uint32_t padding[9];
};

struct os_anim_text { // ID: 0x20
    uint32_t padding[13];
};

// This is only used on 0x5 draw_lines commands, scaling?
struct os_anim_scale { // ID: 0x40
    float scale;
    uint32_t padding[12];
};

// No idea what this one does, but it doesn't have any params
struct os_anim_80 { // ID: 0x80
    uint32_t padding[13];
};

struct os_anim { // 64 bytes (16 x 4)
    uint32_t type;
    int32_t time;
    int32_t duration;
    union {
        struct os_anim_1 anim1; // 0x1
        struct os_anim_points points; // 0x2
        struct os_anim_rotate rotate; // 0x4
        struct os_anim_color color; // 0x8
        struct os_anim_colors colors; // 0x10
        struct os_anim_text text; // 0x20
        struct os_anim_scale scale; // 0x40
        struct os_anim_80 anim80; // 0x80
    };
};

const float original_fps = 20.0f;

char * progname;
char out_path[512];

char text_buffer[1024];

char json_buffer[1<<20]; // 1MB

void writeAmbient(struct ambient_entry * amb) {
    jwObj_array("ambient_color");
        jwArr_double(amb->amb_r);
        jwArr_double(amb->amb_g);
        jwArr_double(amb->amb_b);
    jwEnd();
    jwObj_double("ambient_energy", amb->amb_energy);
    jwObj_array("light_color");
        jwArr_double(amb->lit_r);
        jwArr_double(amb->lit_g);
        jwArr_double(amb->lit_b);
    jwEnd();
    jwObj_double("light_energy", amb->lit_energy);
    jwObj_double("yaw", amb->yaw);
    jwObj_double("pitch", amb->pitch);
}

int unpackCOC(char * path) {
    FILE * coc = fopen(path, "rb");
    if (!coc) {
        fprintf(stderr, "Failed to open: %s\n", path);
        return 1;
    }

    uint32_t file_count;
    fread(&file_count, sizeof(uint32_t), 1, coc);

    struct file_entry files[file_count];
    fread(files, sizeof(struct file_entry), file_count, coc);

    char * sep = strrchr(path, separator);
    if (sep) {
        sep[1] = '\0';
    } else {
        path[0] = '\0';
    }

    printf("Unpacking %d light file entries\n", file_count);

    for (int i = 0; i < file_count; i++) {
        struct file_entry * file = files + i;

        fseek(coc, file->offset, SEEK_SET);

        uint32_t light_count;
        fread(&light_count, sizeof(uint32_t), 1, coc);

        snprintf(out_path, sizeof(out_path), "%sPLIG_%02d.json", path, i);
        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fclose(coc);
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int j = 0; j < light_count; j++) {
            struct light_entry light;
            fread(&light, sizeof(struct light_entry), 1, coc);

            jwArr_object();

            jwObj_int("arg0", light.arg0);
            jwObj_int("arg1", light.arg1);
            jwObj_array("position");
                jwArr_double(light.x);
                jwArr_double(light.y);
                jwArr_double(light.z);
            jwEnd();
            jwObj_array("color");
                jwArr_double(light.r);
                jwArr_double(light.g);
                jwArr_double(light.b);
            jwEnd();
            jwObj_double("energy", light.energy);

            jwEnd(); // End this light object
        }

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(coc);
            fclose(outf);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    fclose(coc);
    return 0;
}

int unpackAMB(char * path) {
    FILE * amb = fopen(path, "rb");
    if (!amb) {
        fprintf(stderr, "Failed to open: %s\n", path);
        return 1;
    }

    uint32_t file_count;
    fread(&file_count, sizeof(uint32_t), 1, amb);

    struct file_entry files[file_count];
    fread(files, sizeof(struct file_entry), file_count, amb);

    char * sep = strrchr(path, separator);
    if (sep) {
        sep[1] = '\0';
    } else {
        path[0] = '\0';
    }

    printf("Unpacking %d ambient file entries\n", file_count);

    for (int i = 0; i < file_count; i++) {
        struct file_entry * file = files + i;

        fseek(amb, file->offset, SEEK_SET);

        snprintf(out_path, sizeof(out_path), "%sAMB_%02d.json", path, i);
        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fclose(amb);
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

        struct ambient_entry ambient;
        fread(&ambient, sizeof(struct ambient_entry), 1, amb);

        writeAmbient(&ambient);

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(amb);
            fclose(outf);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    fclose(amb);
    return 0;
}

int unpackCBT(char * path) {
    FILE * cbt = fopen(path, "rb");
    if (!cbt) {
        fprintf(stderr, "Failed to open: %s\n", path);
        return 1;
    }

    uint32_t file_count;
    fread(&file_count, sizeof(uint32_t), 1, cbt);

    struct file_entry files[file_count];
    fread(files, sizeof(struct file_entry), file_count, cbt);

    char * sep = strrchr(path, separator);
    if (sep) {
        sep[1] = '\0';
    } else {
        path[0] = '\0';
    }

    printf("Unpacking %d boot light file entries\n", file_count);

    for (int i = 0; i < file_count; i++) {
        struct file_entry * file = files + i;
        printf("File entry: %d, length: %d\n", i, file->length);

        // These files seem to be unused anyway
        if (file->length == 40000) {
            printf("Skipping non-standard entry\n\n");
            continue;
        }

        fseek(cbt, file->offset, SEEK_SET);

        snprintf(out_path, sizeof(out_path), "%sBTAMB_%02d.json", path, i);
        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fclose(cbt);
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        bool endReached = false;
        for (int j = 0; j < BOOT_AMBIENT_COUNT; j++) {
            struct boot_ambient_entry bootAmb;
            fread(&bootAmb, sizeof(struct boot_ambient_entry), 1, cbt); // 44

            if (bootAmb.frame < 0 || endReached) {
                endReached = true;
                continue;
            };

            jwArr_object();

            jwObj_int("frame", bootAmb.frame);
            writeAmbient(&bootAmb.ambient);

            jwEnd();
        }

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(cbt);
            fclose(outf);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);

        snprintf(out_path, sizeof(out_path), "%sBTLIG_%02d.json", path, i);
        outf = fopen(out_path, "w");
        if (!outf) {
            fclose(cbt);
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int j = 0; j < 10; j++) {
            bool isEmpty = true;
            endReached = false;
            for (int k = 0; k < BOOT_AMBIENT_COUNT; k++) {
                struct boot_plight_entry plight;
                fread(&plight, sizeof(struct boot_plight_entry), 1, cbt); // 36

                if (plight.frame < 0 || endReached ||
                  ((isnan(plight.x) || plight.x == 0.0f)
                && (isnan(plight.y) || plight.y == 0.0f)
                && (isnan(plight.z) || plight.z == 0.0f))) {
                    endReached = true;
                    continue;
                }

                if (k == 0) {
                    printf("Track %d\n", j);
                    isEmpty = false;
                    jwArr_array();
                }

                jwArr_object();
                    jwObj_int("frame", plight.frame);
                    jwObj_array("position");
                        jwArr_double(plight.x);
                        jwArr_double(plight.y);
                        jwArr_double(plight.z);
                    jwEnd();
                    jwObj_array("color");
                        jwArr_double(plight.r);
                        jwArr_double(plight.g);
                        jwArr_double(plight.b);
                    jwEnd();
                    jwObj_double("energy", plight.energy);
                jwEnd();

                printf("Frame %d, Pos (%.2f, %.2f, %.2f), Color (%.2f, %.2f, %.2f) Energy %.2f\n",
                    plight.frame, plight.x, plight.y, plight.z, plight.r, plight.g, plight.b, plight.energy);
            }

            if (!isEmpty) {
                printf("\n");
                jwEnd();
            }
        }

        jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(cbt);
            fclose(outf);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    fclose(cbt);
    return 0;
}

void write_bgra(uint32_t bgra) {
    jwArr_double((float)((bgra >> 16) & 0xFF) / (float)(0xFF)); // R
    jwArr_double((float)((bgra >> 8) & 0xFF) / (float)(0xFF)); // G
    jwArr_double((float)(bgra & 0xFF) / (float)(0xFF)); // B
    jwArr_double((float)((bgra >> 24) & 0xFF) / (float)(0xFF)); // A
}

int write_os_draw(struct os_draw * draw) {
    jwObj_int("type", draw->type);
    switch (draw->type) {
    case 0x0:
        // NULL entry, ignore it
    break;
    case 0x1:
        jwObj_double("x", draw->text.pos.x);
        jwObj_double("y", draw->text.pos.y);

        jwObj_array("color");
        write_bgra(draw->text.color);
        jwEnd();

        jwObj_int("text", draw->text.id);
        jwObj_int("arg", draw->text.arg);
    break;
    case 0x2:
        jwObj_array("points");
        for (int i = 0; i < 8; i++) {
            jwArr_double(draw->quad.points[i]);
        }
        jwEnd();

        jwObj_array("colors");
        for (int i = 0; i < 4; i++) {
            jwArr_array();
            write_bgra(draw->quad.colors[i]);
            jwEnd();
        }
        jwEnd();
    break;
    case 0x3:
        jwObj_double("x1", draw->line.start.x);
        jwObj_double("y1", draw->line.start.y);
        jwObj_double("x2", draw->line.end.x);
        jwObj_double("y2", draw->line.end.y);

        jwObj_array("color");
        write_bgra(draw->line.color);
        jwEnd();
    break;
    case 0x4:
        jwObj_int("sprite", draw->sprite.id);
        jwObj_double("x1", draw->sprite.start.x);
        jwObj_double("y1", draw->sprite.start.y);
        jwObj_double("x2", draw->sprite.end.x);
        jwObj_double("y2", draw->sprite.end.y);

        jwObj_array("color");
        write_bgra(draw->sprite.color);
        jwEnd();
    break;
    case 0x5:
        jwObj_int("lines", draw->lines.id);
        jwObj_double("x", draw->lines.pos.x);
        jwObj_double("y", draw->lines.pos.y);

        jwObj_double("scale", draw->lines.scale);

        jwObj_array("color");
        write_bgra(draw->lines.color);
        jwEnd();
    break;
    default:
        fprintf(stderr, "Unknown os_draw type %d\n", draw->type);
        return 1;
    }

    return 0;
}

int write_os_anim(struct os_anim * anim) {
    jwObj_int("type", anim->type);
    jwObj_double("time", (float)(anim->time) / original_fps);
    jwObj_double("duration", (float)(anim->duration) / original_fps);
    switch (anim->type) {
    case 0x0:
    case 0x1:
    case 0x20:
    case 0x80:
        // NULL entries, ignore them
    break;
    case 0x2:
        jwObj_array("points");
        for (int i = 0; i < 8; i++) {
            jwArr_double(anim->points.points[i]);
        }
        jwEnd();
    break;
    case 0x4:
        jwObj_int("flag", anim->rotate.flag);
    break;
    case 0x8:
        jwObj_array("color");
        write_bgra(anim->color.color);
        jwEnd();
    break;
    case 0x10:
        jwObj_array("colors");
        for (int i = 0; i < 4; i++) {
            jwArr_array();
            write_bgra(anim->colors.colors[i]);
            jwEnd();
        }
        jwEnd();
    break;
    case 0x40:
        jwObj_double("scale", anim->scale.scale);
    break;
    default:
        printf("Unknown os_anim type %d\n", anim->type);
        return 1;
    }

    return 0;
}

int unpackOS(char * path) {
    if (sizeof(struct os_draw) != 64) {
        fprintf(stderr, "OS Draw struct was %ld bytes not 64\n", sizeof(struct os_draw));
        return 1;
    }

    if (sizeof(struct os_anim) != 64) {
        fprintf(stderr, "OS Anim struct was %ld bytes not 64\n", sizeof(struct os_anim));
        return 1;
    }

    FILE * os = fopen(path, "rb");
    if (!os) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        return 1;
    }

    char * ext = strrchr(path, separator);
    if (ext) ext[1] = '\0';
    else *path = '\0';

    uint32_t file_count;
    fread(&file_count, sizeof(uint32_t), 1, os);

    printf("Converting %d OS files\n", file_count);

    struct file_entry files[file_count];
    fread(files, sizeof(struct file_entry), file_count, os);

    for (int f = 0; f < file_count; f++) {
        fseek(os, files[f].offset, SEEK_SET);

        if ((files[f].length - 4) % sizeof(struct os_draw)) {
            fprintf(stderr, "File length %d not evenly divisible by OS entry size %ld\n",
                files[f].length - 4, sizeof(struct os_draw));
            fclose(os);
            return 1;
        }

        jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

        uint32_t duration;
        fread(&duration, sizeof(uint32_t), 1, os);

        jwObj_double("duration", (float)(duration) / original_fps);

        jwObj_array("elements");
        for (int i = 0; i < 112; i++) {
            struct os_draw draw;
            fseek(os, files[f].offset + 4 + sizeof(struct os_draw) * i, SEEK_SET);
            if (fread(&draw, sizeof(struct os_draw), 1, os) != 1) {
                fprintf(stderr, "Failed to get draw entry %d from file %d\n", i, f);
                fclose(os);
                return 1;
            }

            if (draw.anims < 0) {
                continue;
            }

            jwArr_object();

            write_os_draw(&draw);

            fseek(os, files[f].offset + 4 + (sizeof(struct os_draw) * 112) + (sizeof(struct os_anim) * 16) * i, SEEK_SET);

            jwObj_array("anims");
            int anim_count = draw.anims < 16 ? draw.anims : 16;
            for (int j = 0; j < anim_count; j++) {
                struct os_anim anim;
                if (fread(&anim, sizeof(struct os_anim), 1, os) != 1) {
                    fprintf(stderr, "Failed to get animation entry %d:%d from file %d\n", i, j, f);
                    fclose(os);
                    return 1;
                }

                if (!anim.type || anim.time < 0) {
                    continue;
                }

                jwArr_object();

                write_os_anim(&anim);

                jwEnd();
            }
            jwEnd();

            jwEnd();
        }
        jwEnd();

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(os);
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%sos_%02d.json", path, f);
        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    fclose(os);
    return 0;
}

int write_os_sprite2(struct os_sprite2 * sprite) {
    jwObj_int("ui", sprite->ui_idx);
    jwObj_int("sprite", sprite->frame_idx);
    jwObj_double("x", sprite->pos.x);
    jwObj_double("y", sprite->pos.y);
    jwObj_double("origin_x", sprite->origin.x);
    jwObj_double("origin_y", sprite->origin.y);
    jwObj_double("w", sprite->size.x);
    jwObj_double("h", sprite->size.y);
    jwObj_double("rotation", sprite->rotation);
    jwObj_double("scale", sprite->scale);
    jwObj_double("fade", sprite->fade);
    jwObj_double("fade_add", sprite->fade_add);
    jwObj_double("fade_sub", sprite->fade_sub);

    jwObj_int("color_ptr", sprite->color_ptr);
    jwObj_int("fade_target_ptr", sprite->fade_target_ptr);

    jwObj_int("fade_target_value", sprite->fade_target_value);

    return 0;
}

int unpackDATA(char * path) {
    FILE * datf = fopen(path, "rb");
    if (!datf) {
        fprintf(stderr, "Failed to open file: %s\n", path);
        return 1;
    }

    char * ext = strrchr(path, separator);
    if (ext) ext[1] = '\0';
    else *path = '\0';

    strcpy(out_path, path);
    strcat(out_path, ".data.hdr");

    FILE * hdrf = fopen(out_path, "rb");
    if (!hdrf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    struct section_header hdr_data;
    fread(&hdr_data, sizeof(struct section_header), 1, hdrf);

    fclose(hdrf);

    printf("Unpacking OS lines from .data segment\n");

    struct os_lines_pointer lines_ptrs[38];
    const uint32_t lines_addrs[] = {0x33600, 0x33CB0, 0x34588}; // Sizes: 6, 7, 7

    for (int i = 0; i < 3; i++) {
        int lines_size = i ? 7 : 6;
        fseek(datf, lines_addrs[i], SEEK_SET);
        fread(lines_ptrs, sizeof(struct os_lines_pointer), lines_size, datf);

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int j = 0; j < lines_size; j++) {
            fseek(datf, lines_ptrs[j].offset - hdr_data.vaddr, SEEK_SET);
            jwArr_array();
            for (int k = 0; k < lines_ptrs[j].count * 4; k++) {
                float val;
                fread(&val, sizeof(float), 1, datf);
                jwArr_double(val);
            }
            jwEnd();
        }

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(datf);
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%sos_lines_%d.json", path, i);

        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    fseek(datf, 0x451B8, SEEK_SET);
    fread(lines_ptrs, sizeof(struct os_lines_pointer), 38, datf);

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    for (int i = 0; i < 38; i++) {
        fseek(datf, lines_ptrs[i].offset - hdr_data.vaddr, SEEK_SET);
        jwArr_array();
        for (int j = 0; j < lines_ptrs[i].count * 4; j++) {
            float val;
            fread(&val, sizeof(float), 1, datf);
            jwArr_double(val);
        }
        jwEnd();
    }

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strcpy(out_path, path);
    strcat(out_path, "os_lines.json");

    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    printf("Unpacking HUD lines from .data segment\n");

    for (int c = 0; c < 6; c++) {
        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int i = 0; i < 74; i++) {
            fseek(datf, 0x2D5F0 + sizeof(struct os_lines_pointer) * (c * 74 + i), SEEK_SET);
            struct os_lines_pointer lines_ptr;
            fread(&lines_ptr, sizeof(struct os_lines_pointer), 1, datf);

            fseek(datf, lines_ptr.offset - hdr_data.vaddr, SEEK_SET);

            jwArr_array();
            for (int j = 0; j < lines_ptr.count * 4; j++) {
                float val;
                fread(&val, sizeof(float), 1, datf);
                jwArr_double(val);
            }
            jwEnd();
        }

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(datf);
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%shud_lines_%d.json", path, c);

        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    printf("Unpacking OS sprites from .data segment\n");

    struct os_sprite2 os_sprites[30];

    uint32_t os_sprites_addrs[] = {0x37310, 0x376D0, 0x37AD0};

    for (int gen = 0; gen < 3; gen++) {
        fseek(datf, os_sprites_addrs[gen], SEEK_SET);

        int32_t os_sprites_count = gen == 1 ? 16 : 15;
        fread(os_sprites, sizeof(struct os_sprite2), os_sprites_count, datf);

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int i = 0; i < os_sprites_count; i++) {
            jwArr_object();

            write_os_sprite2(os_sprites + i);

            jwEnd();
        }

        jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%sos_sprites_%d.json", path, gen);

        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    printf("Unpacking HUD sprites from .data segment\n");

    for (int c = 0; c < 6; c++) {
        fseek(datf, 0x2E980 + sizeof(struct os_sprite2) * 30 * c, SEEK_SET);
        fread(os_sprites, sizeof(struct os_sprite2), 30, datf);

        jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

        for (int i = 0; i < 30; i++) {
            jwArr_object();

            write_os_sprite2(os_sprites + i);

            jwEnd();
        }

        jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%shud_sprites_%d.json", path, c);

        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    printf("Unpacking UI sprites from .data segment\n");

    struct file_entry ui_sprite_ptrs[8] = {
        {0x2E760, 8}, // Multi-monitor status sprites
        {0x2E908, 5}, // Multi-monitor systems sprites
        {0x2E700, 4}, // Multi-monitor small-map sprites
        {0x2E4C0, 24}, // Multi-monitor full-map sprites

        {0x2E8A8, 3}, // BR match status sprites
        {0x2E830, 5}, // CQ match status sprites
        {0x321B8, 28}, // Menu map (grid marker) sprites
        {0x32184, 2} // Faction flag sprites (not sure where this is used?)
    };

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    for (int i = 0; i < 8; i++) {
        fseek(datf, ui_sprite_ptrs[i].offset, SEEK_SET);

        jwArr_array();
        for (int j = 0; j < ui_sprite_ptrs[i].length; j++) {
            struct os_sprite sprite;
            fread(&sprite, sizeof(struct os_sprite), 1, datf);

            jwArr_object();

            jwObj_int("sprite", sprite.sprite_idx);
            jwObj_double("x", sprite.pos.x);
            jwObj_double("y", sprite.pos.y);
            jwObj_double("w", sprite.size.x);
            jwObj_double("h", sprite.size.y);
            jwObj_array("color");
            write_bgra(sprite.color);
            jwEnd();

            jwEnd();
        }
        jwEnd();
    }

    jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strcpy(out_path, path);
    strcat(out_path, "ui_sprites.json");

    outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    printf("Extracting pilot camera poses\n");

    jwOpen(json_buffer, sizeof(json_buffer), JW_ARRAY, JW_PRETTY);

    fseek(datf, 0x105D0, SEEK_SET);

    for (int c = 0; c < 6; c++) {
        jwArr_array();
        for (int i = 0; i < 7; i++) {
            jwArr_object();

            struct vector3 pos, dir;
            fread(&pos, sizeof(struct vector3), 1, datf);
            fread(&dir, sizeof(struct vector3), 1, datf);

            jwObj_array("position");
            jwArr_double(pos.x);
            jwArr_double(pos.y);
            jwArr_double(pos.z);
            jwEnd();

            jwObj_array("rotation");
            jwArr_double(dir.x);
            jwArr_double(dir.y);
            jwArr_double(dir.z);
            jwEnd();

            jwEnd();
        }
        jwEnd();
    }

    jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strcpy(out_path, path);
    strcat(out_path, "pilot_poses.json");

    outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    fclose(datf);
    return 0;
}

int unpackTEXT(char * path) {
    FILE * txtf = fopen(path, "rb");
    if (!txtf) {
        fprintf(stderr, "Failed to open file: %s\n", out_path);
        return 1;
    }

    char * ext = strrchr(path, separator);
    if (ext) ext[1] = '\0';
    else *path = '\0';

    printf("Unpacking OS data from .text segment\n");

    // Data layout for this is fucked. Piecemeal it together
    fseek(txtf, 0x616CE, SEEK_SET);

    float switch_quads[160];
    for (int i = 0; i < 160; i++) {
        if (i == 0x60) fseek(txtf, 0x61ADB, SEEK_SET);
        fread(switch_quads + i, sizeof(float), 1, txtf);
        fseek(txtf, i >= 23 ? 7 : 4, SEEK_CUR);
    }

    fseek(txtf, 0x61AA3, SEEK_SET);

    int switch_text_pos[30];
    fread(switch_text_pos, sizeof(int32_t), 1, txtf);
    switch_text_pos[2] = switch_text_pos[0];
    switch_text_pos[4] = switch_text_pos[0];
    switch_text_pos[6] = switch_text_pos[0];
    switch_text_pos[8] = switch_text_pos[0];

    fseek(txtf, 0x61D9B, SEEK_SET);
    for (int i = 1; i < 10; i += 2) {
        fread(switch_text_pos + i, sizeof(int32_t), 1, txtf);
        fseek(txtf, 7, SEEK_CUR);
    }

    fseek(txtf, 0x61ACB, SEEK_SET);
    fread(switch_text_pos + 10, sizeof(int32_t), 1, txtf);

    fseek(txtf, 0x61DD9, SEEK_SET);
    fread(switch_text_pos + 11, sizeof(int32_t), 1, txtf);

    fseek(txtf, 0x61AD0, SEEK_SET);
    fread(switch_text_pos + 12, sizeof(int32_t), 1, txtf);

    fseek(txtf, 0x61DEB, SEEK_SET);
    fread(switch_text_pos + 13, sizeof(int32_t), 1, txtf);

    fseek(txtf, 0x61E16, SEEK_SET);
    fread(switch_text_pos + 14, sizeof(int32_t), 1, txtf);

    fseek(txtf, 0x61E21, SEEK_SET);
    fread(switch_text_pos + 15, sizeof(int32_t), 1, txtf);

    switch_text_pos[16] = switch_text_pos[10];

    fseek(txtf, 0x61DF7, SEEK_SET);
    fread(switch_text_pos + 17, sizeof(int32_t), 1, txtf);

    switch_text_pos[18] = switch_text_pos[12];
    switch_text_pos[20] = switch_text_pos[17];

    fseek(txtf, 0x61E33, SEEK_SET);
    for (int i = 19; i < 30; i++) {
        if (i == 20) continue;
        fread(switch_text_pos + i, sizeof(int32_t), 1, txtf);
        fseek(txtf, 7, SEEK_CUR);
    }

    for (int gen = 0; gen < 3; gen++) {
        jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

        fseek(txtf, 0x60788 + (8 * 10 * gen), SEEK_SET);

        jwObj_array("start_bar_pos");
        for (int i = 0; i < 10; i++) {
            float val;
            fread(&val, sizeof(float), 1, txtf);
            jwArr_double(val);

            // For whatever reason the instruction format changes for gen 2
            fseek(txtf, gen == 2 ? 7 : 4, SEEK_CUR); // Skip instruction bytes
        }
        jwEnd();

        fseek(txtf, 0x6143D + (11 * 20 * gen), SEEK_SET);

        jwObj_array("switch_bar_pos");
        for (int i = 0; i < 20; i++) {
            float val;
            fread(&val, sizeof(float), 1, txtf);
            jwArr_double(val);

            fseek(txtf, 7, SEEK_CUR); // Skip instruction bytes
        }
        jwEnd();

        jwObj_array("switch_quads");
        if (gen == 2) {
            for (int i = 80; i < 160; i++) {
                jwArr_double(switch_quads[i]);
            }
        } else {
            for (int i = 0; i < 40; i++) {
                jwArr_double(switch_quads[i + (40 * gen)]);
            }
        }
        jwEnd();

        jwObj_array("switch_text_pos");
        for (int i = 0; i < 10; i++) {
            jwArr_double(switch_text_pos[i + (10 * gen)]);
        }
        jwEnd();

        int jw_err = jwClose();
        if (jw_err) {
            fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
            fclose(txtf);
            return 1;
        }

        snprintf(out_path, sizeof(out_path), "%sos_data_%d.json", path, gen);

        FILE * outf = fopen(out_path, "w");
        if (!outf) {
            fprintf(stderr, "Failed to open output file: %s\n", out_path);
            fclose(txtf);
            return 1;
        }

        fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
        fclose(outf);
    }

    printf("Unpacking HUD colors from .text segment\n");

    uint32_t color_addrs[] = {0x27B5A, 0x27B91, 0x27BC8, 0x27BFC, 0x27C30};

    jwOpen(json_buffer, sizeof(json_buffer), JW_OBJECT, JW_PRETTY);

    jwObj_array("pointers");
    fseek(txtf, 0x27B56, SEEK_SET);
    for (int i = 0; i < 5; i++) {
        uint32_t ptr;
        fread(&ptr, sizeof(uint32_t), 1, txtf);
        jwArr_int(ptr);
        printf("Pointer: %08X\n", ptr);

        fseek(txtf, 6, SEEK_CUR);
    }
    fseek(txtf, 0x27C8E, SEEK_SET);
    {
        uint32_t ptr;
        fread(&ptr, sizeof(uint32_t), 1, txtf);
        jwArr_int(ptr);
        printf("Pointer: %08X\n", ptr);
    }
    jwEnd();

    jwObj_array("palettes");
    for (int i = 0; i < 5; i++) {
        fseek(txtf, color_addrs[i], SEEK_SET);

        jwArr_array();
        for (int j = 0; j < 5; j++) {
            uint32_t color;
            fread(&color, sizeof(uint32_t), 1, txtf);
            jwArr_array();
            write_bgra(color);
            jwEnd();
            fseek(txtf, 6, SEEK_CUR);
        }
        jwEnd();
    }
    jwEnd();

    int jw_err = jwClose();
    if (jw_err) {
        fprintf(stderr, "JSON writer error: %s\n", jwErrorToString(jw_err));
        return 1;
    }

    strcpy(out_path, path);
    strcat(out_path, "hud_colors.json");

    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }

    fwrite(json_buffer, sizeof(char), strlen(json_buffer), outf);
    fclose(outf);

    fclose(txtf);
    return 0;
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Cockpit Tool - By QuantX\n");

    if (!argc) {
        fprintf(stderr, "Please specify a cockpit file: %s <path/cockpit_file>\n", progname);
        return 1;
    }

    char * path = *argv++; argc--;

    char * ext = strrchr(path, '.');
    if (!ext) {
        fprintf(stderr, "No extension present on path: %s\n", path);
        return 1;
    }

    ext++;
    if (!strcmp(ext, "coc")) return unpackCOC(path);
    else if (!strcmp(ext, "amb")) return unpackAMB(path);
    else if (!strcmp(ext, "cbt")) return unpackCBT(path);
    else if (!strcmp(ext, "os")) return unpackOS(path);
    else if (!strcmp(ext, "seg")) {
        char * sep = strrchr(path, separator);
        if (sep) sep++;
        else sep = path;

        if (!strncmp(sep, ".data.", 6)) return unpackDATA(path);
        else if (!strncmp(sep, ".text.", 6)) return unpackTEXT(path);
        else {
            fprintf(stderr, "Unknown segment file: %s\n", sep);
            return 1;
        }
    } else {
        fprintf(stderr, "Unknown file extension: %s\n", path);
        return 1;
    }
    return 0;
}
