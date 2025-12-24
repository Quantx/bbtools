#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>


#ifdef __linux__
#include <iconv.h>
#else
#include <stringapiset.h>
#endif

#define STRING_COUNT 1637
struct string_pair {
    uint32_t jp;
    uint32_t en;
} string_pointers[STRING_COUNT];

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

#define BUF16_LEN 2048
char buf16[BUF16_LEN];
#define BUF8_LEN 2048
char buf8[BUF8_LEN];

char * progname;
char * in_path;
char * out_path;

char path[256];

void convert_string(void) {
#ifdef __linux__
    size_t buf16_len = BUF16_LEN;
    size_t buf8_len = BUF8_LEN;
    
    char * ptr8 = buf8;
    char * ptr16 = buf16;
    
    iconv_t conv = iconv_open("UTF-8", "UTF-16");
    iconv(conv, &ptr16, &buf16_len, &ptr8, &buf8_len);
    iconv_close(conv);
#else
    WideCharToMultiByte(CP_UTF8, 0, (uint16_t *)buf16, BUF16_LEN / 2, buf8, BUF8_LEN, NULL, NULL);
#endif
}

void output_string(FILE * outf, bool is_english) {
    fputc('"', outf);

    for (int j = 0; buf8[j]; j++) {
        switch (buf8[j]) {
//        case '\n':
//            fprintf(outf, "\\n");
//            break;
        case '\v':
        case '\t':
        case '\a':
            fprintf(outf, "%%s");
            j++;
            break;
        case '"':
            fprintf(outf, "\"\"");
            break;
        default:
            fputc(buf8[j], outf);
        }
    }
    
    fputc('"', outf);
}

int main(int argc, char ** argv) {
    progname = *argv++; argc--;
    
    if (argc != 2) {
        fprintf(stderr, "Usage: ./%s <path/default/> <strings.csv>\n", progname);
    }
    
    in_path  = *argv++; argc--;
    out_path = *argv++; argc--;

    snprintf(path, sizeof(path), "%s.data.seg", in_path);
    FILE * datf = fopen(path, "rb");
    if (!datf) {
        fprintf(stderr, "Failed to open: '%s'\n", path);
        return 1;
    }
    
    fseek(datf, 0x59D50, SEEK_SET);
    fread(string_pointers, sizeof(struct string_pair), STRING_COUNT, datf);
    
    fclose(datf);

    snprintf(path, sizeof(path), "%s.rdata.hdr", in_path);
    FILE * hdrf = fopen(path, "rb");
    if (!hdrf) {
        fprintf(stderr, "Failed to open: '%s'\n", path);
        return 1;
    }
    
    struct section_header hdr_data;
    fread(&hdr_data, sizeof(struct section_header), 1, hdrf);
    
    fclose(hdrf);
    
    snprintf(path, sizeof(path), "%s.rdata.seg", in_path);
    FILE * rdatf = fopen(path, "rb");
    if (!rdatf) {
        fprintf(stderr, "Failed to open: '%s'\n", path);
        return 1;
    }
    
    FILE * outf = fopen(out_path, "w");
    if (!outf) {
        fprintf(stderr, "Failed to open: '%s'\n", out_path);
        return 1;
    }
    
    fprintf(outf, "keys,ja,en\n");
    
    for (int i = 0; i < STRING_COUNT; i++) {
        if (i < 6) continue;
        
        uint16_t jp_present = 0;
        if (string_pointers[i].jp) {
            fseek(rdatf, string_pointers[i].jp - hdr_data.vaddr, SEEK_SET);
            // Ensure string is at least 2 chars long
            fread(&jp_present, sizeof(uint16_t), 1, rdatf);
            if (jp_present) fread(&jp_present, sizeof(uint16_t), 1, rdatf);
        }
        
        uint16_t en_present = 0;
        if (string_pointers[i].en) {
            fseek(rdatf, string_pointers[i].en - hdr_data.vaddr, SEEK_SET);
            // Ensure string is at least 2 chars long
            fread(&en_present, sizeof(uint16_t), 1, rdatf);
            if (en_present) fread(&en_present, sizeof(uint16_t), 1, rdatf);
        }
        
        // Both are missing, skip this
        if (!en_present && !jp_present) continue;
        
        fprintf(outf, "loc:%04d,", i);
        
        if (jp_present) {
            fseek(rdatf, string_pointers[i].jp - hdr_data.vaddr, SEEK_SET);
            fread(buf16, sizeof(char), BUF16_LEN, rdatf);
            
            convert_string();

            output_string(outf, false);
            fprintf(outf, ",");
            if (!en_present) output_string(outf, false);
        }
        
        if (en_present) {
            fseek(rdatf, string_pointers[i].en - hdr_data.vaddr, SEEK_SET);
            fread(buf16, sizeof(char), BUF16_LEN, rdatf);
            
            convert_string();
            
            output_string(outf, true);
            if (!jp_present) {
                fprintf(outf, ",");
                output_string(outf, true);
            }
        }
        
        fprintf(outf, "\n");
    }
    
    fclose(rdatf);
    fclose(outf);
}
