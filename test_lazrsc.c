#include <lazrs/lazrs.h>
#include <lazrs/las.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>



int main(int argc, char *argv[]) {
  if (argc < 2) {
    printf("Usage: %s file.laz\n", argv[0]);
    return EXIT_FAILURE;
  }

  FILE *file = fopen(argv[1], "rb");
  if (file == NULL) {
    perror("fopen");
    return EXIT_FAILURE;
  }

  las_header header;
  las_error error;

  error = fread_las_header(file, &header);
  if (error != las_error_ok) {
    fprintf(stderr, "Error reading header\n");
    fclose(file);
    return EXIT_FAILURE;
  }
  printf("Version: %d.%d\n", (int)header.version_major,
         (int)header.version_minor);
  printf("Point size: %d, point count: %llu\n", header.point_size,
         header.point_count);
  print_vlrs(&header);

  if (header.version_minor != 2) {
    fprintf(stderr, "version not supported\n");
    fclose(file);
    las_clean_header(&header);
    return EXIT_FAILURE;
  }

  const las_vlr *laszip_vlr = find_laszip_vlr(&header);
  if (laszip_vlr == NULL) {
    fprintf(stderr, "No laszip vlr found\n");
    fclose(file);
    las_clean_header(&header);
    return EXIT_FAILURE;
  }

  LasZipDecompressor *decompressor = NULL;
  LazrsResult result = lazrs_decompressor_new_file(
      &decompressor,
      file,
      laszip_vlr->data,
      laszip_vlr->record_len
  );
  if (result != LAZRS_OK) {
    fprintf(stderr, "Failed to create the decompressor");
    goto main_exit;
  }

  uint8_t *point_data = malloc(header.point_size * sizeof(uint8_t));
  if (point_data == NULL) {
    fprintf(stderr, "OOM\n");
    goto main_exit;
  }

  for (size_t i = 0; i < header.point_count; ++i) {
    if (lazrs_decompress_one(decompressor, point_data, header.point_size) !=
        LAZRS_OK) {
      goto main_exit;
    }
    if (ferror(file)) {
      perror("error ");
      goto main_exit;
    }
  }
  printf("Decompressed %llu points\n", header.point_count);
main_exit:
  lazrs_delete_decompressor(decompressor);
  las_clean_header(&header);
  fclose(file);
  return EXIT_SUCCESS;
}
