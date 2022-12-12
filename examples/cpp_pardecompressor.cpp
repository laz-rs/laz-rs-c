#include <lazrs/lazrs_cpp.h>
#include <minilas/las.h>

#include <chrono>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <vector>

void decompress_points(std::string fname,
                       las_file_t *las_file,
                       const las_vlr *laszip_vlr,
                       bool parallel)
{
    lazrs::LasZipDecompressor decompressor(fname,
                                           laszip_vlr->data,
                                           laszip_vlr->record_len,
                                           las_file->header.offset_to_point_data,
                                           parallel);
    std::vector<uint8_t> point_data(
        las_file->header.point_size * las_file->header.point_count * sizeof(uint8_t), 0);
    decompressor.decompress_many(point_data.data(),
                                 las_file->header.point_size * las_file->header.point_count *
                                     sizeof(uint8_t));
    printf("Decompressed %" PRIu64 "points\n", las_file->header.point_count);
}

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
        {
            std::chrono::steady_clock::time_point begin = std::chrono::steady_clock::now();
            decompress_points(argv[1], las_file, laszip_vlr, true);
            std::chrono::steady_clock::time_point end = std::chrono::steady_clock::now();
            std::cout
                << "Parallel decompression done in: "
                << (std::chrono::duration_cast<std::chrono::microseconds>(end - begin).count()) /
                       1000000.0
                << "[s]" << std::endl;
        }

        {
            std::chrono::steady_clock::time_point begin = std::chrono::steady_clock::now();
            decompress_points(argv[1], las_file, laszip_vlr, false);
            std::chrono::steady_clock::time_point end = std::chrono::steady_clock::now();
            std::cout
                << "Single-thread decompression done in: "
                << (std::chrono::duration_cast<std::chrono::microseconds>(end - begin).count()) /
                       1000000.0
                << "[s]" << std::endl;
        }
    }
    catch (const std::exception &exception)
    {
        std::cerr << exception.what() << '\n';
    }

    las_file_close(las_file);
    return EXIT_SUCCESS;
}
