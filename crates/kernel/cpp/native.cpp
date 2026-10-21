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

#include <algorithm>
#include <cmath>
#include <cstring>
#include <filesystem>
#include <sstream>
#include <functional>
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

// A configuration's ghost partition: "Contents/Config-<n>-GhostPartition"
// (construction geometry: sketch points, curves, surfaces).
bool ghost_name(const std::string& n)
{
  static const std::string tail = "GhostPartition";
  return n.size() > tail.size() && n.compare(n.size() - tail.size(), tail.size(), tail) == 0 && partition_name(n.substr(0, n.size() - tail.size()) + "Partition");
}

// A configuration's own stream: "Contents/Config-<n>" (its features,
// visual properties).
bool config_name(const std::string& n)
{
  static const std::string head = "Contents/Config-";
  if (n.size() <= head.size() || n.compare(0, head.size(), head) != 0)
    return false;
  for (size_t i = head.size(); i < n.size(); ++i)
    if (n[i] < '0' || n[i] > '9')
      return false;
  return true;
}

// The part's colour in a configuration stream: its first visual properties
// record ("moVisualProperties_c") holds the base colour right after the class
// name (RGB bytes); an appearance it names (a UTF-16 string, 0x02, RGB)
// overrides it. Matches Onshape's import (live G38: Bell Crank (229,234,237),
// fluxpad (255,241,135), McMaster's "satinfinishstainlesssteel" (255,242,232)).
bool solidworks_colour(const std::string& config, std::array<double, 3>& rgb, double& opacity)
{
  static const std::string tag = "moVisualProperties_c";
  const size_t             at  = config.find(tag);
  if (at == std::string::npos || at + tag.size() + 3 > config.size())
    return false;
  const auto* b    = (const unsigned char*)config.data();
  size_t      base = at + tag.size();
  for (int k = 0; k < 3; ++k)
    rgb[(size_t)k] = b[base + (size_t)k] / 255.0;
  // Then (after the RGBA, 4 bytes and a second RGBA) two counted UTF-16
  // strings (ff fe ff <n> <n chars>; the second the material), then six
  // doubles: ambient, diffuse, specular, shininess, transparency, emission
  // (live G38e: the polycarb window's 0.125 = Onshape's opacity 223).
  {
    size_t i  = base + 12;
    bool   ok = true;
    for (int k = 0; k < 2 && ok; ++k)
    {
      ok = i + 4 <= config.size() && b[i] == 0xff && b[i + 1] == 0xfe && b[i + 2] == 0xff;
      if (ok)
        i += 4 + 2 * (size_t)b[i + 3];
    }
    if (ok && i + 48 <= config.size())
    {
      double t;
      std::memcpy(&t, config.data() + i + 32, 8);
      if (t >= 0 && t < 1)
        opacity = 1 - t;
    }
  }
  // A named appearance: ff fe ff <n> <n UTF-16 chars> 02 R G B.
  const size_t end = std::min(config.size(), base + 1024);
  for (size_t i = base; i + 4 < end; ++i)
  {
    if (b[i] != 0xff || b[i + 1] != 0xfe || b[i + 2] != 0xff)
      continue;
    const size_t n = b[i + 3], text = i + 4, after = text + 2 * n;
    if (n == 0 || after + 4 > config.size() || b[after] != 0x02)
      continue;
    bool printable = true;
    for (size_t c = 0; c < n && printable; ++c)
      printable = b[text + 2 * c] >= 0x20 && b[text + 2 * c] < 0x7f && b[text + 2 * c + 1] == 0;
    if (!printable)
      continue;
    for (int k = 0; k < 3; ++k)
      rgb[(size_t)k] = b[after + 1 + (size_t)k] / 255.0;
    break;
  }
  return true;
}

using Streams = std::vector<std::pair<std::string, std::string>>;

// Of a file's partition streams, the asked configuration's (an assembly's
// reference), else Config-0's, else the first.
const std::pair<std::string, std::string>* pick_partition(const Streams& parts, int config = -1)
{
  const std::pair<std::string, std::string>* first = nullptr;
  if (config >= 0)
    for (const auto& p : parts)
      if (p.first == "Contents/Config-" + std::to_string(config) + "-Partition")
        return &p;
  for (const auto& p : parts)
  {
    if (!partition_name(p.first))
      continue;
    if (p.first == "Contents/Config-0-Partition")
      return &p;
    if (!first)
      first = &p;
  }
  return first;
}

