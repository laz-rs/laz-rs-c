#ifndef TEST_LASRSC_LAS_H
#define TEST_LASRSC_LAS_H

#include <stdlib.h>
#include <string.h>

#define LAS_HEADER_SIZE 227
#define LAS_VLR_HEADER_SIZE 54

typedef struct {
  char user_id[16];
  uint16_t record_id;
  uint16_t record_len;
  uint8_t *data;
} las_vlr;

void las_clean_vlr(las_vlr *vlr) {
  if (vlr->record_len != 0 && vlr->data != NULL) {
    free(vlr->data);
    vlr->data = NULL;
    vlr->record_len = 0;
  }
}

typedef struct {
  uint8_t version_major;
  uint8_t version_minor;
  uint64_t point_count;
  uint16_t point_size;
  uint32_t offset_to_point_data;

  uint32_t number_of_vlrs;
  las_vlr *vlrs;
} las_header;

void las_clean_header(las_header *header) {
  if (header->number_of_vlrs != 0 && header->vlrs != NULL) {
    free(header->vlrs);
    header->vlrs = NULL;
    header->number_of_vlrs = 0;
  }
}
typedef enum {
  las_error_ok = 0,
  las_error_io,
  las_error_oom,
  las_error_other
} las_error;

las_error fread_las_header(FILE *file, las_header *header) {
  if (header == NULL) {
    return las_error_other;
  }

  uint8_t raw_header[LAS_HEADER_SIZE];
  size_t num_read = fread(raw_header, sizeof(uint8_t), LAS_HEADER_SIZE, file);
  if (num_read < LAS_HEADER_SIZE && ferror(file) != 0) {
    return las_error_io;
  }

  if (strncmp((const char *)raw_header, "LASF", 4) != 0) {
    int length = 4;
    printf("%*.*s", length, length, (const char *)raw_header);
    fprintf(stderr, "Invalid file signature\n");
    return las_error_other;
  }

  header->version_major = raw_header[24];
  header->version_minor = raw_header[25];
  header->offset_to_point_data = *(uint32_t *)(raw_header + 96);
  header->number_of_vlrs = *(uint32_t *)(raw_header + 100);
  header->point_size = *(uint16_t *)(raw_header + 105);
  header->point_count = *(uint32_t *)(raw_header + 107);

  uint16_t header_size = *(uint16_t *)(raw_header + 94);
  if (fseek(file, header_size, SEEK_SET) != 0) {
    return las_error_io;
  }

  header->vlrs = malloc(header->number_of_vlrs * sizeof(las_vlr));
  if (header->vlrs == NULL) {
    return las_error_oom;
  }

  uint8_t raw_vlr_header[LAS_VLR_HEADER_SIZE];
  for (size_t i = 0; i < header->number_of_vlrs; ++i) {
    num_read =
        fread(raw_vlr_header, sizeof(uint8_t), LAS_VLR_HEADER_SIZE, file);
    if (num_read < LAS_VLR_HEADER_SIZE && ferror(file) != 0) {
      return las_error_io;
    }

    las_vlr *vlr = &header->vlrs[i];
    memcpy(vlr->user_id, raw_vlr_header + 2, sizeof(uint8_t) * 16);
    vlr->record_id = *(uint16_t *)(raw_vlr_header + 18);
    vlr->record_len = *(uint16_t *)(raw_vlr_header + 20);
    vlr->data = malloc(sizeof(uint8_t) * vlr->record_len);
    if (vlr->data == NULL) {
      return las_error_oom;
    }

    num_read = fread(vlr->data, sizeof(uint8_t), vlr->record_len, file);
    if (num_read < vlr->record_len && ferror(file)) {
      return las_error_io;
    }
  }

  //  fseek(file, header->offset_to_point_data, SEEK_SET);

  return las_error_ok;
}

const las_vlr *find_laszip_vlr(const las_header *las_header) {
  const las_vlr *vlr = NULL;

  for (uint16_t i = 0; i < las_header->number_of_vlrs; ++i) {
    const las_vlr *current = &las_header->vlrs[i];
    if (strcmp(current->user_id, "laszip encoded") == 0 &&
        current->record_id == 22204) {
      vlr = current;
      break;
    }
  }
  return vlr;
}

void print_vlrs(const las_header *header) {
  printf("Number of vlrs: %d\n", header->number_of_vlrs);
  for (uint32_t i = 0; i < header->number_of_vlrs; ++i) {
    las_vlr *vlr = &header->vlrs[i];
    printf("user_id: %s, record_id: %d, data len: %d\n", vlr->user_id,
           vlr->record_id, vlr->record_len);
  }
}
#endif // TEST_LASRSC_LAS_H
