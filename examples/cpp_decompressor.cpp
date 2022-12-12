#include <lazrs/lazrs_cpp.h>
#include <minilas/las.h>

#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <vector>

int main(int argc, char *argv[])
{
    if (argc < 2)
    {
        printf("Usage: %s file.laz\n", argv[0]);
        return EXIT_FAILURE;
    }

    las_file_t *las_file = las_file_open(argv[1]);
    print_header(&las_file->header);

    const las_vlr *laszip_vlr = find_laszip_vlr(&las_file->header);
    if (laszip_vlr == nullptr)
    {
        fprintf(stderr, "No laszip vlr found\n");
        las_file_close(las_file);
        return EXIT_FAILURE;
    }

    try
    {
        lazrs::LasZipDecompressor decompressor(argv[1],
                                               laszip_vlr->data,
                                               laszip_vlr->record_len,
                                               las_file->header.offset_to_point_data);
        std::vector<uint8_t> point_data(las_file->header.point_size * sizeof(uint8_t), 0);
        for (size_t i{0}; i < las_file->header.point_count; ++i)
        {
            decompressor.decompress_one(point_data.data(),
                                        las_file->header.point_size * sizeof(uint8_t));
        }
        printf("Decompressed %" PRIu64 "points\n", las_file->header.point_count);
    }
    catch (const std::exception &exception)
    {
        std::cerr << exception.what() << '\n';
    }

    las_file_close(las_file);
    return EXIT_SUCCESS;
}
