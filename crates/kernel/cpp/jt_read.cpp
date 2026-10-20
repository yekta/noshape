// JT import through its exact geometry: the XT B-rep segments (Parasolid),
// as Onshape's translator reads JT. Header and table of contents (32-bit
// offsets in JT 8/9, 64-bit from JT 10), then each type-17 segment: a
// segment header, the compression prefix (flag, length, algorithm: 1 none,
// 2 zlib, 3 LZMA/XZ), and the element holding the neutral binary transmit
// stream. Parts come unplaced (the LSG's instance transforms are not read).

#include "internal.hpp"
#include "pvz_reader.hpp"

#include <cstring>
#include <fstream>
#include <iterator>

namespace noshape {

std::string xz_decompress(const unsigned char* in, size_t len); // xz_read.cpp

namespace {

std::string read_file(const std::string& path)
{
  std::ifstream in(path, std::ios::binary);
  if (!in)
    fail_io("cannot read " + path);
  return std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
}

struct Reader
{
  const std::string& d;
  bool               big = false;
  uint64_t           u(size_t at, int n) const
  {
    if (at + (size_t)n > d.size())
      fail_invalid("jt: truncated file");
    uint64_t v = 0;
    for (int i = 0; i < n; ++i)
    {
      const uint64_t b = (unsigned char)d[at + (size_t)i];
      v |= big ? b << (8 * (n - 1 - i)) : b << (8 * i);
    }
    return v;
  }
};

// The segment's payload, decompressed.
std::string payload(const std::string& d, size_t at, size_t len, const Reader& r)
{
  if (at + 24 > d.size() || len < 24 || at + len > d.size())
    fail_invalid("jt: bad segment");
  const size_t   p    = at + 24; // after GUID, type, length
  const uint32_t flag = (uint32_t)r.u(p, 4);
  if (flag != 2 && flag != 3)
    return d.substr(p, len - 24);
  const size_t clen = (size_t)r.u(p + 4, 4);
  const int    alg  = (unsigned char)d.at(p + 8);
  if (clen < 1 || p + 8 + clen > at + len)
    fail_invalid("jt: bad compressed segment");
  const auto* c = (const unsigned char*)d.data() + p + 9;
  const size_t n = clen - 1;
  if (alg == 1)
    return std::string((const char*)c, n);
  if (alg == 2)
  {
    if (n < 2)
      fail_invalid("jt: bad zlib segment");
    try
    {
      return pvz::inflate_raw(c + 2, n - 2);
    }
    catch (const std::runtime_error& e)
    {
      fail_invalid(std::string("jt: ") + e.what());
    }
  }
  if (alg == 3)
    return xz_decompress(c, n);
  fail_unsupported("jt: compression " + std::to_string(alg));
}

} // namespace

std::vector<XtPart> read_jt_parts(const std::string& path)
{
  const std::string d = read_file(path);
  if (d.compare(0, 8, "Version ") != 0 || d.size() < 105)
    fail_invalid("jt: not a JT file");
  const int major = std::atoi(d.c_str() + 8);
  Reader    r{d, d[80] != 0};
  const bool wide = major >= 10;
  const size_t toc = (size_t)r.u(85, wide ? 8 : 4);
  const size_t n   = (size_t)r.u(toc, 4);
  const size_t entry = wide ? 32 : 28;
  if (n > 1000000 || toc + 4 + n * entry > d.size())
    fail_invalid("jt: bad table of contents");
  std::vector<XtPart> out;
  for (size_t i = 0; i < n; ++i)
  {
    const size_t   e    = toc + 4 + i * entry;
    const size_t   off  = (size_t)r.u(e + 16, wide ? 8 : 4);
    const size_t   len  = (size_t)r.u(e + 16 + (wide ? 8 : 4), 4);
    const uint32_t attr = (uint32_t)r.u(e + entry - 4, 4);
    if ((attr >> 24) != 17)
      continue;
    const std::string seg = payload(d, off, len, r);
    // The element: its neutral binary transmit data starts "PS\0\0", after
    // its length.
    const size_t ps = seg.find(std::string("PS\0\0", 4));
    if (ps == std::string::npos || ps < 4)
      continue;
    std::vector<XtPart> parts = read_xt_data(seg.substr(ps));
    out.insert(out.end(), parts.begin(), parts.end());
  }
  if (out.empty())
    fail_unsupported("jt: the file has no exact (XT B-rep) geometry, only tessellation");
  return out;
}

} // namespace noshape
