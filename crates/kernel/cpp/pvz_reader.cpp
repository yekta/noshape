// PTC Creo View .pvz reader (see pvz_reader.hpp).
#include "pvz_reader.hpp"

#include <cstring>
#include <functional>
#include <map>
#include <stdexcept>

namespace noshape {
namespace pvz {

namespace {

[[noreturn]] void bad(const std::string& what)
{
  throw std::runtime_error("pvz: " + what);
}

// ---------------------------------------------------------------- inflate

// Canonical Huffman decoding (as zlib's puff.c).
struct Huffman
{
  short count[16] = {};
  short symbol[320] = {};
};

struct Inflater
{
  Inflater(const unsigned char* in_, size_t len_) : in(in_), len(len_) {}
  const unsigned char* in;
  size_t               len, pos = 0;
  unsigned             bitbuf = 0, bitcnt = 0;
  std::string          out;

  int bits(unsigned need)
  {
    unsigned long val = bitbuf;
    while (bitcnt < need)
    {
      if (pos >= len)
        bad("truncated deflate stream");
      val |= (unsigned long)in[pos++] << bitcnt;
      bitcnt += 8;
    }
    bitbuf = (unsigned)(val >> need);
    bitcnt -= need;
    return (int)(val & ((1UL << need) - 1));
  }

  void stored()
  {
    bitbuf = bitcnt = 0;
    if (pos + 4 > len)
      bad("truncated stored block");
    const unsigned n = in[pos] | (in[pos + 1] << 8);
    if ((~n & 0xffff) != (unsigned)(in[pos + 2] | (in[pos + 3] << 8)))
      bad("bad stored block length");
    pos += 4;
    if (pos + n > len)
      bad("truncated stored block");
    out.append((const char*)in + pos, n);
    pos += n;
  }

  int decode(const Huffman& h)
  {
    int code = 0, first = 0, index = 0;
    for (int l = 1; l < 16; ++l)
    {
      code |= bits(1);
      const int count = h.count[l];
      if (code - count < first)
      {
        const int k = index + (code - first);
        if (k < 0 || k >= 320)
          bad("bad Huffman code");
        return h.symbol[k];
      }
      index += count;
      first += count;
      first <<= 1;
      code <<= 1;
    }
    bad("bad Huffman code");
  }

  // As puff.c: the number of unused codes (negative when over-subscribed).
  static int construct(Huffman& h, const short* length, int n)
  {
    std::memset(h.count, 0, sizeof h.count);
    for (int s = 0; s < n; ++s)
      h.count[length[s]]++;
    if (h.count[0] == n)
      return 0;
    int left = 1;
    for (int l = 1; l < 16; ++l)
    {
      left <<= 1;
      left -= h.count[l];
      if (left < 0)
        return left;
    }
    short offs[16];
    offs[1] = 0;
    for (int l = 1; l < 15; ++l)
      offs[l + 1] = (short)(offs[l] + h.count[l]);
    for (int s = 0; s < n; ++s)
      if (length[s] != 0)
        h.symbol[offs[length[s]]++] = (short)s;
    return left;
  }

