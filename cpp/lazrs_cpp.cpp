#include <lazrs/lazrs_cpp.h>
#include <stdexcept>

namespace lazrs
{
LasZipDecompressor::LasZipDecompressor(std::string fname,
                                       const uint8_t *laszip_vlr_record_data,
                                       uint16_t record_data_len,
                                       uint64_t point_offset,
                                       bool parallel)
    : m_decompressor(nullptr, lazrs_decompressor_delete)
{
    Lazrs_LasZipDecompressor *decompressor;
    Lazrs_DecompressorParams params;
    params.source_type = LAZRS_SOURCE_FNAME;
    params.source.buffer.data = reinterpret_cast<const uint8_t*>(&fname[0]);
    params.source.buffer.len = fname.size();
    params.laszip_vlr.data = laszip_vlr_record_data;
    params.laszip_vlr.len = record_data_len;
    params.source_offset = point_offset;
    Lazrs_Result result = lazrs_decompressor_new(&decompressor, params, parallel);
    if (result != LAZRS_OK)
    {
        throw std::runtime_error("Failed to create sequential");
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