#include <lazrs/lazrs.h>
#include <lazrs/las.h>

int main(int argc, char *argv[]) {
  if (argc < 3) {
    printf("USAGE: %s INPUT_LAS_FILE OUTPUT_LAZ\n", argv[0]);
    return EXIT_FAILURE;
  }

  FILE *las_file = fopen(argv[1], "r");
  if (las_file == NULL) {
    perror("fopen");
    return EXIT_FAILURE;
  }

  FILE *file = fopen(argv[2], "w");
  if (file == NULL) {
    fclose(las_file);
    perror("fopen");
    return EXIT_FAILURE;
  }

  las_header header;
  if (fread_las_header(las_file, &header) != las_error_ok)
  {
    goto end;
  }


  LasZipCompressor *compressor = NULL;
  LazrsResult result =
      lazrs_compress_new_for_point_format(&compressor, file, 3, 0);

  if (result != LAZRS_OK) {
    goto end;
  }

  result = lazrs_compressor_done(compressor);
end:
  lazrs_compressor_delete(compressor);
  fclose(file);
  fclose(las_file);
  return (result == LAZRS_OK) ? EXIT_SUCCESS : EXIT_FAILURE;
}