// A SolidWorks part as Onshape imports it: the configuration's bodies, then
// the curves (wire bodies with edges) of its ghost partition, which become
// "Curve 1…" (live G38d: the McMaster screw's helix; the ghost's points and
// sheets are not imported).
std::vector<XtPart> solidworks_parts(const Streams& parts, int config)
{
  const auto* main = pick_partition(parts, config);
  if (!main)
    fail_unsupported("this native CAD file has no geometry noshape can read yet");
  std::vector<XtPart> out    = read_xt_data(solidworks_inflate(main->second));
  const std::string   stream = main->first.substr(0, main->first.size() - std::strlen("-Partition"));
  const std::string   ghost  = stream + "-GhostPartition";
  for (const auto& [n, d] : parts)
    if (n == stream)
    {
      std::array<double, 3> rgb{};
      double                opacity = 1;
      if (solidworks_colour(d, rgb, opacity))
        for (XtPart& p : out)
          if (!p.has_colour)
          {
            p.colour     = rgb;
            p.has_colour = true;
            p.opacity    = opacity;
          }
    }
  for (const auto& [n, d] : parts)
    if (n == ghost)
      try
      {
        for (XtPart& p : read_xt_data(solidworks_inflate(d)))
        {
          ShapeIMap faces, edges;
          TopExp::MapShapes(p.shape, TopAbs_FACE, faces);
          TopExp::MapShapes(p.shape, TopAbs_EDGE, edges);
          if (faces.IsEmpty() && !edges.IsEmpty())
            out.push_back(std::move(p));
        }
      }
      catch (const std::exception&)
      {
      }
  return out;
}

// The partition streams of a SolidWorks 2015+ file: a run of sections, each
// a marker (14 00 06 00 08 00), two words (a checksum), the compressed and
// inflated sizes, the name length, the name (each byte's nibbles swapped),
// then raw deflate. The marker also occurs inside data, so a section counts
// only when it inflates to its size.
std::vector<std::pair<std::string, std::string>> solidworks_sections(const std::string& d, const std::function<bool(const std::string&)>& wanted)
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
    if (!wanted(name))
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

// A part file's partition, ghost partition and configuration streams.
std::vector<std::pair<std::string, std::string>> solidworks_partitions(const std::string& d)
{
  return solidworks_sections(d, [](const std::string& n) { return partition_name(n) || ghost_name(n) || config_name(n); });
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

namespace
{

// Solid Edge's part colour: the diffuse colour of its "Model Default" style
// (Display/Styles/Library0: two size words, then zlib; the style name in
// UTF-16, then 01 00 ff ff, ambient RGBA and diffuse RGBA floats). Live G38
// and G38e: Onshape's (129,136,140) on three files.
bool solid_edge_colour(Compound& cf, std::array<double, 3>& rgb)
{
  std::string s;
  if (!cf.stream("Display/Styles/Library0", s) || s.size() < 10 || (unsigned char)s[8] != 0x78)
    return false;
  std::string x;
  try
  {
    x = pvz::inflate_raw((const unsigned char*)s.data() + 10, s.size() - 10);
  }
  catch (const std::runtime_error&)
  {
    return false;
  }
  const std::string name = std::string("M\0o\0d\0e\0l\0 \0D\0e\0f\0a\0u\0l\0t\0", 26);
  const size_t      at   = x.find(name);
  if (at == std::string::npos)
    return false;
  const size_t mark = x.find(std::string("\x01\x00\xff\xff", 4), at + name.size());
  if (mark == std::string::npos || mark > at + name.size() + 64 || mark + 4 + 32 > x.size())
    return false;
  for (int k = 0; k < 3; ++k)
  {
    float f;
    std::memcpy(&f, x.data() + mark + 4 + 16 + 4 * k, 4);
    if (!(f >= 0 && f <= 1))
      return false;
    rgb[(size_t)k] = f;
  }
  return true;
}

std::vector<XtPart> native_parts_of(const std::string& path, int config);

} // namespace

std::vector<XtPart> read_native_parts(const std::string& path, int config)
{
  // The partitions' own face colours (SOLIDWORKS: the material's base
  // colour; NX: not its display colour) are not what Onshape imports: no
  // face of a native import has its own colour (live G38e).
  std::vector<XtPart> parts = native_parts_of(path, config);
  for (XtPart& p : parts)
    p.face_colours.clear();
  return parts;
}

namespace
{

std::vector<XtPart> native_parts_of(const std::string& path, int config)
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
    return solidworks_parts(solidworks_partitions(data), config);
  }
  Compound cf(std::move(data));
  // SolidWorks through 2014.
  Streams parts;
  for (const std::string& n : cf.stream_paths())
    if ((partition_name(n) || ghost_name(n) || config_name(n)) && cf.stream(n, s))
      parts.emplace_back(n, s);
  if (pick_partition(parts))
    return solidworks_parts(parts, config);
  // Solid Edge: a Parasolid transmit file.
  if (cf.stream("PARASOLID/STREAM0.P_B", s))
  {
    std::vector<XtPart>   parts = read_xt_data(s.substr(std::min(s.find("**ABCDEFGHIJ"), s.size())));
    std::array<double, 3> rgb{};
    if (solid_edge_colour(cf, rgb))
      for (XtPart& p : parts)
        if (!p.has_colour)
        {
          p.colour     = rgb;
          p.has_colour = true;
        }
    return parts;
  }
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

} // namespace


