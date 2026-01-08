#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdbool.h>
#include <math.h>

#define CGLTF_IMPLEMENTATION
#include "cgltf.h"

#define HEADER_SIZE 16

struct vector3 {
    float x, y, z;
} __attribute__((__packed__));

struct atari_header {
    int32_t center[3]; // Center of AABB
    uint16_t extents[3]; // Extents of AABB
    uint16_t sub_header_count;
    uint16_t data_count;
    uint16_t zero;
    uint32_t next_header_offset;
    uint32_t data_index;
} __attribute__((__packed__));

#define ATARI_DATA_COUNT_MAX 4000
struct atari_data {
    int32_t val;
    struct vector3 verts[4];
    uint32_t zero0;
    uint64_t flags;
    struct vector3 norm;
    uint32_t zero1;
} __attribute__((__packed__)) atari_data_list[ATARI_DATA_COUNT_MAX];
size_t atari_data_count = 0;

char * progname;
char out_path[256];
char * ppd_path;
char * gltf_path;

const float scale_factor = 100.0f;

static inline size_t write_vec3(struct vector3 v, FILE * f) {
    return fwrite(&v, sizeof(struct vector3), 1, f);
}

int process_header(FILE * ppd, unsigned int level) {
    while (true) {
        size_t header_pos = ftell(ppd);
        
        struct atari_header header;
        fread(&header, sizeof(struct atari_header), 1, ppd);
        
        struct vector3 center = {header.center[0], header.center[1], header.center[2]};
        center.x /= scale_factor;
        center.y /= scale_factor;
        center.z /= scale_factor;
        
        struct vector3 extents = {header.extents[0], header.extents[1], header.extents[2]};
        extents.x /= scale_factor;
        extents.y /= scale_factor;
        extents.z /= scale_factor;
        
        printf("%*sAABB-START (%f, %f, %f)\n", level, "",
            center.x - extents.x, center.y - extents.y, center.z - extents.z);
        printf("%*sAABB-END   (%f, %f, %f)\n", level, "",
            center.x + extents.x, center.y + extents.y, center.z + extents.z);
        
        printf("%*sSub Header Count %u\n", level, "", header.sub_header_count);
        printf("%*sNext Header Offset %u\n", level, "", header.next_header_offset);
        printf("%*sData Index %u\n", level, "", header.data_index);
        printf("%*sData Count %u\n", level, "", header.data_count);
        
        if (header.zero) {
            fprintf(stderr, "Header entry \"zero\" was non-zero: %08X\n", header.zero);
            return 1;
        }
        
        if (header.sub_header_count) {
            if (process_header(ppd, level + 2)) return 1;
        } else {
            if (atari_data_count + header.data_count >= ATARI_DATA_COUNT_MAX) {
                fprintf(stderr, "Atari data overflow\n");
                return 1;
            }
            
            struct atari_data * data = atari_data_list + atari_data_count;
            
            size_t r = fread(data, sizeof(struct atari_data), header.data_count, ppd);
            if (r != header.data_count) {
                fprintf(stderr, "Failed to read all data entries, got %lu of %u\n", r, header.data_count);
                return 1;
            }
            
            atari_data_count += r;
            
            for (int di = 0; di < r; di++) {
                for (int i = 0; i < 4; i++) {
                    data[di].verts[i].x /= scale_factor;
                    data[di].verts[i].y /= scale_factor;
                    data[di].verts[i].z /= scale_factor;
                    
                    printf("%*sPOS%d (%f, %f, %f)\n", level + 2, "",
                        i, data[di].verts[i].x, data[di].verts[i].y, data[di].verts[i].z);
                }
                
                printf("%*sFlags %16lX\n", level + 2, "", data[di].flags);
                
                printf("%*sNORM (%f, %f, %f)\n", level + 2, "",
                    data[di].norm.x, data[di].norm.y, data[di].norm.z);
                
                if (data[di].val != -1) {
                    fprintf(stderr, "Data entry \"val\" was not -1: %d\n", data[di].val);
                    return 1;
                }
                
                if (data[di].zero0) {
                    fprintf(stderr, "Data entry \"zero0\" was non-zero: %08X\n", data[di].zero0);
                    return 1;
                }
                
                if (data[di].zero1) {
                    fprintf(stderr, "Data entry \"zero1\" was non-zero: %08X\n", data[di].zero1);
                    return 1;
                }
            }
        }

        if (!header.next_header_offset) return 0;
        fseek(ppd, header_pos + header.next_header_offset, SEEK_SET);
    }
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Hitbox Tool - By QuantX\n");

    if (argc < 2) {
        fprintf(stderr, "Please specify a PPD file: %s <path/example.ppd> <path/example.gltf>\n", progname);
        return 1;
    }
    
    ppd_path = *argv++; argc--;
    gltf_path = *argv++; argc--;
    
    FILE * ppd = fopen(ppd_path, "rb");
    if (!ppd) {
        fprintf(stderr, "Failed to open PPD file: %s\n", ppd_path);
        return 1;
    }
    
    cgltf_options options = {0};
    cgltf_data * data = NULL;
    cgltf_result result = cgltf_parse_file(&options, gltf_path, &data);
    if (result != cgltf_result_success) {
        fclose(ppd);
        fprintf(stderr, "Failed to open glTF file: %s\n", gltf_path);
        return 1;
    }

    uint32_t margin;
    fread(&margin, sizeof(uint32_t), 1, ppd);
    float margin_scaled = (float)(margin) / scale_factor;
    printf("Margin: %d, %f\n", margin, margin_scaled);
    
    uint32_t part_count;
    fread(&part_count, sizeof(uint32_t), 1, ppd);

    printf("Processing %d hitbox parts\n", part_count);

    uint32_t zero;
    fread(&zero, sizeof(uint32_t), 1, ppd);
    if (zero) printf("Non-zero value at 0x8: %08X\n", zero);
    
    fread(&zero, sizeof(uint32_t), 1, ppd);
    if (zero) printf("Non-zero value at 0x12: %08X\n", zero);
    
    uint32_t part_offsets[part_count];
    fread(part_offsets, sizeof(uint32_t), part_count, ppd);
    
    uint32_t bone_offset;
    fread(&bone_offset, sizeof(uint32_t), 1, ppd);
    
    strncpy(out_path, ppd_path, sizeof(out_path));

    char * ext = strrchr(out_path, '.');
    if (!ext) {
        fprintf(stderr, "File path is missing extension: %s\n", ppd_path);
        return 1;
    }
    strcpy(ext + 1, "hbx");
    
    FILE * outf = fopen(out_path, "wb");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", out_path);
        return 1;
    }
    
    fwrite(&part_count, sizeof(uint32_t), 1, outf);
    
    for (int p = 0; p < part_count; p++) {
        fseek(ppd, HEADER_SIZE + part_offsets[p], SEEK_SET);
        
        printf("*** Processing part %d ***\n", p);
        
        atari_data_count = 0;
        if (process_header(ppd, 0)) return 1;
        
        printf("*** Processed %lu data entries ***\n", atari_data_count);
        
        fwrite(&atari_data_count, sizeof(uint32_t), 1, outf);
        
        // Output all triangles
        for (int di = 0; di < atari_data_count; di++) {
            struct vector3 * verts = atari_data_list[di].verts;
            // Triangle 2 (First Half of Quad)
            // Reverse the winding order 1 -> 0 -> 2
            write_vec3(verts[1], outf);
            write_vec3(verts[0], outf);
            write_vec3(verts[2], outf);
            // Triangle 2 (Second Half of Quad)
            write_vec3(verts[3], outf);
            write_vec3(verts[1], outf);
            write_vec3(verts[2], outf);
        }
        
        // Output all flags
        for (int di = 0; di < atari_data_count; di++) {
            fwrite(&atari_data_list[di].flags, sizeof(uint64_t), 1, outf);
        }
    }
    
    if (bone_offset) {
        fseek(ppd, HEADER_SIZE + bone_offset, SEEK_SET);
        
        uint8_t bone_count, bone_parts;
        fread(&bone_count, sizeof(uint8_t), 1, ppd); // Expected number of bones in the XBO/GLTF
        fread(&bone_parts, sizeof(uint8_t), 1, ppd); // Should be identical to part_count
        
        printf("Reading data for %d bones, %d parts\n", bone_count, bone_parts);
        
        if (bone_parts != part_count) {
            printf("Wrong number of parts in bone data: %d != %d\n", bone_parts, part_count);
        }
        
        fwrite(&bone_parts, sizeof(uint8_t), 1, outf);

        if (!data->skins_count) {
            fprintf(stderr, "GLTF does not contain any skins");
            return 1;        
        }
        
        cgltf_skin * skin = data->skins;
        if (skin->joints_count < bone_count) {
            fprintf(stderr, "GLTF has %ld bones, not %d\n", skin->joints_count, bone_count);
            return 1;
        }
        
        int valid_bones = 0;
        for (int i = 0; i < bone_count; i++) {
            uint8_t bone_id, part_id;
            fread(&bone_id, sizeof(uint8_t), 1, ppd);
            fread(&part_id, sizeof(uint8_t), 1, ppd);

            printf("Index %d, Bone ID %d, Part ID %d\n", i, bone_id, part_id);
            
            if (part_id == 99) continue;

            // There are technically "bone_count + 1" bones including the root bone
            if (bone_id > bone_count) {
                fprintf(stderr, "Bone ID %d exceeded bone count of %d\n", bone_id, bone_count);
                return 1;
            }
            
            if (part_id >= bone_parts) {
                fprintf(stderr, "Part ID %d exceeded part count of %d\n", part_id, bone_parts);
                return 1;
            }

            cgltf_node * bone = skin->joints[bone_id];
            
            uint32_t name_len = strlen(bone->name);
            
            fwrite(&part_id, sizeof(uint8_t), 1, outf);
            fwrite(&name_len, sizeof(uint32_t), 1, outf);
            fwrite(bone->name, sizeof(char), name_len, outf);
            
            valid_bones++;
        }
        
        if (valid_bones != bone_parts) {
            fprintf(stderr, "Bone part count missmatch %d != %d\n", valid_bones, bone_parts);
            return 1;
        }
    } else {
        printf("No bone definitions\n");
        uint8_t no_bones = 0;
        fwrite(&no_bones, sizeof(uint8_t), 1, outf);
    }
    
    cgltf_free(data);
    fclose(ppd);
    fclose(outf);

    return 0;
}
