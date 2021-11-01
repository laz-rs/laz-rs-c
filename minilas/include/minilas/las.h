#ifndef TEST_LASRSC_LAS_H
#define TEST_LASRSC_LAS_H

#ifdef __cplusplus
extern "C"
{
#endif

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LAS_HEADER_SIZE 227
#define LAS_VLR_HEADER_SIZE 54

    typedef struct
    {
        char user_id[16];
        uint16_t record_id;
        uint16_t record_len;
        uint8_t *data;
    } las_vlr;

    typedef struct
    {
        uint8_t version_major;
        uint8_t version_minor;
        uint64_t point_count;
        uint16_t point_size;
        uint8_t point_format;
        uint8_t is_data_compressed;
        uint32_t offset_to_point_data;

        uint32_t number_of_vlrs;
        las_vlr *vlrs;
    } las_header;

    typedef enum
    {
        las_error_ok = 0,
        las_error_io,
        las_error_oom,
        las_error_other
    } las_error;

    typedef struct
    {
        FILE *file;
        las_header header;
    } las_file_t;

    las_file_t *las_file_open(const char *path);
    void las_file_close(las_file_t *file);

    void las_file_read_all_point_data(las_file_t *las_file, uint8_t **output, size_t *len);

    void las_clean_header(las_header *header);
    las_error fread_las_header(FILE *file, las_header *header);
    const las_vlr *find_laszip_vlr(const las_header *las_header);
    void print_vlrs(const las_header *header);
    void print_header(const las_header *header);

#ifdef __cplusplus
}
#endif
#endif // TEST_LASRSC_LAS_H
