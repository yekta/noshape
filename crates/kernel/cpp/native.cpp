// Native CAD files, read as Onshape's translators read them: the exact
// geometry these systems embed. SolidWorks keeps each configuration's bodies
// as a zlib-compressed Parasolid partition ("Contents/Config-N-Partition", a
// stream of an OLE2 compound file through 2014, a section of its own
// container since 2015); Solid Edge keeps a Parasolid transmit file
// in "PARASOLID/STREAM0.P_B"; NX keeps one (zlib-compressed since NX 9) in its
// part stream ("UG_PART/UG_PART" of an OLE2 file, or a whole "SPLMSSTR"
// file). All go through the Parasolid reader. Inventor keeps ShapeManager
// (ACIS) data in its B-rep segment: read by the ACIS reader.

#include "internal.hpp"
#include "pvz_reader.hpp"

#include <cstring>
#include <fstream>
#include <iterator>
#include <map>

namespace noshape {

namespace {

// ---------------------------------------------------------------- compound files (OLE2 / CFB)

class Compound
{
public:
  explicit Compound(std::string data) : d(std::move(data))
  {
    static const unsigned char magic[8] = {0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1};
    if (d.size() < 512 || std::memcmp(d.data(), magic, 8) != 0)
      fail_invalid("not a compound file");
    sector = 1u << u16(30);
    mini   = 1u << u16(32);
    if (sector < 512 || sector > 65536 || mini > sector)
      fail_invalid("compound file: bad sector size");
    const uint32_t nfat = u32(44), dir_start = u32(48), mini_cutoff = u32(56), minifat_start = u32(60), nminifat = u32(64);
    uint32_t       difat_start = u32(68), ndifat = u32(72);
    cutoff                     = mini_cutoff;
    // FAT sectors: 109 in the header, the rest in DIFAT sectors.
    std::vector<uint32_t> fat_sectors;
    for (uint32_t i = 0; i < 109 && fat_sectors.size() < nfat; ++i)
      fat_sectors.push_back(u32(76 + 4 * i));
    for (uint32_t k = 0; k < ndifat && difat_start < 0xFFFFFFFA; ++k)
    {
      const size_t off = offset(difat_start);
      for (uint32_t i = 0; i + 1 < sector / 4 && fat_sectors.size() < nfat; ++i)
        fat_sectors.push_back(u32(off + 4 * i));
      difat_start = u32(off + sector - 4);
    }
    for (uint32_t s : fat_sectors)
    {
      const size_t off = offset(s);
      for (uint32_t i = 0; i < sector / 4; ++i)
        fat.push_back(u32(off + 4 * i));
    }
    // Directory entries (128 bytes each).
    const std::string dir = chain(dir_start, 0, false);
    for (size_t at = 0; at + 128 <= dir.size(); at += 128)
    {
      Entry e;
      const int nlen = (int)le16(dir, at + 64);
      for (int i = 0; i + 2 < nlen && i < 64; i += 2) // nlen counts the terminating 0
        e.name += (char)le16(dir, at + (size_t)i); // ASCII names (UTF-16LE)
      e.type  = (unsigned char)dir[at + 66];
      e.left  = le32(dir, at + 68);
      e.right = le32(dir, at + 72);
      e.child = le32(dir, at + 76);
      e.start = le32(dir, at + 116);
      e.size  = le32(dir, at + 120);
      entries.push_back(e);
    }
    if (entries.empty())
      fail_invalid("compound file: no directory");
    // The mini stream (in the root entry) and the mini FAT.
    ministream = chain(entries[0].start, entries[0].size, false);
    const std::string mf = chain(minifat_start, (uint64_t)nminifat * sector, false);
    for (size_t i = 0; i + 4 <= mf.size(); i += 4)
      minifat.push_back(le32(mf, i));
    // Full paths.
    name_tree(entries[0].child, "");
  }

