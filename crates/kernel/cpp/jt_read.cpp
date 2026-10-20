// JT import through its exact geometry: the XT B-rep segments (Parasolid),
// as Onshape's translator reads JT. Header and table of contents (32-bit
// offsets in JT 8/9, 64-bit from JT 10), then each type-17 segment: a
// segment header, the compression prefix (flag, length, algorithm: 1 none,
// 2 zlib, 3 LZMA/XZ), and the element holding the neutral binary transmit
// stream (type 30 "multi XT" segments too). Parts are placed by the scene
// graph (LSG): instances' transform attributes composed down to each Part
// node, which names its XT segment(s) in the property table; a copy per
// instance path. A scene graph that doesn't parse leaves the parts unplaced.

#include "internal.hpp"
#include "pvz_reader.hpp"

#include <cstring>
#include <functional>
#include <map>
#include <set>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <gp_GTrsf.hxx>
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


// ---- the scene graph (LSG): where each part is placed

constexpr uint32_t kPartition = 0x10dd103e, kPart = 0xce357244, kInstance = 0x10dd102a, kXform = 0x10dd1083;

struct LsgNode
{
  uint32_t              type = 0;
  int                   base = 0;
  std::vector<int32_t>  attrs, children;
  int32_t               child = -1; // an instance's
  bool                  has_xform = false;
  double                m[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1}; // row vectors: p' = p M
};

struct Lsg
{
  std::map<int32_t, LsgNode>                      nodes;
  int32_t                                         root = -1;
  std::map<int32_t, std::vector<std::string>>     part_segments; // part node -> segment GUIDs (16 bytes)
  double                                          mm = 1;       // millimetres per LSG unit
};

std::string utf16(const char* a)
{
  std::string s;
  for (; *a; ++a)
    s += *a, s += '\0';
  return s;
}

