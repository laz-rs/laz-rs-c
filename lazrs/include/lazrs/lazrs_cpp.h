#include <lazrs/lazrs.h>

#include <memory>

namespace lazrs
{
class LasZipDecompressor
{
  public:
    LasZipDecompressor(const uint8_t *data,
                       size_t size,
                       const uint8_t *laszip_vlr_record_data,
                       uint16_t record_data_len,
                       uint64_t point_offset);

    void decompress_one(uint8_t *out, size_t len, Lazrs_Result &result);
    void decompress_one(uint8_t *out, size_t len);
    void decompress_many(uint8_t *out, size_t len, Lazrs_Result &result);
    void decompress_many(uint8_t *out, size_t len);

  private:
    using Lazrs_LasZipDecompressorPtr =
        std::unique_ptr<Lazrs_LasZipDecompressor, decltype(&lazrs_decompressor_delete)>;
    Lazrs_LasZipDecompressorPtr m_decompressor;
};

class LasZipCompressor
{
  public:
    void compress_one(uint8_t *out, size_t len, Lazrs_Result &result);
    void compress_one(uint8_t *out, size_t len);
    void compress_many(uint8_t *out, size_t len, Lazrs_Result &result);
    void compress_many(uint8_t *out, size_t len);

  private:
    using Lazrs_LasZipCompressorPtr =
        std::unique_ptr<Lazrs_LasZipCompressor, void (*)(Lazrs_LasZipCompressor *)>;
    Lazrs_LasZipCompressorPtr m_compressor;
};
} // namespace lazrs