  void codes(const Huffman& lencode, const Huffman& distcode)
  {
    static const short lbase[29] = {3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
    static const short lext[29] = {0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
    static const short dbase[30] = {1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577};
    static const short dext[30] = {0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13};
    for (;;)
    {
      int sym = decode(lencode);
      if (sym < 256)
        out.push_back((char)sym);
      else if (sym == 256)
        return;
      else
      {
        sym -= 257;
        if (sym >= 29)
          bad("bad length symbol");
        const int n = lbase[sym] + bits(lext[sym]);
        const int ds = decode(distcode);
        if (ds >= 30)
          bad("bad distance symbol");
        const size_t dist = (size_t)(dbase[ds] + bits(dext[ds]));
        if (dist > out.size())
          bad("distance too far back");
        for (int i = 0; i < n; ++i)
          out.push_back(out[out.size() - dist]);
      }
    }
  }

  void fixed()
  {
    static Huffman lencode, distcode;
    static bool    built = false;
    if (!built)
    {
      short lengths[288];
      int   s = 0;
      for (; s < 144; ++s)
        lengths[s] = 8;
      for (; s < 256; ++s)
        lengths[s] = 9;
      for (; s < 280; ++s)
        lengths[s] = 7;
      for (; s < 288; ++s)
        lengths[s] = 8;
      construct(lencode, lengths, 288);
      for (s = 0; s < 30; ++s)
        lengths[s] = 5;
      construct(distcode, lengths, 30);
      built = true;
    }
    codes(lencode, distcode);
  }

  void dynamic()
  {
    static const short order[19] = {16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15};
    const int          nlen = bits(5) + 257, ndist = bits(5) + 1, ncode = bits(4) + 4;
    if (nlen > 286 || ndist > 30)
      bad("bad dynamic block counts");
    short lengths[320] = {};
    for (int i = 0; i < ncode; ++i)
      lengths[order[i]] = (short)bits(3);
    Huffman lencode, distcode;
    if (construct(lencode, lengths, 19) != 0)
      bad("bad code lengths code");
    for (int i = 0; i < nlen + ndist;)
    {
      int sym = decode(lencode);
      if (sym < 16)
        lengths[i++] = (short)sym;
      else
      {
        short len = 0;
        int   rep;
        if (sym == 16)
        {
          if (i == 0)
            bad("repeat with no first length");
          len = lengths[i - 1];
          rep = 3 + bits(2);
        }
        else if (sym == 17)
          rep = 3 + bits(3);
        else
          rep = 11 + bits(7);
        if (i + rep > nlen + ndist)
          bad("too many lengths");
        while (rep--)
          lengths[i++] = len;
      }
    }
    if (lengths[256] == 0)
      bad("no end-of-block code");
    // Incomplete codes are allowed only for a single code of length 1.
    int left = construct(lencode, lengths, nlen);
    if (left < 0 || (left > 0 && nlen != lencode.count[0] + lencode.count[1]))
      bad("bad literal/length code");
    left = construct(distcode, lengths + nlen, ndist);
    if (left < 0 || (left > 0 && ndist != distcode.count[0] + distcode.count[1]))
      bad("bad distance code");
    codes(lencode, distcode);
  }

  void run()
  {
    int last;
    do
    {
      last = bits(1);
      const int type = bits(2);
      if (type == 0)
        stored();
      else if (type == 1)
        fixed();
      else if (type == 2)
        dynamic();
      else
        bad("bad deflate block type");
    } while (!last);
  }
};

std::string gunzip(const unsigned char* p, size_t n)
{
  if (n < 18 || p[0] != 0x1f || p[1] != 0x8b || p[2] != 8)
    bad("bad gzip header");
  const int flags = p[3];
  size_t    at    = 10;
  if (flags & 4)
  {
    if (at + 2 > n)
      bad("truncated gzip header");
    at += 2 + (p[at] | (p[at + 1] << 8));
  }
  for (int f : {8, 16})
    if (flags & f)
    {
      while (at < n && p[at])
        ++at;
      ++at;
    }
  if (flags & 2)
    at += 2;
  if (at >= n)
    bad("truncated gzip header");
  return inflate_raw(p + at, n - at);
}

// ---------------------------------------------------------------- readers

uint32_t u32(const std::string& s, size_t at)
{
  if (at + 4 > s.size())
    bad("truncated");
  const auto* b = (const unsigned char*)s.data() + at;
  return b[0] | (b[1] << 8) | (b[2] << 16) | ((uint32_t)b[3] << 24);
}
uint16_t u16(const std::string& s, size_t at)
{
  if (at + 2 > s.size())
    bad("truncated");
  const auto* b = (const unsigned char*)s.data() + at;
  return (uint16_t)(b[0] | (b[1] << 8));
}
uint64_t u64(const std::string& s, size_t at)
{
  return u32(s, at) | ((uint64_t)u32(s, at + 4) << 32);
}
float f32(const std::string& s, size_t at)
{
  const uint32_t v = u32(s, at);
  float          f;
  std::memcpy(&f, &v, 4);
  return f;
}
double f64(const std::string& s, size_t at)
{
  const uint64_t v = u64(s, at);
  double         d;
  std::memcpy(&d, &v, 8);
  return d;
}

// Zip members by name (inflated).
std::map<std::string, std::string> unzip(const std::string& z)
{
  if (z.size() < 22)
    bad("not a zip file");
  size_t eocd = std::string::npos;
  for (size_t i = z.size() - 22 + 1; i-- > 0 && z.size() - i <= 22 + 65535;)
    if (u32(z, i) == 0x06054b50)
    {
      eocd = i;
      break;
    }
  if (eocd == std::string::npos)
    bad("no zip directory");
  const unsigned count = u16(z, eocd + 10);
  size_t         at    = u32(z, eocd + 16);
  std::map<std::string, std::string> out;
  for (unsigned k = 0; k < count; ++k)
  {
    if (u32(z, at) != 0x02014b50)
      bad("bad zip directory entry");
    const unsigned method = u16(z, at + 10);
    uint64_t       csize = u32(z, at + 20), usize = u32(z, at + 24), local = u32(z, at + 42);
    const unsigned nlen = u16(z, at + 28), elen = u16(z, at + 30), clen = u16(z, at + 32);
    const std::string name = z.substr(at + 46, nlen);
    // Zip64 extra: the fields that are 0xFFFFFFFF above, in this order.
    for (size_t e = at + 46 + nlen; e + 4 <= at + 46 + nlen + elen;)
    {
      const unsigned id = u16(z, e), sz = u16(z, e + 2);
      if (id == 1)
      {
        size_t f = e + 4;
        if (usize == 0xffffffffu)
          usize = u64(z, f), f += 8;
        if (csize == 0xffffffffu)
          csize = u64(z, f), f += 8;
        if (local == 0xffffffffu)
          local = u64(z, f);
      }
      e += 4 + sz;
    }
    if (u32(z, local) != 0x04034b50)
      bad("bad zip local header");
    const size_t data = local + 30 + u16(z, local + 26) + u16(z, local + 28);
    if (data + csize > z.size())
      bad("truncated zip member");
    const auto* p = (const unsigned char*)z.data() + data;
    if (method == 0)
      out[name] = std::string((const char*)p, csize);
    else if (method == 8)
      out[name] = inflate_raw(p, csize);
    else
      bad("unsupported zip compression");
    at += 46 + nlen + elen + clen;
  }
  return out;
}

// ---- PVS product tree

struct Cursor
{
  const std::string& s;
  size_t             at, end;
  unsigned char      u8()
  {
    if (at >= end)
      bad("truncated product tree");
    return (unsigned char)s[at++];
  }
  uint64_t varint()
  {
    uint64_t v = 0;
    for (int shift = 0; shift < 64; shift += 7)
    {
      const unsigned char b = u8();
      v |= (uint64_t)(b & 0x7f) << shift;
      if (!(b & 0x80))
        return v;
    }
    bad("bad varint");
  }
  std::string bytes(size_t n)
  {
    if (at + n > end)
      bad("truncated product tree");
    std::string r = s.substr(at, n);
    at += n;
    return r;
  }
};

struct PNode
{
  int                tag = 0, kind = 0, extra = -1;
  std::string        payload;
  std::vector<PNode> list0;
};

bool pnode(Cursor& c, PNode& n)
{
  n.tag = c.u8();
  if (n.tag == 0)
    return false;
  n.kind = c.u8();
  if (n.kind & 0x02)
  {
    const size_t len = c.varint();
    if (n.kind & 0x10)
      n.extra = c.u8();
    n.payload = c.bytes(len);
  }
  if (n.kind & 0x80)
    for (int l = 0; l < 2; ++l)
      for (;;)
      {
        PNode ch;
        if (!pnode(c, ch))
          break;
        if (l == 0)
          n.list0.push_back(std::move(ch));
      }
  return true;
}

std::string pvs_string(Cursor& c, const std::vector<std::string>& table)
{
  const uint64_t v = c.varint();
  if (v & 1)
  {
    if ((v >> 1) >= table.size())
      bad("bad string reference");
    return table[v >> 1];
  }
  return c.bytes(v >> 1);
}

using Placement = std::array<double, 12>;

Placement compose(const Placement& a, const Placement& b) // a * b
{
  Placement r{};
  for (int i = 0; i < 3; ++i)
  {
    for (int j = 0; j < 3; ++j)
      r[i * 4 + j] = a[i * 4] * b[j] + a[i * 4 + 1] * b[4 + j] + a[i * 4 + 2] * b[8 + j];
    r[i * 4 + 3] = a[i * 4] * b[3] + a[i * 4 + 1] * b[7] + a[i * 4 + 2] * b[11] + a[i * 4 + 3];
  }
  return r;
}

// ---- OL tessellation

void ol_mesh(const std::string& ol, MeshInstance& m)
{
  if (ol.compare(0, 10, "DIV-OL09BL") != 0)
    bad("bad part file header");
  if (u32(ol, 10) != 1)
    bad("bad part file directory");
  const uint32_t n = u32(ol, 18);
  std::vector<std::pair<uint32_t, uint32_t>> dir;
  for (uint32_t i = 0; i < n; ++i)
    dir.emplace_back(u32(ol, 22 + 8 * i), u32(ol, 26 + 8 * i));
  size_t from = 0, to = 0;
  for (size_t i = 0; i + 1 < dir.size(); ++i)
    if ((dir[i].first & 0xffffff) == 1)
      from = dir[i].second, to = dir[i + 1].second;
  if (to <= from || to > ol.size())
    bad("no tessellation in a part file");
  const std::string t = gunzip((const unsigned char*)ol.data() + from, to - from);
  // Records: u32 tag (bit 24 = container: children follow, closed by 0 0), u32 len, payload.
  size_t at = 0, base = 0;
  while (at + 8 <= t.size())
  {
    const uint32_t tag = u32(t, at), len = u32(t, at + 4);
    const size_t   p   = at + 8;
    if (p + len > t.size())
      bad("truncated tessellation record");
    at = p + len;
    switch (tag & 0xffffff)
    {
    case 0x16: // VERTICES: u8 format, u32 count, count x (px py pz nx ny nz) f32
    {
      const uint32_t cnt = u32(t, p + 1);
      if (5 + (size_t)cnt * 24 > len)
        bad("truncated vertices");
      base = m.positions.size() / 3;
      for (uint32_t i = 0; i < cnt; ++i)
        for (int k = 0; k < 3; ++k)
          m.positions.push_back(f32(t, p + 5 + 24 * (size_t)i + 4 * k));
      break;
    }
    case 0x19: // fan
    case 0x1a: // strip
    case 0x1b: // list
    {
      std::vector<uint32_t> ix(len / 4);
      for (size_t i = 0; i < ix.size(); ++i)
        ix[i] = (uint32_t)base + u32(t, p + 4 * i);
      const size_t nv = m.positions.size() / 3;
      for (uint32_t v : ix)
        if (v >= nv)
          bad("bad vertex index");
      auto tri = [&](uint32_t a, uint32_t b, uint32_t c) { m.triangles.insert(m.triangles.end(), {a, b, c}); };
      const int type = tag & 0xffffff;
      if (type == 0x19)
        for (size_t k = 1; k + 1 < ix.size(); ++k)
          tri(ix[0], ix[k], ix[k + 1]);
      else if (type == 0x1a)
        for (size_t k = 0; k + 2 < ix.size(); ++k)
          k % 2 == 0 ? tri(ix[k], ix[k + 1], ix[k + 2]) : tri(ix[k + 1], ix[k], ix[k + 2]);
      else
        for (size_t k = 0; k + 2 < ix.size(); k += 3)
          tri(ix[k], ix[k + 1], ix[k + 2]);
      break;
    }
    default:
      break;
    }
  }
}

} // namespace

std::map<std::string, std::string> zip_members(const std::string& zip)
{
  return unzip(zip);
}

std::string inflate_raw(const unsigned char* in, size_t len)
{
  Inflater f(in, len);
  f.run();
  return std::move(f.out);
}

std::vector<MeshInstance> read_meshes(const std::string& bytes, std::string* root_name)
{
  const auto members = unzip(bytes);
  const std::string* pvs = nullptr;
  for (const auto& [name, data] : members)
    if (name.size() > 4 && name.compare(name.size() - 4, 4, ".pvs") == 0)
      pvs = &data;
  if (!pvs || pvs->compare(0, 10, "PV-PVS0305") != 0)
    bad("no product structure (.pvs) in the file");
  const std::string& s     = *pvs;
  const uint32_t     hlen  = u32(s, 10), nblk = u32(s, 14);
  size_t             at    = hlen;
  std::vector<std::string> table;
  PNode                    root;
  bool                     have_root = false;
  for (uint32_t b = 0; b < nblk; ++b)
  {
    const int      type = (unsigned char)s.at(18 + 7 * b);
    const uint32_t glen = u32(s, 18 + 7 * b + 3);
    if (at + glen > s.size())
      bad("truncated product structure");
    const std::string blk = gunzip((const unsigned char*)s.data() + at, glen);
    at += glen;
    Cursor c{blk, 0, blk.size()};
    PNode  n;
    if (!pnode(c, n))
      continue;
    if (type == 1) // string table: varint count, count x (varint n, bytes)
    {
      Cursor t{n.payload, 0, n.payload.size()};
      for (uint64_t k = t.varint(); k-- > 0;)
        table.push_back(t.bytes(t.varint()));
    }
    else if (type == 2)
    {
      root      = std::move(n);
      have_root = true;
    }
  }
  if (!have_root || root.list0.empty())
    bad("empty product structure");
  // NODEs in post-order, the root last.
  std::vector<MeshInstance> out;
  std::map<std::string, MeshInstance> meshes;
  std::function<void(size_t, const Placement&, int)> visit = [&](size_t i, const Placement& M, int depth) {
    if (depth > 64)
      bad("product structure too deep");
    const PNode& node = root.list0.at(i);
    Cursor       nc{node.payload, 0, node.payload.size()};
    const std::string name = pvs_string(nc, table);
    for (const PNode& e : node.list0)
    {
      Cursor c{e.payload, 0, e.payload.size()};
      if (e.tag == 0x23) // SHAPE
      {
        const std::string ol = pvs_string(c, table);
        auto              it = meshes.find(ol);
        if (it == meshes.end())
        {
          auto m = members.find(ol);
          if (m == members.end())
            bad("missing part file " + ol);
          MeshInstance mi;
          ol_mesh(m->second, mi);
          it = meshes.emplace(ol, std::move(mi)).first;
        }
        MeshInstance inst = it->second;
        inst.name         = name;
        inst.ol_file      = ol;
        inst.placement    = M;
        out.push_back(std::move(inst));
      }
      else if (e.tag == 0x21) // CHILD
      {
        const size_t idx = c.varint();
        if (idx >= i)
          bad("bad child reference");
        Placement T{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
        const int extra = e.extra < 0 ? 0 : e.extra;
        size_t    p     = c.at;
        if (extra & 2)
        {
          for (int k = 0; k < 3; ++k)
            T[k * 4 + 3] = f64(e.payload, p + 8 * k);
          p += 24;
        }
        if (extra & 4) // R column by column
          for (int col = 0; col < 3; ++col)
            for (int row = 0; row < 3; ++row)
              T[row * 4 + col] = f64(e.payload, p + 8 * (col * 3 + row));
        visit(idx, compose(M, T), depth + 1);
      }
    }
  };
  if (root_name)
  {
    Cursor rc{root.list0.back().payload, 0, root.list0.back().payload.size()};
    *root_name = pvs_string(rc, table);
  }
  visit(root.list0.size() - 1, {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0}, 0);
  return out;
}

} // namespace pvz
} // namespace noshape
