#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdbool.h>
#include <math.h>

#define CGLTF_IMPLEMENTATION
#define CGLTF_WRITE_IMPLEMENTATION
#define CGLTF_VALIDATE_ENABLE_ASSERTS 1
#include "cgltf_write.h"

#ifdef __linux__
#define SEPARATOR '/'
#else
#define SEPARATOR '\\'
#endif

#define MODEL_HEADER_OFFSET 0x50
#define MODEL_HEADER_SIZE 16

#define FORMAT_XBO  0x152
#define FORMAT_XBO2 0x52
#define FORMAT_SHA  0x12

#define CHECK_BIT(mask,idx) ((mask)&(1<<(idx)))

char path[256];

const unsigned int verts_stride = 8 * sizeof(float) + 12 * sizeof(uint8_t);
const float scale_factor = 0.01f;

bool flip_faces = false;

const cgltf_primitive_type prim_type = cgltf_primitive_type_triangle_strip;

struct buffer {
    uint32_t offset; // Offset in bytes
    uint32_t size;   // Size in bytes
};

struct vector2 {
    float x, y;
};

struct vector3 {
    float x, y, z;
};

struct primative {
    uint32_t offset;
    uint32_t size;
    uint32_t count;

    struct vector3 min, max;

    struct vector3 pos, dir;
};

char * progname;