  // The stream at `path` ("Contents/Config-0-Partition"), if there is one.
  bool stream(const std::string& path, std::string& out) const
  {
    auto it = paths.find(path);
    if (it == paths.end())
      return false;
    const Entry& e = entries[it->second];
    out            = e.size < cutoff ? chain(e.start, e.size, true) : chain(e.start, e.size, false);
    return true;
  }
  std::vector<std::string> stream_paths() const
  {
    std::vector<std::string> v;
    for (const auto& [p, i] : paths)
      v.push_back(p);
    return v;
  }

private:
  struct Entry
  {
    std::string name;
    int         type = 0;
    uint32_t    left = 0xFFFFFFFF, right = 0xFFFFFFFF, child = 0xFFFFFFFF, start = 0;
    uint64_t    size = 0;
  };
  std::string                     d, ministream;
  uint32_t                        sector = 512, mini = 64, cutoff = 4096;
  std::vector<uint32_t>           fat, minifat;
  std::vector<Entry>              entries;
  std::map<std::string, size_t>   paths;

  static uint32_t le32(const std::string& s, size_t at)
  {
    if (at + 4 > s.size())
      fail_invalid("compound file: truncated");
    const auto* b = (const unsigned char*)s.data() + at;
    return b[0] | (b[1] << 8) | (b[2] << 16) | ((uint32_t)b[3] << 24);
  }
  static uint32_t le16(const std::string& s, size_t at)
  {
    if (at + 2 > s.size())
      fail_invalid("compound file: truncated");
    const auto* b = (const unsigned char*)s.data() + at;
    return b[0] | (b[1] << 8);
  }
  uint32_t u32(size_t at) const { return le32(d, at); }
  uint32_t u16(size_t at) const { return le16(d, at); }
  size_t   offset(uint32_t s) const
  {
    const size_t off = (size_t)(s + 1) * sector;
    if (off + sector > d.size() + sector) // the last sector may be short
      fail_invalid("compound file: sector out of range");
    return off;
  }

  // The bytes of a sector chain (the mini stream's when `small`), cut to size (0 = whole chain).
  std::string chain(uint32_t s, uint64_t size, bool small) const
  {
    std::string out;
    const auto& table = small ? minifat : fat;
    const uint32_t unit = small ? mini : sector;
    size_t      guard = 0;
    while (s < 0xFFFFFFFA && (size == 0 || out.size() < size))
    {
      if (small)
      {
        const size_t off = (size_t)s * unit;
        if (off + unit > ministream.size() + unit)
          fail_invalid("compound file: mini sector out of range");
        out.append(ministream, off, std::min<size_t>(unit, ministream.size() - std::min(off, ministream.size())));
      }
      else
      {
        const size_t off = offset(s);
        out.append(d, off, std::min<size_t>(unit, d.size() - std::min(off, d.size())));
      }
      if (s >= table.size() || ++guard > table.size() + 1)
        break;
      s = table[s];
    }
    if (size && out.size() > size)
      out.resize((size_t)size);
    return out;
  }