// ---- SOLIDWORKS assemblies

namespace
{

std::string xml_unescape(const std::string& v)
{
  std::string out;
  for (size_t i = 0; i < v.size(); ++i)
  {
    if (v[i] != '&')
    {
      out += v[i];
      continue;
    }
    const size_t e = v.find(';', i);
    if (e == std::string::npos)
    {
      out += v[i];
      continue;
    }
    const std::string ent = v.substr(i + 1, e - i - 1);
    if (ent == "amp")
      out += '&';
    else if (ent == "lt")
      out += '<';
    else if (ent == "gt")
      out += '>';
    else if (ent == "quot")
      out += '"';
    else if (ent == "apos")
      out += '\'';
    else
      out += "&" + ent + ";";
    i = e;
  }
  return out;
}

// The attributes of one tag (the text between '<' and '>').
std::map<std::string, std::string> xml_attrs(const std::string& t)
{
  std::map<std::string, std::string> a;
  size_t i = t.find_first_of(" \t\r\n");
  while (i != std::string::npos && i < t.size())
  {
    const size_t eq = t.find('=', i);
    if (eq == std::string::npos)
      break;
    const size_t q = t.find('"', eq);
    if (q == std::string::npos)
      break;
    const size_t q2 = t.find('"', q + 1);
    if (q2 == std::string::npos)
      break;
    std::string key = t.substr(i, eq - i);
    key.erase(0, key.find_first_not_of(" \t\r\n"));
    key.erase(key.find_last_not_of(" \t\r\n") + 1);
    a[key] = xml_unescape(t.substr(q + 1, q2 - q - 1));
    i = q2 + 1;
  }
  return a;
}

int to_int(const std::string& s, int dflt)
{
  try
  {
    return s.empty() ? dflt : std::stoi(s);
  }
  catch (const std::exception&)
  {
    return dflt;
  }
}

std::string file_name_of(const std::string& path)
{
  const size_t k = path.find_last_of("\\/");
  return k == std::string::npos ? path : path.substr(k + 1);
}

// The component instance tree an assembly file keeps as XML
// ("swXmlContents/COMPINSTANCETREE": in the 2015+ container, or an OLE stream
// "…__ZLB" with a 24-byte header before zlib).
std::string sldasm_xml(const std::string& path)
{
  std::string             data = read_file(path);
  static const unsigned char ole[4] = {0xD0, 0xCF, 0x11, 0xE0};
  if (data.size() >= 4 && std::memcmp(data.data(), ole, 4) != 0)
  {
    for (auto& [n, x] : solidworks_sections(data, [](const std::string& n) { return n == "swXmlContents/COMPINSTANCETREE"; }))
      return x;
    fail_unsupported("this SOLIDWORKS assembly has no component tree noshape can read");
  }
  Compound    cf(std::move(data));
  std::string s;
  if (cf.stream("swXmlContents/COMPINSTANCETREE", s))
    return s;
  if (cf.stream("swXmlContents/COMPINSTANCETREE__ZLB", s) && s.size() > 26 && (unsigned char)s[24] == 0x78)
    try
    {
      return pvz::inflate_raw((const unsigned char*)s.data() + 26, s.size() - 26);
    }
    catch (const std::runtime_error& e)
    {
      fail_invalid(std::string("solidworks: ") + e.what());
    }
  fail_unsupported("this SOLIDWORKS assembly has no component tree noshape can read");
}

} // namespace

