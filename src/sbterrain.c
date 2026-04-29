#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>

#include "dds.h"

#include "swizzle.h"

#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include "stb_image_write.h"

#define MAP_WIDTH 280
#define MAP_HEIGHT 280

char * helpmsg = "Used pack and unpack SB heightmap terrain files\n"
"Show help: sbterrain -h\n"
"Unpack map: sbterrain -u map00.gnd\n"
"Pack map: sbterrain -p map00.gnd\n";

char * progname;

int pack(char * path) {
    fprintf(stderr, "Not implemented\n");
    return 1;
}

int unpack(char * path) {
    char * ext = strrchr(path, '.');
    if (!ext) {
        fprintf(stderr, "Path is missing extension: %s\n", path);
        return 1;
    }

    FILE * gndf = fopen(path, "rb");
    if (!gndf) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 1;
    }

    printf("Reading terrain heightmap data from %s\n", path);

    // Change path extension to DDS
    strcpy(ext + 1, "dds");

    FILE * hmdf = fopen(path, "wb");
    if (!hmdf) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 1;
    }

    struct dds_header header = DDS_HEADER_INIT;

    header.width = MAP_WIDTH;
    header.height = MAP_HEIGHT;
    size_t size = header.width * header.height;

    header.flags |= 0x8; // Linear size is provided
    header.pitch = size * sizeof(float) * 2;

    // Enable DX10 header
    memcpy(header.format.codeStr, "DX10", 4);
    header.format.flags = 0x4;

    struct dds_header_dx10 header10 = {
        .format = 16, // R32G32F
        .dimensions = DDS_DX10_DIMENSION_2D,
        .arraySize = 1,
    };

    fputs("DDS ", hmdf);
    fwrite(&header, sizeof(struct dds_header), 1, hmdf);
    fwrite(&header10, sizeof(struct dds_header_dx10), 1, hmdf);

    float * rChannel = malloc(size * sizeof(float));
    float * gChannel = malloc(size * sizeof(float));

    fread(rChannel, sizeof(float), size, gndf); // This channel stores the heightmap data
    fread(gChannel, sizeof(float), size, gndf);

    // Stripe the R and G channels
    for (uint32_t px = 0; px < size; px++) {
        fwrite(rChannel + px, sizeof(float), 1, hmdf);
        fwrite(gChannel + px, sizeof(float), 1, hmdf);
    }

    free(rChannel);
    free(gChannel);

    fclose(hmdf);

    uint8_t * texture = malloc(size * 4);
    fread(texture, sizeof(uint8_t), size * 4, gndf);

    fclose(gndf);

    // Swizzle the R and B channels
    for (int i = 0; i < size * 4; i+= 4) {
        uint8_t temp = texture[i];
        texture[i] = texture[i + 2];
        texture[i + 2] = temp;
    }

    strcpy(ext + 1, "tga");

    if (!stbi_write_tga(path, MAP_WIDTH, MAP_HEIGHT, 4, texture)) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 1;
    }

    free(texture);

    return 0;
}

int convertXRAW(char * path) {
    char * ext = strrchr(path, '.');
    if (!ext) {
        fprintf(stderr, "Path is missing extension: %s\n", path);
        return 1;
    }

    // Open XRAW file
    FILE * xraw = fopen(path, "rb");
    if (!xraw) {
        fprintf(stderr, "Could not open file: %s\n", path);
        return 1;
    }

    char xraw_magic[4];
    fread(xraw_magic, sizeof(char), 4, xraw);

    if (strncmp(xraw_magic, "XRAW", 4)) {
        fclose(xraw);
        fprintf(stderr, "Not an XRAW texture file\n");
        return 1;
    }

    uint8_t xraw_unknown;
    fread(&xraw_unknown, sizeof(uint8_t), 1, xraw);

    uint8_t xraw_format;
    fread(&xraw_format, sizeof(uint8_t), 1, xraw);

    uint16_t xraw_size;
    fread(&xraw_size, sizeof(uint16_t), 1, xraw);

    uint32_t width  = 1 << ((xraw_size & 0x0F00) >> 8);
    uint32_t height = 1 << ((xraw_size & 0x00F0) >> 4);
    uint32_t levels =       (xraw_size & 0x000F);

    printf("Converting XRAW width %u, height %u, levels %u, format %02X, unknown %02X\n",
        width, height, levels, xraw_format, xraw_unknown);

    // Skip to the data
    fseek(xraw, 16, SEEK_SET);

    // Change path extension to DDS
    strcpy(ext + 1, "dds");

    FILE * dds = fopen(path, "wb");
    if (!dds) {
        fprintf(stderr, "Failed to open %s\n", path);
        return 1;
    }

    struct dds_header header = DDS_HEADER_INIT;

    header.width = width;
    header.height = height;
    header.levels = levels;

    header.flags |= 0x8;
    //header.flags |= 0x20000 | 0x8; // Linear size is provided, mipmaps present
    header.pitch = header.width * header.height * (sizeof(float) * 2);

    header.format.flags = 0x40; // RGB
    header.format.rgbBitCount = 24;
    header.format.rBitMask = 0x0000FF;
    header.format.gBitMask = 0x00FF00;
    header.format.bBitMask = 0xFF0000;

    fputs("DDS ", dds);
    fwrite(&header, sizeof(struct dds_header), 1, dds);

    uint8_t * xraw_texture = malloc(width * height * 2);
    uint8_t * dds_texture = malloc(width * height * 2);

    for (uint32_t l = 0; l < levels; l++) {
        fread(xraw_texture, sizeof(uint8_t), width * height * 2, xraw);

        unswizzle_rect(
            xraw_texture, width, height,
            dds_texture, width * 2, 2
        );

        for (uint32_t i = 0; i < width * height; i++) {
            // These are actually signed textures, so convert them to unsigned
            uint8_t r = dds_texture[i * 2] - 0x80;
            uint8_t g = dds_texture[i * 2 + 1] - 0x80;
            uint8_t b = 0;

            fwrite(&r, sizeof(uint8_t), 1, dds);
            fwrite(&g, sizeof(uint8_t), 1, dds);
            fwrite(&b, sizeof(uint8_t), 1, dds);
        }

        width /= 2;
        height /= 2;
    }

    free(xraw_texture);
    free(dds_texture);

    fclose(dds);
    fclose(xraw);
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;

    printf("SB Terrain Tool - By QuantX\n");

    char progmode = 0;
    while (argc && **argv == '-') {
        (*argv)++;
        switch (**argv) {
        case 'h':
            printf("%s", helpmsg);
            return 0;
        case 'p':
        case 'u':
        case 'x':
            progmode = **argv;
            break;
        }
    	argv++; argc--;
    }

    if (!progmode) {
        fprintf(stderr, "Please provide either '-p', '-u', or '-x', for help run: %s -h\n", progname);
        return 1;
    }

    if (!argc) {
        if (progmode == 'p') fprintf(stderr, "Please specify a map to pack\n");
        else if (progmode == 'u') fprintf(stderr, "Please specify a map unpack\n");
        else if (progmode == 'x') fprintf(stderr, "Please specify a raw file to unpack\n");
        return 1;
    }

    if (progmode == 'p') return pack(*argv);
    else if (progmode == 'u') return unpack(*argv);
    else if (progmode == 'x') return convertXRAW(*argv);

    fprintf(stderr, "Unknown progmode: %c\n", progmode);
    return 1;
}
