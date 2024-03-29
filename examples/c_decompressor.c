#include <lazrs/lazrs.h>
#include <minilas/las.h>

#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char *argv[])
{
    if (argc < 2)
    {
        printf("Usage: %s file.laz\n", argv[0]);
        return EXIT_FAILURE;
    }

    las_file_t *las_file = NULL;
    las_file = las_file_open(argv[1]);
    print_header(&las_file->header);

    const las_vlr *laszip_vlr = find_laszip_vlr(&las_file->header);
    if (laszip_vlr == NULL)
    {
        fprintf(stderr, "No laszip vlr found\n");
        las_file_close(las_file);
        return EXIT_FAILURE;
    }

    uint8_t *point_data = NULL;
    Lazrs_LasZipDecompressor *decompressor = NULL;
    Lazrs_Result result;

    // Create our sequential, that will decompress directly from a file
    Lazrs_DecompressorParams params;
    params.laszip_vlr.data = laszip_vlr->data;
    params.laszip_vlr.len = laszip_vlr->record_len;
    params.source_type = LAZRS_SOURCE_CFILE;
    params.source.file = las_file->file;
    params.source_offset = las_file->header.offset_to_point_data;
    int prefer_parallel = false;
    result = lazrs_decompressor_new(params, prefer_parallel, &decompressor);

    if (result != LAZRS_OK)
    {
        fprintf(stderr, "Failed to create the sequential");
        goto main_exit;
    }

    // We will decompress points one-by-one into this buffer
    point_data = malloc(las_file->header.point_size * sizeof(uint8_t));
    if (point_data == NULL)
    {
        fprintf(stderr, "Out Of Memory\n");
        goto main_exit;
    }

    // Decompression loop
    for (size_t i = 0; i < las_file->header.point_count; ++i)
    {
        result = lazrs_decompressor_decompress_one(
            decompressor, point_data, las_file->header.point_size);
        if (result != LAZRS_OK)
        {
            fprintf(stderr, "Error when decompressing");
            goto main_exit;
        }
        if (ferror(las_file->file))
        {
            perror("error ");
            goto main_exit;
        }
    }
    printf("Decompressed %" PRIu64 "points\n", las_file->header.point_count);

main_exit:
    lazrs_decompressor_delete(decompressor);
    las_file_close(las_file);
    if (point_data)
    {
        free(point_data);
    }
    return (result == LAZRS_OK) ? EXIT_SUCCESS : EXIT_FAILURE;
}