void euler2quat(float quat[4], struct vector3 euler) {
    // Conver from Euler to Quaternion
    double cx = cos(euler.x * 0.5);
    double sx = sin(euler.x * 0.5);
    double cy = cos(euler.y * 0.5);
    double sy = sin(euler.y * 0.5);
    double cz = cos(euler.z * 0.5);
    double sz = sin(euler.z * 0.5);

    quat[0] = sx * cy * cz - cx * sy * sz; // X
    quat[1] = cx * sy * cz + sx * cy * sz; // Y
    quat[2] = cx * cy * sz - sx * sy * cz; // Z
    quat[3] = cx * cy * cz + sx * sy * sz; // W
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Model Tool - By QuantX\n");

    if (!argc) {
        fprintf(stderr, "Please specify an XBO model file: %s <path/example.xbo> (--flip)\n", progname);
        return 1;
    }

    strncpy(path, *argv, sizeof(path));
    argv++; argc--;

    if (argc && !strcmp(*argv, "--flip")) {
        flip_faces = true;
        argv++; argc--;
    }

    FILE * sbmdl = fopen(path, "rb");
    if (!sbmdl) {
        fprintf(stderr, "Failed to open XBO file: %s\n", path);
        return 1;
    }

    char model_name[64];
    char * sep = strrchr(path, SEPARATOR);
    if (sep) {
        strncpy(model_name, sep + 1, sizeof(model_name));
    } else {
        strncpy(model_name, path, sizeof(model_name));
    }
    *strrchr(model_name, '.') = '\0';

    // Get position of index table
    fseek(sbmdl, 8, SEEK_SET);

    struct buffer nodespecial_section;
    fread(&nodespecial_section, sizeof(struct buffer), 1, sbmdl);

    struct buffer nodemirror_section;
    fread(&nodemirror_section, sizeof(struct buffer), 1, sbmdl);

    struct buffer index_section;
    fread(&index_section, sizeof(struct buffer), 1, sbmdl);

    // Read vertex header
    fseek(sbmdl, MODEL_HEADER_OFFSET, SEEK_SET);
    uint8_t node_count, mesh_attr;
    fread(&node_count, sizeof(uint8_t), 1, sbmdl);
    fread(&mesh_attr, sizeof(uint8_t), 1, sbmdl);

    uint8_t mesh_count = node_count - 1;

    uint16_t header_size;
    fread(&header_size, sizeof(uint16_t), 1, sbmdl);

    uint32_t extra;
    fread(&extra, sizeof(uint32_t), 1, sbmdl);
    if (extra != 0) printf("Extra data is present at %08X + MODEL_HEADER_OFFSET = %08X\n", extra, extra + MODEL_HEADER_OFFSET);

    // Check the magic numbers
    uint32_t magic;
    fread(&magic, sizeof(uint32_t), 1, sbmdl);
    if (magic != 8) {
        fprintf(stderr, "Magic 0x58 was %X instead of 0x08, this is probably not an SB model\n", magic);
        fclose(sbmdl);
        return 1;
    }
    fread(&magic, sizeof(uint32_t), 1, sbmdl);
    if (magic != 0x18) {
        fprintf(stderr, "Magic 0x5C was %X instead of 0x18, this is probably not an SB model\n", magic);
        fclose(sbmdl);
        return 1;
    }

    printf("Processing %d meshes, %d attributes\n", mesh_count, mesh_attr);

    // Read vertex properties
    struct buffer * verts_in = malloc(mesh_count * sizeof(struct buffer));
    fread(verts_in, sizeof(struct buffer), mesh_count, sbmdl);

    //printf("Mesh vertex offset last %08X %08X\n", meshes[mesh_count - 1].offset, meshes[mesh_count - 1].size);

    // Read nodes
    uint8_t * node_parents = malloc(node_count * sizeof(uint8_t));
    fread(node_parents, sizeof(uint8_t), node_count, sbmdl);

    struct vector3 global_pos;
    fread(&global_pos, sizeof(struct vector3), 1, sbmdl);
    struct vector3 global_dir;
    fread(&global_dir, sizeof(struct vector3), 1, sbmdl);

    global_pos.x *= scale_factor;
    global_pos.y *= scale_factor;
    global_pos.z *= scale_factor;

    printf("Global POS(%f, %f, %f), DIR(%f, %f, %f)\n",
        global_pos.x, global_pos.y, global_pos.z,
        global_dir.x, global_dir.y, global_dir.z);

    // Replace file extension
    char * ext = strrchr(path, '.') + 1;
    strcpy(ext, "glbin");

    FILE * outf = fopen(path, "wb");
    if (!outf) {
        fprintf(stderr, "Failed to open output file: %s\n", path);
        fclose(sbmdl);
        return 1;
    };

    struct primative * verts_out = malloc(mesh_count * sizeof(struct primative));

    // Mesh data
    for (uint8_t mi = 0; mi < mesh_count; mi++) {
        //fseek(sbmdl, MODEL_HEADER_OFFSET + verts_in[mi].offset, SEEK_SET);

        verts_out[mi].offset = mi ? verts_out[mi - 1].offset + verts_out[mi - 1].size : 0;

        struct vector3 pos;
        fread(&pos, sizeof(struct vector3), 1, sbmdl);
        struct vector3 dir;
        fread(&dir, sizeof(struct vector3), 1, sbmdl);

        printf("Mesh %d POS(%f, %f, %f), DIR(%f, %f, %f)\n",
            mi,
            pos.x, pos.y, pos.z,
            dir.x, dir.y, dir.z);

        // Validate mesh format
        int32_t format;
        fread(&format, sizeof(uint32_t), 1, sbmdl);
        if (format != FORMAT_XBO && format != FORMAT_XBO2) {// && format != FORMAT_SHA) {
            fprintf(stderr, "Mesh %d, unknown format %08X at position %08lX\n", mi, format, ftell(sbmdl) - 4);
            return 1;
        }

        // Validate mesh size
        int32_t mesh_size;
        fread(&mesh_size, sizeof(uint32_t), 1, sbmdl);

        if (mesh_size != verts_in[mi].size - 48) {
            fprintf(stderr, "mesh_size mismatch, %d %d\n", mesh_size, verts_in[mi].size - 48);
            return 1;
        }

        // Valide mesh magic
        fread(&magic, sizeof(uint32_t), 1, sbmdl);
        if (magic != 6) {
            fprintf(stderr, "Mesh %d magic2 was %08X instead of 0x6\n", mi, magic);
            return 1;
        }

        uint16_t mesh_header_size;
        fread(&mesh_header_size, sizeof(uint16_t), 1, sbmdl);
        if (mesh_header_size != 48) {
            fprintf(stderr, "Expected mesh header size of 48, got: %u\n", mesh_header_size);
            return 1;
        }

        // triangle_count is functionally the number of indices + 2 due to the nature of triangle strips
        uint16_t triangle_count;
        fread(&triangle_count, sizeof(int16_t), 1, sbmdl);

        uint16_t vert_count;
        fread(&vert_count, sizeof(uint16_t), 1, sbmdl);

        uint8_t byteAttrs[2];
        fread(byteAttrs, sizeof(uint8_t), 2, sbmdl);

        printf("Mesh attributes: %u, %u\n", byteAttrs[0], byteAttrs[1]);

        uint8_t vert_size;
        fread(&vert_size, sizeof(uint8_t), 1, sbmdl);

        uint8_t unk_format;
        fread(&unk_format, sizeof(uint8_t), 1, sbmdl);

        uint16_t unk_data;
        fread(&unk_data, sizeof(uint16_t), 1, sbmdl);

        if (unk_format == 1) {
            printf("unk_format 1, data = %u\n", unk_data);
        } else if (unk_format == 3) {
            struct vector3 unk_vec3;
            unk_vec3.x = (float)((unk_data & 0x7C00) >> 10) / 31.0;
            unk_vec3.y = (float)((unk_data & 0x3E0) >> 5) / 31.0;
            unk_vec3.z = (float)(unk_data & 0x1F) / 31.0;

            printf("unk_format 3, data = (%f, %f, %f)\n", unk_vec3.x, unk_vec3.y, unk_vec3.z);
        } else if (unk_format) {
            fprintf(stderr, "Invalid unk_foramt %u\n", unk_format);
            return 1;
        }

        bool is_floats = false;
        if (format == FORMAT_XBO) {
            if (vert_size == 26) is_floats = true;
            else if (vert_size != 20) printf("Format XBO does not have vert_size of 20, vert_size == %d\n", vert_size);

            printf("Format is XBO%s at %08lX\n", is_floats ? "_FLOAT" : "", ftell(sbmdl) - 24);
        } else if (format == FORMAT_XBO2) {
            if (vert_size == 22) is_floats = true;
            else if (vert_size != 16) printf("Format XBO2 does not have vert_size of 16, vert_size == %d\n", vert_size);

            printf("Format is XBO2%s at %08lX\n", is_floats ? "_FLOAT" : "", ftell(sbmdl) - 24);
        } else if (format == FORMAT_SHA) {
            if (vert_size != 12) printf("Format SHA does not have vert_size of 12, vert_size == %d\n", vert_size);
            printf("Format is SHA at %08lX\n", ftell(sbmdl) - 24);
        }

        if (!is_floats) {
            pos.x *= scale_factor;
            pos.y *= scale_factor;
            pos.z *= scale_factor;
        }

        verts_out[mi].pos.x = pos.x;
        verts_out[mi].pos.y = pos.y;
        verts_out[mi].pos.z = pos.z;

        verts_out[mi].dir.x = dir.x;
        verts_out[mi].dir.y = dir.y;
        verts_out[mi].dir.z = dir.z;

        if (vert_count * vert_size != mesh_size) {
            fprintf(stderr, "%d verts * %d vert_size != mesh_size %d\n", vert_count, vert_size, mesh_size);
            return 1;
        }

        verts_out[mi].count = vert_count;
        verts_out[mi].size = vert_count * verts_stride;

        printf("Reading %d vertices for %d triangles\n", vert_count, triangle_count);

        // Vertex data
        for (int vi = 0; vi < vert_count; vi++) {
            float vp[3];
            if (is_floats) {
                fread(vp, sizeof(float), 3, sbmdl);
            } else {
                int16_t vpi[3];
                fread(vpi, sizeof(int16_t), 3, sbmdl);
                for (int pi = 0; pi < 3; pi++) {
                    vp[pi] = vpi[pi];
                    vp[pi] *= scale_factor;
                }
            }

            fwrite(vp, sizeof(float), 3, outf);

            if (!vi) {
                verts_out[mi].max.x = verts_out[mi].min.x = vp[0];
                verts_out[mi].max.y = verts_out[mi].min.y = vp[1];
                verts_out[mi].max.z = verts_out[mi].min.z = vp[2];
            } else {
                verts_out[mi].min.x = fminf(verts_out[mi].min.x, vp[0]);
                verts_out[mi].min.y = fminf(verts_out[mi].min.y, vp[1]);
                verts_out[mi].min.z = fminf(verts_out[mi].min.z, vp[2]);

                verts_out[mi].max.x = fmaxf(verts_out[mi].max.x, vp[0]);
                verts_out[mi].max.y = fmaxf(verts_out[mi].max.y, vp[1]);
                verts_out[mi].max.z = fmaxf(verts_out[mi].max.z, vp[2]);
            }

            // Read vertex normals
            int16_t vni[3];
            fread(vni, sizeof(int16_t), 3, sbmdl);
            float vn[3] = { vni[0], vni[1], vni[2] };

            // Prevent divide by zero error
            if (!vni[0] && !vni[1] && !vni[2]) vn[0] = 1.0f;

            // Normalize
            float vn_len = sqrt((vn[0] * vn[0]) + (vn[1] * vn[1]) + (vn[2] * vn[2]));

            vn[0] /= vn_len;
            vn[1] /= vn_len;
            vn[2] /= vn_len;

            fwrite(vn, sizeof(float), 3, outf);

            // Vertex color
            uint8_t color[4] = {0xFF, 0xFF, 0xFF, 0xFF}; // Default to white
            if (format == FORMAT_XBO || format == FORMAT_XBO2) {
                uint8_t argb[4]; // ARGB
                fread(argb, sizeof(uint8_t), 4, sbmdl);
                color[0] = argb[1]; // R
                color[1] = argb[2]; // G
                color[2] = argb[3]; // B
                color[4] = argb[0]; // A
            }

            float vt[2] = {0.0f, 0.0f};
            if (format == FORMAT_XBO) {
                // Read vertex texture coords
                int16_t vti[2];
                fread(vti, sizeof(int16_t), 2, sbmdl);
                vt[0] = vti[0];
                vt[1] = vti[1];

                vt[0] /= 32768.0f;
                vt[1] /= 32768.0f;
            }
            fwrite(vt, sizeof(float), 2, outf);

            uint8_t joints[4] = {mi + 1};
            fwrite(joints, sizeof(uint8_t), 4, outf);

            uint8_t weights[4] = {0xFF};
            fwrite(weights, sizeof(uint8_t), 4, outf);

            fwrite(color, sizeof(uint8_t), 4, outf);
        }
    }

    if (extra) {
        fseek(sbmdl, MODEL_HEADER_OFFSET + extra, SEEK_SET);

        uint16_t extra_count;
        fread(&extra_count, sizeof(uint16_t), 1, sbmdl);

        fseek(sbmdl, 12, SEEK_CUR); // Skip offsets

        printf("Processing %u extra data entries at %08X\n", extra_count, MODEL_HEADER_OFFSET + extra);

        uint8_t nodes[extra_count * 3];
        fread(nodes, sizeof(uint8_t), extra_count * 3, sbmdl);

        struct vector2 vec2s[extra_count];
        fread(vec2s, sizeof(struct vector2), extra_count, sbmdl);

        struct vector3 vec3sA[extra_count];
        fread(vec3sA, sizeof(struct vector3), extra_count, sbmdl);

        struct vector3 vec3sB[extra_count];
        fread(vec3sB, sizeof(struct vector3), extra_count, sbmdl);

        struct vector3 vec3sC[extra_count];
        fread(vec3sC, sizeof(struct vector3), extra_count, sbmdl);

        for (int xi = 0; xi < extra_count; xi++) {
            uint8_t * nd = nodes + xi * 3;
            printf("Extra nodes: %u %u %u\n", nd[0], nd[1], nd[2]);

            printf("Extra vec2: (%f, %f)\n", vec2s[xi].x, vec2s[xi].y);

            printf("Extra vec3A: (%f, %f, %f)\n", vec3sA[xi].x, vec3sA[xi].y, vec3sA[xi].z);
            printf("Extra vec3B: (%f, %f, %f)\n", vec3sB[xi].x, vec3sB[xi].y, vec3sB[xi].z);
            printf("Extra vec3C: (%f, %f, %f)\n", vec3sC[xi].x, vec3sC[xi].y, vec3sB[xi].z);
        }
    }

    uint32_t node_special_count = 0;
    uint32_t node_special_flags = 0;
    uint8_t node_special[32];
    if (nodespecial_section.size) {
        fseek(sbmdl, nodespecial_section.offset, SEEK_SET);

        fread(&node_special_count, sizeof(uint32_t), 1, sbmdl);
        fread(&node_special_flags, sizeof(uint32_t), 1, sbmdl);

        printf("Special node enabled flags: %08X\n", node_special_flags);
        printf("Read %d special node IDs:", node_special_count);

        for (int ni = 0, nr = 0; ni < 32; ni++) {
            if (CHECK_BIT(node_special_flags, ni)) {
                if (nr >= node_special_count) {
                    fprintf(stderr, "Special node enable flag bits do not match count\n");
                    return 1;
                }
                nr++;

                fread(node_special + ni, sizeof(uint8_t), 1, sbmdl);
                printf(" %02d", node_special[ni]);
            } else {
                printf(" XX");
            }
        }

        printf("\n");
    }

    uint8_t * mirror_ids = NULL;
    if (nodemirror_section.size) {
        fseek(sbmdl, nodemirror_section.offset, SEEK_SET);

        if (nodemirror_section.size != node_count) {
            fprintf(stderr, "Node ID section size %d != node_count %d\n", nodemirror_section.size, node_count);
            return 1;
        }

        mirror_ids = malloc(node_count * sizeof(uint8_t));
        fread(mirror_ids, sizeof(uint8_t), node_count, sbmdl);

        printf("Read %d mirror node IDs:", node_count);
        for (int ni = 0; ni < node_count; ni++) {
            int mi = mirror_ids[ni];

            // Fixup mirror IDs
            if (mi != ni && mirror_ids[mi] == mi) {
                mirror_ids[mi] = ni;
            }

            if (mi == ni) printf(" XX");
            else printf(" %02d", mi);
        }
        printf("\n");
    }

    /*
    if (ftell(sbmdl) != index_section.offset) {
        fprintf(stderr, "Index section mismatch %08lX %08X\n", ftell(sbmdl), index_section.offset);
        return 1;
    }
    */

    fseek(sbmdl, index_section.offset, SEEK_SET);

    // Read Index Data Header
    struct buffer * inds_in = malloc(mesh_count * sizeof(struct buffer));
    fread(inds_in, sizeof(struct buffer), mesh_count, sbmdl);

    struct primative * inds_out = malloc(mesh_count * sizeof(struct primative));

    //printf("Index offset last %08X %08X\n", indexes[mesh_count - 1].offset, indexes[mesh_count - 1].size);

    for (int mi = 0; mi < mesh_count; mi++) {
        //fseek(sbmdl, index_section.offset + inds_in[mi].offset, SEEK_SET);
        int32_t inds = inds_in[mi].size;

        inds_out[mi].offset = mi ? inds_out[mi - 1].offset + inds_out[mi - 1].size : 0;

        if (inds % 2) {
            printf("inds %% 2 == %d, inds == %d\n", inds % 2, inds);
        }
        inds /= 2;

        printf("Mesh %d: Reading %d indexes\n", mi, inds);

        if (prim_type == cgltf_primitive_type_triangle_strip) {
            inds_out[mi].count = inds;
            for (int ci = 0; ci < inds; ci++) {
                uint16_t ind;
                fread(&ind,  sizeof(uint16_t), 1, sbmdl);

                if (flip_faces && !ci) {
                    // Reverse the winding order
                    fwrite(&ind, sizeof(uint16_t), 1, outf);
                    inds_out[mi].count++;
                }

                fwrite(&ind, sizeof(uint16_t), 1, outf);
            }
        } else if (prim_type == cgltf_primitive_type_triangles) {
            inds_out[mi].count = 0;
            uint16_t face[3] = {0, 0, 0};
            for (int ci = 0; ci < inds; ci++) {
                face[2] = face[1];
                face[1] = face[0];
                fread(face, sizeof(uint16_t), 1, sbmdl);

                if (ci >= 2) {
                    if (inds == 3 || (face[0] != face[1] && face[1] != face[2] && face[2] != face[0])) {
                        fwrite(face, sizeof(uint16_t), 3, outf);
                        inds_out[mi].count += 3;
                    }
                }
            }
        } else {
            fprintf(stderr, "Unsupported primitive type\n");
            fclose(outf);
            fclose(sbmdl);
            return 1;
        }

        inds_out[mi].size = inds_out[mi].count * sizeof(uint16_t);
    }

    fclose(outf);
    fclose(sbmdl);

    cgltf_options options = {0};
    cgltf_data data = {0};

    data.file_type = cgltf_file_type_gltf;

    data.asset.generator = strdup("sbmodel");
    data.asset.version = strdup("2.0");

    data.meshes_count = 1;
    cgltf_mesh * mesh = data.meshes = calloc(data.meshes_count, sizeof(cgltf_mesh));
    mesh->name = strdup(model_name);

    const int accessor_count = 7;

    data.accessors_count = mesh_count * accessor_count;
    data.accessors = calloc(data.accessors_count, sizeof(cgltf_accessor));

    data.buffers_count = 1;
    cgltf_buffer * buf = data.buffers = calloc(data.buffers_count, sizeof(cgltf_buffer));

    buf->name = strdup(model_name);

    char * glbin_name = sep ? sep + 1 : path;
    buf->uri = strdup(glbin_name);

    data.buffer_views_count = 2;
    data.buffer_views = calloc(data.buffer_views_count, sizeof(cgltf_buffer_view));

    data.skins_count = 1;
    cgltf_skin * skin = data.skins = calloc(data.skins_count, sizeof(cgltf_skin));
    skin->joints_count = node_count;
    skin->joints = malloc(skin->joints_count * sizeof(cgltf_node *));

    data.nodes_count = node_count;
    data.nodes = calloc(data.nodes_count, sizeof(cgltf_node));

    data.scenes_count = 1;
    cgltf_scene * scene = data.scene = data.scenes = calloc(data.scenes_count, sizeof(cgltf_scene));

    scene->name = strdup(model_name);
    scene->nodes_count = 1;
    scene->nodes = calloc(scene->nodes_count, sizeof(cgltf_node *));
    scene->nodes[0] = data.nodes;

    printf("Generating GLTF and GLBIN files\n");

    for (int ni = 0; ni < node_count; ni++) {
        cgltf_node * node = data.nodes + ni;

        int mirror_id = mirror_ids ? mirror_ids[ni] : ni;
        int special_id = -1;
        for (int nsi = 0; nsi < 32; nsi++) {
            if (CHECK_BIT(node_special_flags, nsi) && node_special[nsi] == ni) {
                special_id = nsi;
                break;
            }
        }

        char name[64];
        if (special_id >= 0) {
            if (mirror_id < ni) {
                snprintf(name, sizeof(name), "special_%d:%d_a", special_id, mirror_id);
            } else if (mirror_id > ni) {
                snprintf(name, sizeof(name), "special_%d:%d_b", special_id, ni);
            } else {
                snprintf(name, sizeof(name), "special_%d", special_id);
            }
        } else {
            if (mirror_id < ni) {
                snprintf(name, sizeof(name), "%d_a", mirror_id);
            } else if (mirror_id > ni) {
                snprintf(name, sizeof(name), "%d_b", ni);
            } else {
                snprintf(name, sizeof(name), "%d", ni);
            }
        }
        node->name = strdup(name);

        if (!ni) {
            node->mesh = mesh;
            node->skin = skin;

            node->translation[0] = global_pos.x;
            node->translation[1] = global_pos.y;
            node->translation[2] = global_pos.z;
            node->has_translation = true;

            euler2quat(node->rotation, global_dir);
            node->has_rotation = true;
        } else if (node_parents[ni] == 0xFF) {
            fprintf(stderr, "Node %d is a second root node!\n", ni);
            return 1;
        } else {
            int mi = ni - 1;

            node->translation[0] = verts_out[mi].pos.x;
            node->translation[1] = verts_out[mi].pos.y;
            node->translation[2] = verts_out[mi].pos.z;
            node->has_translation = true;

            euler2quat(node->rotation, verts_out[mi].dir);
            node->has_rotation = true;

        }

        skin->joints[ni] = node;

        for (int nj = 0; nj < node_count; nj++) {
            if (node_parents[nj] == ni) {
                if (nj == ni) {
                    fprintf(stderr, "Node %d is it's own child!\n", nj);
                    return 1;
                }

                node->children_count++;
            }
        }

        if (node->children_count) {
            node->children = malloc(node->children_count * sizeof(cgltf_node *));

            for (int cni = 0, nj = 0; nj < node_count; nj++) {
                if (node_parents[nj] == ni) {
                    node->children[cni++] = data.nodes + nj;
                }
            }
        };
    }

    skin->skeleton = *skin->joints;

    cgltf_buffer_view * verts_view = data.buffer_views;
    cgltf_buffer_view * inds_view = data.buffer_views + 1;

    verts_view->name = strdup("Verticies");
    verts_view->buffer = buf;
    verts_view->type = cgltf_buffer_view_type_vertices;
    verts_view->stride = verts_stride;

    inds_view->name = strdup("Indicies");
    inds_view->buffer = buf;
    inds_view->type = cgltf_buffer_view_type_indices;

    for (int mi = 0; mi < mesh_count; mi++) {
        verts_view->size += verts_out[mi].size;
        inds_view->size += inds_out[mi].size;
    }

    inds_view->offset = verts_view->size;

    buf->size = verts_view->size + inds_view->size;

    mesh->primitives = calloc(mesh_count, sizeof(cgltf_primitive));
    mesh->primitives_count = mesh_count;

    for (int mi = 0; mi < mesh_count; mi++) {
        cgltf_primitive * prim = mesh->primitives + mi;

        char name[16];

        // There's a bug in this cgltf where we need to subtract 1
        prim->type = prim_type - 1;

        const int accessor_pos = mi * accessor_count;

        prim->attributes_count = accessor_count - 1;
        prim->attributes = calloc(prim->attributes_count, sizeof(cgltf_attribute));

        for (int si = 0; si < prim->attributes_count; si++) {
            int sio = accessor_pos + si;

            data.accessors[sio].buffer_view = verts_view;

            data.accessors[sio].offset = verts_out[mi].offset;
            data.accessors[sio].stride = verts_view->stride;
            data.accessors[sio].count  = verts_out[mi].count;
        }

        cgltf_accessor * pos_acc = data.accessors + accessor_pos;

        printf("Mesh %d: Offset %ld, Size %d, Count %ld\n", mi, pos_acc->offset, verts_out[mi].size, pos_acc->count);

        snprintf(name, sizeof(name), "Position %d", mi);
        pos_acc->name = strdup(name);
        pos_acc->component_type = cgltf_component_type_r_32f;
        pos_acc->type = cgltf_type_vec3;

        pos_acc->min[0] = verts_out[mi].min.x;
        pos_acc->min[1] = verts_out[mi].min.y;
        pos_acc->min[2] = verts_out[mi].min.z;
        pos_acc->has_min = true;

        pos_acc->max[0] = verts_out[mi].max.x;
        pos_acc->max[1] = verts_out[mi].max.y;
        pos_acc->max[2] = verts_out[mi].max.z;
        pos_acc->has_max = true;

        cgltf_accessor * norm_acc = data.accessors + accessor_pos + 1;
        snprintf(name, sizeof(name), "Normal %d", mi);
        norm_acc->name = strdup(name);
        norm_acc->component_type = cgltf_component_type_r_32f;
        norm_acc->type = cgltf_type_vec3;
        norm_acc->offset += 3 * sizeof(float);

        cgltf_accessor * uv_acc = data.accessors + accessor_pos + 2;
        snprintf(name, sizeof(name), "Texture %d", mi);
        uv_acc->name = strdup(name);
        uv_acc->component_type = cgltf_component_type_r_32f;
        uv_acc->type = cgltf_type_vec2;
        uv_acc->offset += 6 * sizeof(float);

        cgltf_accessor * joint_acc = data.accessors + accessor_pos + 3;
        snprintf(name, sizeof(name), "Joint %d", mi);
        joint_acc->name = strdup(name);
        joint_acc->component_type = cgltf_component_type_r_8u;
        joint_acc->type = cgltf_type_vec4;
        joint_acc->offset += 8 * sizeof(float);

        cgltf_accessor * weight_acc = data.accessors + accessor_pos + 4;
        snprintf(name, sizeof(name), "Weight %d", mi);
        weight_acc->name = strdup(name);
        weight_acc->normalized = true;
        weight_acc->component_type = cgltf_component_type_r_8u;
        weight_acc->type = cgltf_type_vec4;
        weight_acc->offset += (8 * sizeof(float)) + (4 * sizeof(uint8_t));

        cgltf_accessor * color_acc = data.accessors + accessor_pos + 5;
        snprintf(name, sizeof(name), "Color %d", mi);
        color_acc->name = strdup(name);
        color_acc->normalized = true;
        color_acc->component_type = cgltf_component_type_r_8u;
        color_acc->type = cgltf_type_vec4;
        color_acc->offset += (8 * sizeof(float)) + ((4 * sizeof(uint8_t)) * 2);

        cgltf_accessor * ind_acc = data.accessors + accessor_pos + 6;
        snprintf(name, sizeof(name), "Indicies %d", mi);
        ind_acc->name = strdup(name);
        ind_acc->component_type = cgltf_component_type_r_16u;
        ind_acc->type = cgltf_type_scalar;

        ind_acc->offset = inds_out[mi].offset;
        ind_acc->stride = sizeof(uint16_t);
        ind_acc->count  = inds_out[mi].count;

        ind_acc->buffer_view = inds_view;

        prim->indices = ind_acc;

        cgltf_attribute * pos_atr = prim->attributes;
        pos_atr->name = strdup("POSITION");
        pos_atr->type = cgltf_attribute_type_position;
        pos_atr->index = accessor_pos;

        cgltf_attribute * norm_atr = prim->attributes + 1;
        norm_atr->name = strdup("NORMAL");
        norm_atr->type = cgltf_attribute_type_position;
        norm_atr->index = accessor_pos + 1;

        cgltf_attribute * uv_atr = prim->attributes + 2;
        uv_atr->name = strdup("TEXCOORD_0");
        uv_atr->type = cgltf_attribute_type_texcoord;
        uv_atr->index = accessor_pos + 2;

        cgltf_attribute * joint_atr = prim->attributes + 3;
        joint_atr->name = strdup("JOINTS_0");
        joint_atr->type = cgltf_attribute_type_joints;
        joint_atr->index = accessor_pos + 3;

        cgltf_attribute * weights_atr = prim->attributes + 4;
        weights_atr->name = strdup("WEIGHTS_0");
        weights_atr->type = cgltf_attribute_type_weights;
        weights_atr->index = accessor_pos + 4;

        cgltf_attribute * color_atr = prim->attributes + 5;
        color_atr->name = strdup("COLOR_0");
        color_atr->type = cgltf_attribute_type_color;
        color_atr->index = accessor_pos + 5;

        for (int ai = 0; ai < prim->attributes_count; ai++) {
            prim->attributes[ai].data = data.accessors + prim->attributes[ai].index;
        }
    }

    cgltf_result result = cgltf_validate(&data);
    if (result != cgltf_result_success) {
	    fprintf(stderr, "Failed to validate glTF data\n");
        return 1;
    }

    ext[-1] = '.';
    strcpy(ext, "gltf");

    result = cgltf_write_file(&options, path, &data);
    if (result != cgltf_result_success) {
	    fprintf(stderr, "Failed to write glTF file\n");
        return 1;
    }

    return 0;
}