// Parse the LSG; throws on anything unexpected (the caller then falls back
// to reading the parts unplaced).
Lsg parse_lsg(const std::string& s, int major, const std::vector<std::string>& segment_guids)
{
  Lsg       g;
  size_t    at  = 0;
  const bool v10 = major >= 10;
  auto i32 = [&](size_t p) {
    if (p + 4 > s.size())
      fail_invalid("jt: truncated scene graph");
    int32_t v;
    std::memcpy(&v, s.data() + p, 4);
    return v;
  };
  auto u16 = [&](size_t p) {
    if (p + 2 > s.size())
      fail_invalid("jt: truncated scene graph");
    uint16_t v;
    std::memcpy(&v, s.data() + p, 2);
    return v;
  };
  const auto eoe = [&](size_t p) { return s.compare(p + 4, 16, std::string(16, '\xff')) == 0; };
  // Graph elements.
  for (int guard = 0; !eoe(at); ++guard)
  {
    const int32_t len = i32(at);
    if (len < 21 || at + 4 + (size_t)len > s.size() || guard > 10000000)
      fail_invalid("jt: bad scene graph element");
    uint32_t type;
    std::memcpy(&type, s.data() + at + 4, 4);
    LsgNode n;
    n.type            = type;
    n.base            = (unsigned char)s[at + 20];
    const int32_t id  = i32(at + 21);
    size_t        p   = at + 25;
    const size_t  end = at + 4 + (size_t)len;
    if (n.base <= 2)
    {
      // Base node: version, flags, attributes; a group's children; an
      // instance's child. An element that doesn't read so is skipped (only
      // the nodes on the way to parts matter here).
      p += (v10 ? 1 : 2) + 4; // JT 10 versions are one byte, JT 8/9 two
      const int32_t na = p + 4 <= end ? i32(p) : -1;
      p += 4;
      bool ok = na >= 0 && p + (size_t)na * 4 <= end;
      for (int32_t k = 0; ok && k < na; ++k, p += 4)
        n.attrs.push_back(i32(p));
      if (ok && n.base == 1)
      {
        p += v10 ? 1 : 2;
        const int32_t nc = p + 4 <= end ? i32(p) : -1;
        p += 4;
        ok = nc >= 0 && p + (size_t)nc * 4 <= end;
        for (int32_t k = 0; ok && k < nc; ++k, p += 4)
          n.children.push_back(i32(p));
      }
      else if (ok && type == kInstance)
      {
        p += v10 ? 1 : 2;
        ok = p + 4 <= end;
        if (ok)
          n.child = i32(p);
      }
      if (!ok)
      {
        if (type == kPartition || type == kInstance || type == kPart)
          fail_invalid("jt: bad node");
        n = LsgNode{};
      }
    }
    else if (n.base == 3 && type == kXform)
    {
      // Base attribute (JT 10: U8 version, U8 state, U32 inhibit, U32;
      // JT 8/9: I16, U8, U32), version, mask (bit 15 - i: element i stored),
      // the stored elements (F64 in JT 10, F32 before).
      p += v10 ? 10 : 7;
      p += v10 ? 1 : 2;
      const uint16_t mask = u16(p);
      p += 2;
      for (int i = 0; i < 16; ++i)
        if (mask & (1u << (15 - i)))
        {
          if (v10)
          {
            if (p + 8 > end)
              fail_invalid("jt: bad transform");
            std::memcpy(&n.m[i], s.data() + p, 8);
            p += 8;
          }
          else
          {
            if (p + 4 > end)
              fail_invalid("jt: bad transform");
            float f;
            std::memcpy(&f, s.data() + p, 4);
            n.m[i] = f;
            p += 4;
          }
        }
      n.has_xform = true;
    }
    if (g.root < 0 && type == kPartition)
      g.root = id;
    g.nodes[id] = n;
    at          = end;
  }
  at += 20;
  // Property atoms: the XT keys, and the values that name segments or units.
  std::map<int32_t, std::string> atoms;
  for (int guard = 0; !eoe(at); ++guard)
  {
    const int32_t len = i32(at);
    if (len < 21 || at + 4 + (size_t)len > s.size() || guard > 10000000)
      fail_invalid("jt: bad property atom");
    atoms[i32(at + 21)] = s.substr(at + 25, (size_t)len - 21);
    at += 4 + (size_t)len;
  }
  at += 20;
  std::set<int32_t> xt_keys;
  for (const auto& [id, d] : atoms)
  {
    if (d.find(utf16("JT_LLPROP_XTBREP")) != std::string::npos || d.find(utf16("JT_LLPROP_MULTIXTBREP")) != std::string::npos)
      xt_keys.insert(id);
    // A units value: a string atom that is exactly the unit's name (its
    // MbString: I32 count, then UTF-16).
    for (const auto& [unit, mm] : std::initializer_list<std::pair<const char*, double>>{
             {"millimeters", 1}, {"centimeters", 10}, {"meters", 1000}, {"inches", 25.4}, {"feet", 304.8}})
    {
      const std::string w = utf16(unit);
      if (d.size() < w.size() + 4 || d.compare(d.size() - w.size(), w.size(), w) != 0)
        continue;
      int32_t count;
      std::memcpy(&count, d.data() + d.size() - w.size() - 4, 4);
      if (count == (int32_t)std::strlen(unit))
      {
        g.mm = mm;
        break;
      }
    }
  }
  // The property table: element -> (key, value) pairs.
  at += 2; // version
  const int32_t rows = i32(at);
  at += 4;
  for (int32_t r = 0; r < rows; ++r)
  {
    const int32_t el = i32(at);
    at += 4;
    for (int guard = 0;; ++guard)
    {
      const int32_t key = i32(at);
      at += 4;
      if (key == 0)
        break;
      const int32_t val = i32(at);
      at += 4;
      if (guard > 100000)
        fail_invalid("jt: bad property table");
      if (!xt_keys.count(key) || !atoms.count(val))
        continue;
      for (const std::string& sg : segment_guids)
        if (atoms[val].find(sg) != std::string::npos)
          g.part_segments[el].push_back(sg);
    }
  }
  if (g.root < 0)
    fail_invalid("jt: no partition node");
  return g;
}

} // namespace

