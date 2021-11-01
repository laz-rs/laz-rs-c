#include <minilas/las.h>

las_file_t *las_file_open(const char *path)
{
    FILE *file = fopen(path, "rb");
    if (file == NULL)
    {
        return NULL;
    }

    las_header header;
    if (fread_las_header(file, &header) != las_error_ok)
    {
        fclose(file);
        return NULL;
    }

    las_file_t *las_file = malloc(sizeof(las_file_t));
    las_file->header = header;
    las_file->file = file;
    return las_file;
}

void las_file_close(las_file_t *las_file)
{
    if (las_file != NULL)
    {
        fclose(las_file->file);
        las_clean_header(&las_file->header);
        free(las_file);
    }
}

void las_clean_vlr(las_vlr *vlr)
{
    if (vlr->record_len != 0 && vlr->data != NULL)
    {
        free(vlr->data);
        vlr->data = NULL;
        vlr->record_len = 0;
    }
}
void las_clean_header(las_header *header)
{
    if (header->number_of_vlrs != 0 && header->vlrs != NULL)
    {
        for (int i = 0; i < header->number_of_vlrs; ++i)
        {
            las_clean_vlr(&header->vlrs[i]);
        }
        free(header->vlrs);
        header->vlrs = NULL;
        header->number_of_vlrs = 0;
    }
}
las_error fread_las_header(FILE *file, las_header *header)
{
    if (header == NULL)
    {
        return las_error_other;
    }

    uint8_t raw_header[LAS_HEADER_SIZE];
    size_t num_read = fread(raw_header, sizeof(uint8_t), LAS_HEADER_SIZE, file);
    if (num_read < LAS_HEADER_SIZE && ferror(file) != 0)
    {
        return las_error_io;
    }

    if (strncmp((const char *)raw_header, "LASF", 4) != 0)
    {
        int length = 4;
        printf("%*.*s", length, length, (const char *)raw_header);
        fprintf(stderr, "Invalid file signature\n");
        return las_error_other;
    }

    header->version_major = raw_header[24];
    header->version_minor = raw_header[25];
    header->offset_to_point_data = *(uint32_t *)(raw_header + 96);
    header->number_of_vlrs = *(uint32_t *)(raw_header + 100);
    header->point_format = *(uint8_t *)(raw_header + 104);
    header->point_size = *(uint16_t *)(raw_header + 105);
    header->point_count = *(uint32_t *)(raw_header + 107);

    int compression_bit_7 = (header->point_format & 0x80) >> 7;
    int compression_bit_6 = (header->point_format & 0x40) >> 6;
    header->is_data_compressed = !compression_bit_6 && compression_bit_7;
    header->point_format = header->point_format & 0x3F;

    uint16_t header_size = *(uint16_t *)(raw_header + 94);
    if (fseek(file, header_size, SEEK_SET) != 0)
    {
        return las_error_io;
    }

    header->vlrs = (las_vlr *)malloc(header->number_of_vlrs * sizeof(las_vlr));
    if (header->vlrs == NULL)
    {
        return las_error_oom;
    }

    uint8_t raw_vlr_header[LAS_VLR_HEADER_SIZE];
    for (size_t i = 0; i < header->number_of_vlrs; ++i)
    {
        num_read = fread(raw_vlr_header, sizeof(uint8_t), LAS_VLR_HEADER_SIZE, file);
        if (num_read < LAS_VLR_HEADER_SIZE && ferror(file) != 0)
        {
            return las_error_io;
        }

        las_vlr *vlr = &header->vlrs[i];
        memcpy(vlr->user_id, raw_vlr_header + 2, sizeof(uint8_t) * 16);
        vlr->record_id = *(uint16_t *)(raw_vlr_header + 18);
        vlr->record_len = *(uint16_t *)(raw_vlr_header + 20);
        vlr->data = (uint8_t *)malloc(sizeof(uint8_t) * vlr->record_len);
        if (vlr->data == NULL)
        {
            return las_error_oom;
        }

        num_read = fread(vlr->data, sizeof(uint8_t), vlr->record_len, file);
        if (num_read < vlr->record_len && ferror(file))
        {
            return las_error_io;
        }
    }

    if (header->version_minor >= 4)
    {
        if (fseek(file, 247, SEEK_SET) != 0)
        {
            return las_error_io;
        }
        fread(&header->point_count, sizeof(uint64_t), 1, file);
    }

    return las_error_ok;
}

const las_vlr *find_laszip_vlr(const las_header *las_header)
{
    const las_vlr *vlr = NULL;

    for (uint16_t i = 0; i < las_header->number_of_vlrs; ++i)
    {
        const las_vlr *current = &las_header->vlrs[i];
        if (strcmp(current->user_id, "laszip encoded") == 0 && current->record_id == 22204)
        {
            vlr = current;
            break;
        }
    }
    return vlr;
}

void print_vlrs(const las_header *header)
{
    printf("Number of vlrs: %d\n", header->number_of_vlrs);
    for (uint32_t i = 0; i < header->number_of_vlrs; ++i)
    {
        las_vlr *vlr = &header->vlrs[i];
        printf("user_id: %s, record_id: %d, data len: %d\n",
               vlr->user_id,
               vlr->record_id,
               vlr->record_len);
    }
}

void print_header(const las_header *header)
{
    printf("Version: %d.%d\n", (int)header->version_major, (int)header->version_minor);
    printf("Point size: %d, point count: %llu\n", header->point_size, header->point_count);
    print_vlrs(header);
}

void las_file_read_all_point_data(las_file_t *las_file, uint8_t **output, size_t *len)
{
    // TODO Err handling
    size_t len_to_read;
    if (las_file->header.is_data_compressed)
    {
        fseek(las_file->file, 0, SEEK_END);
        long end = ftell(las_file->file);
        len_to_read = end - las_file->header.offset_to_point_data;
    }
    else
    {
        len_to_read = las_file->header.point_size * las_file->header.point_count;
    }

    if (len_to_read == 0)
    {
        return;
    }

    *output = (uint8_t *)malloc(sizeof(uint8_t) * len_to_read);
    assert(output != NULL);
    fseek(las_file->file, las_file->header.offset_to_point_data, SEEK_SET);
    fread(*output, sizeof(uint8_t), len_to_read, las_file->file);
    *len = len_to_read;
}