#include <lazrs/lazrs_cpp.h>
#include <stdexcept>

namespace lazrs
{
LasZipDecompressor::LasZipDecompressor(const uint8_t *data,
                                       size_t size,
                                       const uint8_t *laszip_vlr_record_data,
                                       uint16_t record_data_len)
    : m_decompressor(nullptr, lazrs_decompressor_delete)
{
    Lazrs_LasZipDecompressor *decompressor;
    Lazrs_Result result = lazrs_decompressor_new_buffer(
        &decompressor, data, size, laszip_vlr_record_data, record_data_len);
    if (result != LAZRS_OK)
    {
        throw std::runtime_error("Failed to create decompressor");
    }
    m_decompressor.reset(decompressor);
}

void LasZipDecompressor::decompress_one(uint8_t *out, size_t len, Lazrs_Result &result)
{
    result = lazrs_decompressor_decompress_one(m_decompressor.get(), out, len);
}

void LasZipDecompressor::decompress_one(uint8_t *out, size_t len)
{
    if (lazrs_decompressor_decompress_one(m_decompressor.get(), out, len) != LAZRS_OK)
    {
        throw std::runtime_error("decompression failed");
    }
}

void LasZipDecompressor::decompress_many(uint8_t *out, size_t len)
{
    if (lazrs_decompressor_decompress_many(m_decompressor.get(), out, len) != LAZRS_OK)
    {
        throw std::runtime_error("decompression failed");
    }
}

void LasZipDecompressor::decompress_many(uint8_t *out, size_t len, Lazrs_Result &result)
{
    result = lazrs_decompressor_decompress_many(m_decompressor.get(), out, len);
}

void LasZipCompressor::compress_one(uint8_t *in, size_t len, Lazrs_Result &result)
{
    result = lazrs_compressor_compress_one(m_compressor.get(), in, len);
}

void LasZipCompressor::compress_one(uint8_t *in, size_t len)
{
    if (lazrs_compressor_compress_one(m_compressor.get(), in, len) != LAZRS_OK)
    {
        throw std::runtime_error("compression failed");
    }
}

void LasZipCompressor::compress_many(uint8_t *in, size_t len, Lazrs_Result &result)
{
    result = lazrs_compressor_compress_many(m_compressor.get(), in, len);
}

void LasZipCompressor::compress_many(uint8_t *in, size_t len)
{
    if (lazrs_compressor_compress_many(m_compressor.get(), in, len) != LAZRS_OK)
    {
        throw std::runtime_error("compression failed");
    }
}

} // namespace lazrs