std::vector<AsmNode> read_sldasm_tree(const std::string& path)
{
  const std::string xml = sldasm_xml(path);
  struct Model
  {
    std::string                                     conf;
    int                                             conf_id = -1, file = -1;
    std::vector<std::map<std::string, std::string>> refs;
  };
  std::map<int, std::pair<std::string, std::string>> files; // id -> (doc type, path)
  std::map<int, Model>                               models;
  int                                                root = -1, open = -1;
  for (size_t i = xml.find('<'); i != std::string::npos; i = xml.find('<', i + 1))
  {
    const size_t e = xml.find('>', i);
    if (e == std::string::npos)
      break;
    const std::string tag = xml.substr(i + 1, e - i - 1);
    if (tag.empty() || tag[0] == '?' || tag[0] == '!')
      continue;
    const bool   closing = tag[0] == '/';
    const size_t b       = closing ? 1 : 0;
    const std::string name = tag.substr(b, tag.find_first_of(" /\t\r\n", b) - b);
    if (closing)
    {
      if (name == "swModel")
        open = -1;
      continue;
    }
    const auto a = xml_attrs(tag);
    auto get = [&](const char* k) {
      const auto it = a.find(k);
      return it == a.end() ? std::string() : it->second;
    };
    if (name == "swFile")
      files[to_int(get("id"), -1)] = {get("swDocType"), get("swPath")};
    else if (name == "swModel")
    {
      const int id = to_int(get("id"), -1);
      Model&    m  = models[id];
      m.conf       = get("swConfigurationName");
      m.conf_id    = to_int(get("swConfigurationId"), -1);
      m.file       = to_int(get("swFileRef"), -1);
      if (tag.back() != '/')
        open = id;
    }
    else if (name == "swReference" && open >= 0)
      models[open].refs.push_back(a);
    else if (name == "swConfiguration" && get("swMostRecentConfiguration") == "YES" && root < 0)
      root = to_int(get("swModelRef"), -1);
  }
  if (root < 0 || !models.count(root))
    for (const auto& [id, m] : models)
      if (!m.refs.empty() && files.count(m.file) && files[m.file].first == "ASSEMBLY")
      {
        root = id;
        break;
      }
  if (root < 0)
    fail_invalid("solidworks: the assembly lists no components");
  std::vector<AsmNode> out;
  std::function<void(int, int, int)> walk = [&](int mid, int parent, int depth) {
    if (depth > 32 || !models.count(mid))
      return;
    const std::vector<std::map<std::string, std::string>> refs = models[mid].refs;
    for (const auto& a : refs)
    {
      auto get = [&](const char* k) {
        const auto it = a.find(k);
        return it == a.end() ? std::string() : it->second;
      };
      AsmNode n;
      n.parent         = parent;
      n.name           = get("swName");
      n.number         = to_int(get("swReferenceNumber"), 1);
      const int mref   = to_int(get("swModelRef"), -1);
      const Model& m   = models[mref];
      if (files.count(m.file))
      {
        n.file     = file_name_of(files[m.file].second);
        n.assembly = files[m.file].first == "ASSEMBLY";
      }
      n.configuration = m.conf.empty() ? get("swConfigurationName") : m.conf;
      n.config_id     = m.conf_id >= 0 ? m.conf_id : to_int(get("swConfigurationId"), -1);
      n.suppressed    = get("swSuppressed") == "YES";
      n.hidden        = get("swHidden") == "YES";
      n.exclude_bom   = get("swExcludeFromBOM") == "YES";
      std::istringstream ts(get("swTransform"));
      for (int k = 0; k < 16 && (ts >> n.m[(size_t)k]); ++k)
      {
      }
      out.push_back(n);
      if (n.assembly && mref != mid)
        walk(mref, (int)out.size() - 1, depth + 1);
    }
  };
  walk(root, -1, 0);
  return out;
}

// ---- zip archives (Onshape imports assemblies as a flat zip)

std::vector<std::string> extract_zip(const std::string& zip, const std::string& dir)
{
  const std::string d = read_file(zip);
  const auto        u16 = [&](size_t at) -> uint32_t {
    if (at + 2 > d.size())
      fail_invalid("zip: truncated");
    return (unsigned char)d[at] | ((unsigned char)d[at + 1] << 8);
  };
  const auto u32 = [&](size_t at) -> uint32_t { return u16(at) | (u16(at + 2) << 16); };
  size_t eocd = std::string::npos;
  for (size_t back = 22; back <= std::min<size_t>(d.size(), 22 + 65535); ++back)
    if (u32(d.size() - back) == 0x06054b50)
    {
      eocd = d.size() - back;
      break;
    }
  if (eocd == std::string::npos)
    fail_invalid("zip: not a zip archive");
  const uint32_t count = u16(eocd + 10);
  size_t         at    = u32(eocd + 16);
  std::vector<std::string> names;
  for (uint32_t i = 0; i < count; ++i)
  {
    if (u32(at) != 0x02014b50)
      fail_invalid("zip: bad central directory");
    const uint32_t method = u16(at + 10), csize = u32(at + 20), usize = u32(at + 24);
    const uint32_t nlen = u16(at + 28), xlen = u16(at + 30), clen = u16(at + 32), local = u32(at + 42);
    if (at + 46 + nlen > d.size())
      fail_invalid("zip: truncated");
    const std::string name = d.substr(at + 46, nlen);
    at += 46 + (size_t)nlen + xlen + clen;
    names.push_back(name);
    // Only plain files at the top level are extracted.
    if (name.empty() || name.find_first_of("/\\") != std::string::npos || name == "." || name == ".." || dir.empty())
      continue;
    if (u32(local) != 0x04034b50)
      fail_invalid("zip: bad local header");
    const size_t start = local + 30 + u16(local + 26) + u16(local + 28);
    if (start > d.size() || csize > d.size() - start)
      fail_invalid("zip: truncated");
    std::string data;
    if (method == 0)
      data = d.substr(start, csize);
    else if (method == 8)
      data = pvz::inflate_raw((const unsigned char*)d.data() + start, csize);
    else
      fail_unsupported("zip: compression method " + std::to_string(method));
    if (data.size() != usize)
      fail_invalid("zip: " + name + " has the wrong size");
    std::ofstream o(dir + "/" + name, std::ios::binary);
    if (!o)
      fail_io("cannot write " + dir + "/" + name);
    o << data;
  }
  return names;
}