std::vector<XtPart> read_jt_parts(const std::string& path)
{
  const std::string d = read_file(path);
  if (d.compare(0, 8, "Version ") != 0 || d.size() < 105)
    fail_invalid("jt: not a JT file");
  const int    major = std::atoi(d.c_str() + 8);
  Reader       r{d, d[80] != 0};
  const bool   wide  = major >= 10;
  const size_t toc   = (size_t)r.u(85, wide ? 8 : 4);
  const std::string lsg_guid = d.substr(85 + (wide ? 8 : 4), 16);
  const size_t n     = (size_t)r.u(toc, 4);
  const size_t entry = wide ? 32 : 28;
  if (n > 1000000 || toc + 4 + n * entry > d.size())
    fail_invalid("jt: bad table of contents");
  struct Seg
  {
    size_t off, len;
    int    type;
  };
  std::map<std::string, Seg> segs;
  std::vector<std::string>   xt_guids;
  for (size_t i = 0; i < n; ++i)
  {
    const size_t   e    = toc + 4 + i * entry;
    const size_t   off  = (size_t)r.u(e + 16, wide ? 8 : 4);
    const size_t   len  = (size_t)r.u(e + 16 + (wide ? 8 : 4), 4);
    const int      type = (int)((uint32_t)r.u(e + entry - 4, 4) >> 24);
    segs[d.substr(e, 16)] = {off, len, type};
    if (type == 17 || type == 30)
      xt_guids.push_back(d.substr(e, 16));
  }
  // The parts of one XT (or multi-XT) segment, read once.
  std::map<std::string, std::vector<XtPart>> cache;
  auto parts_of = [&](const std::string& guid) -> const std::vector<XtPart>& {
    auto it = cache.find(guid);
    if (it != cache.end())
      return it->second;
    std::vector<XtPart> parts;
    const Seg&          sg = segs.at(guid);
    const std::string   pl = payload(d, sg.off, sg.len, r);
    const size_t        ps = pl.find(std::string("PS\0\0", 4));
    if (ps != std::string::npos && ps >= 4)
      parts = read_xt_data(pl.substr(ps));
    return cache[guid] = parts;
  };
  std::vector<XtPart> out;
  // Placed by the scene graph when it reads; otherwise every XT segment unplaced.
  try
  {
    auto it = segs.find(lsg_guid);
    if (it == segs.end())
      fail_invalid("jt: no scene graph");
    const Lsg g = parse_lsg(payload(d, it->second.off, it->second.len, r), major, xt_guids);
    std::function<void(int32_t, const gp_GTrsf&, int)> visit = [&](int32_t id, const gp_GTrsf& parent, int depth) {
      auto nt = g.nodes.find(id);
      if (nt == g.nodes.end() || depth > 64)
        return;
      const LsgNode& node = nt->second;
      gp_GTrsf       here = parent;
      for (int32_t a : node.attrs)
      {
        auto at = g.nodes.find(a);
        if (at == g.nodes.end() || !at->second.has_xform)
          continue;
        const double* m = at->second.m;
        gp_GTrsf      local;
        local.SetVectorialPart(gp_Mat(m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]));
        local.SetTranslationPart(gp_XYZ(m[12], m[13], m[14]) * (g.mm * 1e-3));
        here = parent;
        here.Multiply(local); // parent ∘ local
      }
      if (node.type == kPart)
        if (auto ps = g.part_segments.find(id); ps != g.part_segments.end())
          for (const std::string& guid : ps->second)
            for (const XtPart& p : parts_of(guid))
            {
              XtPart q = p;
              if (here.Form() != gp_Identity)
              {
                try
                {
                  gp_Trsf t;
                  const gp_Mat& v = here.VectorialPart();
                  const gp_XYZ& tr = here.TranslationPart();
                  t.SetValues(v(1, 1), v(1, 2), v(1, 3), tr.X(), v(2, 1), v(2, 2), v(2, 3), tr.Y(), v(3, 1), v(3, 2), v(3, 3), tr.Z());
                  q.shape = BRepBuilderAPI_Transform(p.shape, t, true).Shape();
                }
                catch (const Standard_Failure&)
                {
                  q.shape = BRepBuilderAPI_GTransform(p.shape, here, true).Shape();
                }
              }
              out.push_back(q);
            }
      if (node.child >= 0)
        visit(node.child, here, depth + 1);
      for (int32_t c : node.children)
        visit(c, here, depth + 1);
    };
    visit(g.root, gp_GTrsf(), 0);
  }
  catch (const std::exception&)
  {
    out.clear();
  }
  if (out.empty())
    for (const std::string& guid : xt_guids)
    {
      const std::vector<XtPart>& parts = parts_of(guid);
      out.insert(out.end(), parts.begin(), parts.end());
    }
  if (out.empty())
    fail_unsupported("jt: the file has no exact (XT B-rep) geometry, only tessellation");
  return out;
}

} // namespace noshape