  void name_tree(uint32_t i, const std::string& prefix)
  {
    if (i >= entries.size())
      return;
    std::vector<uint32_t> stack{i};
    std::vector<bool>     seen(entries.size(), false);
    while (!stack.empty())
    {
      const uint32_t k = stack.back();
      stack.pop_back();
      if (k >= entries.size() || seen[k])
        continue;
      seen[k]        = true;
      const Entry& e = entries[k];
      const std::string p = prefix.empty() ? e.name : prefix + "/" + e.name;
      if (e.type == 2)
        paths[p] = k;
      else if (e.type == 1)
        name_tree(e.child, p);
      stack.push_back(e.left);
      stack.push_back(e.right);
    }
  }
};

std::string read_file(const std::string& path)
{
  std::ifstream in(path, std::ios::binary);
  if (!in)
    fail_io("cannot read " + path);
  return std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
}

// A SolidWorks partition stream: u32, a 16-byte marker, u32 inflated size,
// u32 compressed size, then a zlib stream (2-byte header, raw deflate).
std::string solidworks_inflate(const std::string& s)
{
  if (s.size() < 30)
    fail_invalid("solidworks: short partition stream");
  const auto*    b    = (const unsigned char*)s.data();
  const uint32_t clen = b[24] | (b[25] << 8) | (b[26] << 16) | ((uint32_t)b[27] << 24);
  const size_t   len  = std::min<size_t>(clen, s.size() - 28);
  if (len < 2)
    fail_invalid("solidworks: empty partition stream");
  try
  {
    return pvz::inflate_raw(b + 30, len - 2);
  }
  catch (const std::runtime_error& e)
  {
    fail_invalid(std::string("solidworks: ") + e.what());
  }
}

// A configuration's partition stream name: "Contents/Config-<n>-Partition".
bool partition_name(const std::string& n)
{
  static const std::string head = "Contents/Config-", tail = "-Partition";
  if (n.size() <= head.size() + tail.size() || n.compare(0, head.size(), head) != 0 || n.compare(n.size() - tail.size(), tail.size(), tail) != 0)
    return false;
  for (size_t i = head.size(); i < n.size() - tail.size(); ++i)
    if (n[i] < '0' || n[i] > '9')
      return false;
  return true;
}

// Of a file's partition streams, Config-0's, else the first.
const std::string* pick_partition(const std::vector<std::pair<std::string, std::string>>& parts)
{
  for (const auto& [n, d] : parts)
    if (n == "Contents/Config-0-Partition")
      return &d;
  return parts.empty() ? nullptr : &parts.front().second;
}

// The partition streams of a SolidWorks 2015+ file: a run of sections, each
// a marker (14 00 06 00 08 00), two words (a checksum), the compressed and
// inflated sizes, the name length, the name (each byte's nibbles swapped),
// then raw deflate. The marker also occurs inside data, so a section counts
// only when it inflates to its size.
std::vector<std::pair<std::string, std::string>> solidworks_partitions(const std::string& d)
{
  static const std::string mark("\x14\x00\x06\x00\x08\x00", 6);
  const auto u32 = [&](size_t at) {
    const auto* b = (const unsigned char*)d.data() + at;
    return b[0] | (b[1] << 8) | (b[2] << 16) | ((uint32_t)b[3] << 24);
  };
  std::vector<std::pair<std::string, std::string>> out;
  for (size_t i = d.find(mark); i != std::string::npos && i + 26 <= d.size(); i = d.find(mark, i + 1))
  {
    const uint32_t csize = u32(i + 14), usize = u32(i + 18), nlen = u32(i + 22);
    const size_t   start = i + 26 + nlen;
    if (nlen > 256 || start > d.size() || csize > d.size() - start)
      continue;
    std::string name;
    for (size_t k = i + 26; k < start; ++k)
    {
      const auto c = (unsigned char)d[k];
      name += (char)(((c >> 4) | (c << 4)) & 0xff);
    }
    if (!partition_name(name))
      continue;
    try
    {
      std::string x = pvz::inflate_raw((const unsigned char*)d.data() + start, csize);
      if (x.size() == usize)
        out.emplace_back(name, std::move(x));
    }
    catch (const std::runtime_error&)
    {
    }
  }
  return out;
}

// The Parasolid partition NX embeds in a part file: a transmit file's text
// header, then (after an optional '+') the binary data, zlib-compressed in
// newer files. Empty when there is none.
std::string nx_transmit(const std::string& s)
{
  const size_t h = s.find("**ABCDEFGHIJ");
  if (h == std::string::npos)
    return {};
  const size_t e = s.find("**END_OF_HEADER", h);
  if (e == std::string::npos)
    return {};
  size_t at = s.find('\n', e);
  if (at == std::string::npos)
    return {};
  ++at;
  std::string out = s.substr(h, at - h);
  if (at < s.size() && s[at] == '+')
    ++at;
  if (s.compare(at, 4, std::string("PS\0\0", 4)) == 0)
    return out + s.substr(at);
  // zlib: CMF 0x78, a header check, then raw deflate.
  const auto* b = (const unsigned char*)s.data() + at;
  if (at + 2 < s.size() && b[0] == 0x78 && ((b[0] << 8) | b[1]) % 31 == 0)
  {
    try
    {
      return out + pvz::inflate_raw(b + 2, s.size() - at - 2);
    }
    catch (const std::runtime_error& err)
    {
      fail_invalid(std::string("nx: ") + err.what());
    }
  }
  return {};
}

} // namespace

std::vector<XtPart> read_native_parts(const std::string& path)
{
  std::string data = read_file(path);
  // NX (since NX 9, "SPLMSSTR" files): the part stream is stored whole.
  if (data.compare(0, 8, "SPLMSSTR") == 0)
  {
    std::string x = nx_transmit(data);
    if (x.empty())
      fail_unsupported("this NX file has no Parasolid geometry (an assembly's parts are separate files)");
    return read_xt_data(std::move(x));
  }
  std::string s;
  // SolidWorks 2015 and later: not a compound file.
  static const unsigned char ole[4] = {0xD0, 0xCF, 0x11, 0xE0};
  if (data.size() >= 4 && std::memcmp(data.data(), ole, 4) != 0)
  {
    const auto parts = solidworks_partitions(data);
    if (const std::string* p = pick_partition(parts))
      return read_xt_data(solidworks_inflate(*p));
    fail_unsupported("this native CAD file has no geometry noshape can read yet");
  }
  Compound cf(std::move(data));
  // SolidWorks through 2014.
  std::vector<std::pair<std::string, std::string>> parts;
  for (const std::string& n : cf.stream_paths())
    if (partition_name(n) && cf.stream(n, s))
      parts.emplace_back(n, s);
  if (const std::string* p = pick_partition(parts))
    return read_xt_data(solidworks_inflate(*p));
  // Solid Edge: a Parasolid transmit file.
  if (cf.stream("PARASOLID/STREAM0.P_B", s))
    return read_xt_data(s.substr(std::min(s.find("**ABCDEFGHIJ"), s.size())));
  // Inventor: the B-rep segment's ShapeManager (ACIS) data. Each segment
  // has a meta stream "RSeStorage/M<id>" naming it and a data stream
  // "RSeStorage/B<id>" (a header, then zlib).
  for (const std::string& n : cf.stream_paths())
  {
    static const std::string meta = "RSeStorage/M";
    if (n.compare(0, meta.size(), meta) != 0 || !cf.stream(n, s))
      continue;
    static const std::string seg = std::string("P\0m\0B\0R\0e\0p\0S\0e\0g\0m\0e\0n\0t\0", 26); // "PmBRepSegment", UTF-16
    if (s.find(seg) == std::string::npos)
      continue;
    std::string data;
    if (!cf.stream("RSeStorage/B" + n.substr(meta.size()), data))
      continue;
    for (size_t z = 0; z + 2 < std::min<size_t>(data.size(), 64); ++z)
    {
      const auto* b = (const unsigned char*)data.data() + z;
      if (b[0] != 0x78 || ((b[0] << 8) | b[1]) % 31 != 0)
        continue;
      std::string brep;
      try
      {
        brep = pvz::inflate_raw(b + 2, data.size() - z - 2);
      }
      catch (const std::runtime_error&)
      {
        continue;
      }
      size_t at = brep.find("ASM BinaryFile");
      if (at == std::string::npos)
        at = brep.find("ACIS BinaryFile");
      if (at != std::string::npos)
        return read_sat_data(brep.substr(at));
    }
  }
  // NX before NX 9 (and Unigraphics).
  if (cf.stream("UG_PART/UG_PART", s))
  {
    std::string x = nx_transmit(s);
    if (x.empty())
      fail_unsupported("this NX file has no Parasolid geometry (an assembly's parts are separate files)");
    return read_xt_data(std::move(x));
  }
  fail_unsupported("this native CAD file has no geometry noshape can read yet");
}

} // namespace noshape