// ---- Inventor assemblies (.iam)
//
// The occurrences' placements live in one of the assembly's RSe segments
// ("RSeStorage/B…": an 18-byte header, then zlib): a record per occurrence
// ending in UTF-16 "GRx" and the record's index, its placement right before
// that as a compact 4x4 (row-major, translation in cm, relative to the parent
// assembly): a flag word (low 16 bits: entries equal to 1, high 16 bits:
// entries equal to 0, both: −1), then every other entry as a double. The
// "UFRxDoc" stream lists, in the same order, a property set per occurrence:
// the referenced document's internal id (property 0x13, a GUID found in that
// document's own UFRxDoc) and its name (property 0x05).

namespace
{

uint32_t iam_u32(const std::string& d, size_t at)
{
  uint32_t v = 0;
  std::memcpy(&v, d.data() + at, 4);
  return v;
}

std::string iam_utf16(const std::string& ascii)
{
  std::string u;
  for (char c : ascii)
  {
    u += c;
    u += '\0';
  }
  return u;
}

// UTF-16LE (BMP) as UTF-8.
std::string iam_utf8(const std::string& d, size_t at, size_t chars)
{
  std::string out;
  for (size_t i = 0; i < chars && at + 2 * i + 1 < d.size(); ++i)
  {
    const unsigned c = (unsigned char)d[at + 2 * i] | ((unsigned char)d[at + 2 * i + 1] << 8);
    if (c < 0x80)
      out += (char)c;
    else if (c < 0x800)
    {
      out += (char)(0xC0 | (c >> 6));
      out += (char)(0x80 | (c & 0x3F));
    }
    else
    {
      out += (char)(0xE0 | (c >> 12));
      out += (char)(0x80 | ((c >> 6) & 0x3F));
      out += (char)(0x80 | (c & 0x3F));
    }
  }
  return out;
}

// The compact placement ending at `end`; false when none fits there.
bool iam_placement(const std::string& d, size_t end, std::array<double, 16>& m)
{
  for (size_t c = 0; c <= 16; ++c)
  {
    if (end < 8 * c + 4)
      return false;
    const size_t   at = end - 8 * c - 4;
    const uint32_t fl = iam_u32(d, at);
    if ((fl & 0x70008000u) != 0x70008000u)
      continue;
    const uint32_t ones = fl & 0xFFFFu, zeros = fl >> 16;
    size_t         stored = 0;
    for (int i = 0; i < 16; ++i)
      if (!((ones >> i) & 1) && !((zeros >> i) & 1))
        ++stored;
    if (stored != c)
      continue;
    size_t next = at + 4;
    for (int i = 0; i < 16; ++i)
    {
      const bool o = (ones >> i) & 1, z = (zeros >> i) & 1;
      if (o && z)
        m[(size_t)i] = -1.0;
      else if (o)
        m[(size_t)i] = 1.0;
      else if (z)
        m[(size_t)i] = 0.0;
      else
      {
        std::memcpy(&m[(size_t)i], d.data() + next, 8);
        next += 8;
      }
    }
    return true;
  }
  return false;
}

struct IamOccurrence
{
  std::string            name, guid;
  std::array<double, 16> m{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
  uint32_t               id = 0; // the record's object id (see iam_browser_order)
};

// The occurrences' object ids in the assembly's browser order: each browser
// entry ("AmPartInstEntry" / "AmAssmInstEntry") is followed by `02 <u32 id>
// 03 00 00 00` + UTF-16 "BRx"; its placement record ends `00 07 <u32 id>
// 03 00 00 00` + "GRx". The browser can be reordered, so it differs from the
// record order (Wrench.iam); Onshape lists the occurrences in the reverse of
// this order (live G42).
std::vector<uint32_t> iam_browser_order(const std::string& d)
{
  // (position, id) of both kinds of entries, then in segment order.
  std::vector<std::pair<size_t, uint32_t>> at;
  const std::string                        brx = iam_utf16("BRx");
  for (const char* kind : {"AmPartInstEntry", "AmAssmInstEntry"})
  {
    const std::string k = iam_utf16(kind);
    for (size_t i = d.find(k); i != std::string::npos; i = d.find(k, i + 1))
    {
      const size_t b = d.find(brx, i + k.size());
      if (b == std::string::npos || b > i + k.size() + 48 || b < 9 || d[b - 9] != 0x02 || iam_u32(d, b - 4) != 3)
        continue;
      at.emplace_back(i, iam_u32(d, b - 8));
    }
  }
  std::sort(at.begin(), at.end());
  std::vector<uint32_t> ids;
  for (const auto& e : at)
    ids.push_back(e.second);
  return ids;
}

std::string iam_ufrx(const std::string& path)
{
  Compound    cf(read_file(path));
  std::string x;
  if (!cf.stream("UFRxDoc", x))
    fail_invalid("inventor: " + file_name_of(path) + " has no document stream");
  return x;
}

std::vector<IamOccurrence> iam_occurrences(const std::string& path)
{
  Compound    cf(read_file(path));
  std::string ufrx;
  if (!cf.stream("UFRxDoc", ufrx))
    fail_invalid("inventor: the assembly has no document stream");
  // The occurrences' property sets, in record order.
  std::vector<IamOccurrence> out;
  static const std::string tag("\x01\x13\x01\x00\x00\x00\x13", 7), name_tag("\x05\x01\x00\x00\x00\x05", 6);
  for (size_t i = ufrx.find(tag); i != std::string::npos && i + tag.size() + 16 <= ufrx.size(); i = ufrx.find(tag, i + 1))
  {
    IamOccurrence o;
    o.guid         = ufrx.substr(i + tag.size(), 16);
    const size_t n = ufrx.find(name_tag, i + tag.size() + 16);
    if (n != std::string::npos && n < i + tag.size() + 16 + 200 && n + name_tag.size() + 4 <= ufrx.size())
      o.name = iam_utf8(ufrx, n + name_tag.size() + 4, iam_u32(ufrx, n + name_tag.size()));
    out.push_back(std::move(o));
  }
  // Their placements: the segment with the "GRx" records.
  const std::string grx = iam_utf16("GRx");
  for (const std::string& p : cf.stream_paths())
  {
    if (p.rfind("RSeStorage/B", 0) != 0)
      continue;
    std::string raw, d;
    if (!cf.stream(p, raw) || raw.size() < 21 || (unsigned char)raw[18] != 0x78)
      continue;
    try
    {
      d = pvz::inflate_raw((const unsigned char*)raw.data() + 20, raw.size() - 20);
    }
    catch (const std::runtime_error&)
    {
      continue;
    }
    std::vector<size_t> marks;
    for (size_t k = d.find(grx); k != std::string::npos; k = d.find(grx, k + 1))
      marks.push_back(k);
    if (marks.empty())
      continue;
    if (marks.size() != out.size())
      fail_unsupported("inventor: the assembly's occurrences don't match their placements");
    for (size_t k = 0; k < marks.size(); ++k)
    {
      std::array<double, 16> r;
      if (marks[k] < 10 || !iam_placement(d, marks[k] - 10, r))
        fail_unsupported("inventor: an occurrence placement noshape can't read");
      out[k].id = iam_u32(d, marks[k] - 8);
      // Row-major, cm → column-major, metres.
      auto& m = out[k].m;
      for (int row = 0; row < 3; ++row)
        for (int col = 0; col < 3; ++col)
          m[(size_t)(col * 4 + row)] = r[(size_t)(row * 4 + col)];
      for (int row = 0; row < 3; ++row)
        m[(size_t)(12 + row)] = r[(size_t)(row * 4 + 3)] * 0.01;
    }
    // Browser order; occurrences the browser doesn't list keep their place
    // after the listed ones.
    std::vector<uint32_t> order;
    for (const std::string& q : cf.stream_paths())
    {
      std::string qraw, qd;
      if (q.rfind("RSeStorage/B", 0) != 0 || !cf.stream(q, qraw) || qraw.size() < 21 || (unsigned char)qraw[18] != 0x78)
        continue;
      try
      {
        qd = pvz::inflate_raw((const unsigned char*)qraw.data() + 20, qraw.size() - 20);
      }
      catch (const std::runtime_error&)
      {
        continue;
      }
      if (qd.find(iam_utf16("AmBrowserArrangement")) == std::string::npos)
        continue;
      order = iam_browser_order(qd);
      break;
    }
    auto rank = [&](const IamOccurrence& o) {
      const auto it = std::find(order.begin(), order.end(), o.id);
      return it == order.end() ? order.size() : (size_t)(it - order.begin());
    };
    std::stable_sort(out.begin(), out.end(), [&](const IamOccurrence& a, const IamOccurrence& b) { return rank(a) < rank(b); });
    return out;
  }
  if (out.empty())
    return out;
  fail_unsupported("inventor: the assembly's placements are missing");
}

} // namespace

std::vector<AsmNode> read_iam_tree(const std::string& path, const std::string& search_dir)
{
  namespace fs = std::filesystem;
  // Every Inventor document under `search_dir` (relative paths) and its
  // UFRxDoc, read once.
  std::vector<std::pair<std::string, std::string>> docs;
  std::error_code                                  ec;
  for (fs::recursive_directory_iterator it(search_dir, ec), end; !ec && it != end; it.increment(ec))
  {
    if (!it->is_regular_file(ec))
      continue;
    const std::string e = it->path().extension().string();
    if (e.size() != 4 || (e[1] != 'i' && e[1] != 'I') || (std::tolower((unsigned char)e[3]) != 't' && std::tolower((unsigned char)e[3]) != 'm'))
      continue;
    try
    {
      docs.emplace_back(fs::relative(it->path(), search_dir, ec).generic_string(), iam_ufrx(it->path().string()));
    }
    catch (const std::exception&)
    {
    }
  }
  auto lower = [](std::string s) {
    for (char& c : s)
      c = (char)std::tolower((unsigned char)c);
    return s;
  };
  auto stem = [&](const std::string& rel) { return lower(fs::path(rel).stem().string()); };
  auto ext  = [&](const std::string& rel) { return lower(fs::path(rel).extension().string()); };
  std::vector<AsmNode>                        out;
  std::function<void(const std::string&, int, std::vector<std::string>&, int)> walk = [&](const std::string& file, int parent, std::vector<std::string>& chain, int depth) {
    if (depth > 32)
      return;
    std::map<std::string, int> numbers;
    for (const IamOccurrence& o : iam_occurrences(file))
    {
      AsmNode n;
      n.parent = parent;
      n.name   = o.name;
      n.number = ++numbers[o.name];
      n.m      = o.m;
      // The referenced document: the one whose UFRxDoc holds the id; a part
      // over the assemblies that use it, then the name (mirrored copies share
      // the id), never an assembly up the chain.
      const std::string want = lower(o.name);
      int               best = -1, best_score = -1;
      for (size_t i = 0; i < docs.size(); ++i)
      {
        if (o.guid.empty() || docs[i].second.find(o.guid) == std::string::npos)
          continue;
        if (std::find(chain.begin(), chain.end(), docs[i].first) != chain.end())
          continue;
        const std::string s     = stem(docs[i].first);
        const int         score = (ext(docs[i].first) == ".ipt" ? 4 : 0) + (s == want ? 2 : 0) + (s.rfind(want, 0) == 0 ? 1 : 0);
        if (score > best_score)
          best = (int)i, best_score = score;
      }
      if (best >= 0)
      {
        n.file     = docs[(size_t)best].first;
        n.assembly = ext(n.file) == ".iam";
      }
      out.push_back(n);
      if (n.assembly)
      {
        chain.push_back(n.file);
        walk((fs::path(search_dir) / n.file).string(), (int)out.size() - 1, chain, depth + 1);
        chain.pop_back();
      }
    }
  };
  std::string root = fs::relative(fs::path(path), search_dir, ec).generic_string();
  std::vector<std::string> chain{root};
  walk(path, -1, chain, 0);
  return out;
}


// ---- Solid Edge assemblies (.asm)
//
// "JAsmRefAttr" lists the occurrences in order, by name "<file>:<k>" (UTF-16,
// NUL-terminated); "Attachments" holds each one's placement: 16 doubles
// (row-major, row-vector convention like SOLIDWORKS' swTransform, metres),
// a 6-double box and a u32, then the name's byte length and the name, so the
// matrix starts 180 bytes before that length (live G43: Onshape's
// transforms). "Preview/Structure/IDS" keeps the same list for previews but
// can be stale (G43: the module holders 5 mm off), so it is only a fallback.

namespace
{

// UTF-16 names "<file>.<ext>:<k>" in `d` with the offset of their u32 length
// (`bytes`: the length counts bytes and the terminating NUL; else characters).
std::vector<std::pair<size_t, std::string>> se_names(const std::string& d, bool bytes)
{
  std::vector<std::pair<size_t, std::string>> out;
  for (size_t k = 0; k + 8 <= d.size(); ++k)
  {
    const uint32_t n     = iam_u32(d, k);
    const size_t   chars = bytes ? (n >= 2 && n % 2 == 0 ? n / 2 - 1 : 0) : n;
    if (chars < 5 || chars > 260 || k + 4 + 2 * chars > d.size())
      continue;
    bool ascii = true;
    for (size_t c = 0; c < chars && ascii; ++c)
      ascii = d[k + 4 + 2 * c] >= 0x20 && d[k + 4 + 2 * c] < 0x7f && d[k + 5 + 2 * c] == 0;
    if (!ascii)
      continue;
    const std::string s     = iam_utf8(d, k + 4, chars);
    const size_t      colon = s.rfind(':');
    if (colon == std::string::npos || colon < 5 || s[colon - 4] != '.' || colon + 1 >= s.size() || !std::isdigit((unsigned char)s[colon + 1]))
      continue;
    out.emplace_back(k, s);
    k += 3 + 2 * chars;
  }
  return out;
}

// Printable UTF-16 runs "<file>.<ext>:<k>" in `d`, in order (JAsmRefAttr
// keeps its names NUL-terminated, without a length).
std::vector<std::string> se_runs(const std::string& d)
{
  std::vector<std::string> out;
  for (size_t k = 0; k + 1 < d.size(); ++k)
  {
    size_t e = k;
    while (e + 1 < d.size() && d[e] >= 0x20 && d[e] < 0x7f && d[e + 1] == 0)
      e += 2;
    if (e - k < 10)
      continue;
    const std::string s     = iam_utf8(d, k, (e - k) / 2);
    const size_t      colon = s.rfind(':');
    if (colon != std::string::npos && colon >= 5 && s[colon - 4] == '.' && colon + 1 < s.size() && std::isdigit((unsigned char)s[colon + 1]))
      out.push_back(s);
    k = e;
  }
  return out;
}

bool se_rigid(const std::array<double, 16>& m)
{
  if (std::abs(m[3]) > 1e-9 || std::abs(m[7]) > 1e-9 || std::abs(m[11]) > 1e-9 || std::abs(m[15] - 1) > 1e-9)
    return false;
  for (int i = 0; i < 3; ++i)
    for (int j = 0; j < 3; ++j)
    {
      double dot = 0;
      for (int k = 0; k < 3; ++k)
        dot += m[(size_t)(4 * i + k)] * m[(size_t)(4 * j + k)];
      if (std::abs(dot - (i == j ? 1.0 : 0.0)) > 1e-6)
        return false;
    }
  return true;
}

} // namespace

std::vector<AsmNode> read_se_tree(const std::string& path, const std::string& search_dir)
{
  namespace fs = std::filesystem;
  std::vector<AsmNode>                              out;
  std::function<void(const std::string&, int, int)> walk = [&](const std::string& file, int parent, int depth) {
    if (depth > 32)
      return;
    Compound    cf(read_file(file));
    std::string order, att, ids;
    cf.stream("JAsmRefAttr", order);
    cf.stream("Attachments", att);
    cf.stream("Preview/Structure/IDS", ids);
    // Placements by name: Attachments first, the preview list as fallback.
    std::map<std::string, std::array<double, 16>> placed;
    auto take = [&](const std::string& d, bool bytes, size_t back) {
      for (const auto& [at, name] : se_names(d, bytes))
      {
        if (placed.count(name) || at < back)
          continue;
        std::array<double, 16> m;
        std::memcpy(m.data(), d.data() + at - back, 128);
        if (se_rigid(m))
          placed[name] = m;
      }
    };
    take(att, true, 180);
    take(ids, false, 128);
    std::vector<std::string> names;
    for (const std::string& name : se_runs(order))
      if (std::find(names.begin(), names.end(), name) == names.end())
        names.push_back(name);
    if (names.empty())
      for (const auto& [name, m] : placed)
        names.push_back(name);
    if (names.empty())
      fail_unsupported("this Solid Edge assembly has no occurrence list noshape can read");
    for (const std::string& full : names)
    {
      const size_t colon = full.rfind(':');
      AsmNode      n;
      n.parent = parent;
      n.file   = full.substr(0, colon);
      n.name   = fs::path(n.file).stem().string();
      n.number = std::max(1, std::atoi(full.c_str() + colon + 1));
      if (placed.count(full))
        n.m = placed[full];
      std::string ext = fs::path(n.file).extension().string();
      for (char& c : ext)
        c = (char)std::tolower((unsigned char)c);
      n.assembly = ext == ".asm";
      out.push_back(n);
      if (n.assembly)
        walk((fs::path(search_dir) / n.file).string(), (int)out.size() - 1, depth + 1);
    }
  };
  walk(path, -1, 0);
  return out;
}

std::vector<AsmNode> read_native_tree(const std::string& path, const std::string& search_dir)
{
  std::string ext = std::filesystem::path(path).extension().string();
  for (char& c : ext)
    c = (char)std::tolower((unsigned char)c);
  if (ext == ".iam")
    return read_iam_tree(path, search_dir);
  if (ext == ".asm")
    return read_se_tree(path, search_dir);
  return read_sldasm_tree(path);
}

} // namespace noshape
