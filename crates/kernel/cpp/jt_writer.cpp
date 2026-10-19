// JT 9.5 file writer -- C++17 port of writer2.py / topoenc.py / enc.py (see jt_writer.hpp).
//
// The byte stream produced here is identical to the Python prototype's (after inflating
// the zlib segments) for the same input and GUIDs.  Structure of this file:
//
//   1. Byte buffer, zlib (stored blocks) and GUID helpers
//   2. lookup2 hashes and the CDP2 fixed-width bit-length codec
//   3. Source mesh: boundary closing (cover polygons) and per-vertex corner rings
//   4. Topology encoder (Appendix-E decoder state machine driven by the source mesh)
//   5. Shape LOD element (topology + lossless vertex records)
//   6. LSG segment and file assembly
#include "jt_writer.hpp"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <limits>
#include <random>
#include <stdexcept>
#include <unordered_map>
#include <utility>

namespace noshape {
namespace jt {

namespace {

using Guid = std::array<uint8_t, 16>;

// ===================================================================================
// 1. Byte buffer, zlib, GUIDs
// ===================================================================================

uint32_t float_bits(float f)
{
  uint32_t u;
  std::memcpy(&u, &f, 4);
  return u;
}

// Little-endian byte sink (all JT primitives used here are little-endian).
class Bytes
{
public:
  void u8(uint8_t v) { s_.push_back(char(v)); }
  void u16(uint16_t v)
  {
    u8(uint8_t(v));
    u8(uint8_t(v >> 8));
  }
  void i16(int v) { u16(uint16_t(int16_t(v))); }
  void u32(uint32_t v)
  {
    for (int i = 0; i < 4; ++i)
      u8(uint8_t(v >> (8 * i)));
  }
  void i32(int64_t v) { u32(uint32_t(v)); }
  void u64(uint64_t v)
  {
    for (int i = 0; i < 8; ++i)
      u8(uint8_t(v >> (8 * i)));
  }
  // Python struct.pack('<f', x): round the double to the nearest float32.
  void f32(double v) { u32(float_bits(float(v))); }
  void guid(const Guid& g) { s_.append(reinterpret_cast<const char*>(g.data()), g.size()); }
  void raw(const std::string& b) { s_ += b; }
  void zeros(size_t n) { s_.append(n, '\0'); }

  // JT MbString: int32 character count + UTF-16LE code units (input is UTF-8).
  void mbstring(const std::string& utf8);

  size_t size() const { return s_.size(); }
  const std::string& str() const { return s_; }
  std::string& str() { return s_; }

private:
  std::string s_;
};

void Bytes::mbstring(const std::string& utf8)
{
  std::vector<uint16_t> units;
  for (size_t i = 0; i < utf8.size();)
  {
    const uint8_t c = uint8_t(utf8[i]);
    uint32_t cp = 0xFFFD;
    int len = 1;
    if (c < 0x80)
      cp = c;
    else if ((c >> 5) == 0x6)
      cp = c & 0x1F, len = 2;
    else if ((c >> 4) == 0xE)
      cp = c & 0x0F, len = 3;
    else if ((c >> 3) == 0x1E)
      cp = c & 0x07, len = 4;
    if (len > 1)
    {
      bool ok = i + len <= utf8.size();
      for (int k = 1; ok && k < len; ++k)
      {
        const uint8_t cc = uint8_t(utf8[i + k]);
        ok = (cc >> 6) == 0x2;
        cp = (cp << 6) | (cc & 0x3F);
      }
      if (!ok)
        cp = 0xFFFD, len = 1;
    }
    i += len;
    if (cp >= 0x10000)
    {
      cp -= 0x10000;
      units.push_back(uint16_t(0xD800 + (cp >> 10)));
      units.push_back(uint16_t(0xDC00 + (cp & 0x3FF)));
    }
    else
      units.push_back(uint16_t(cp));
  }
  i32(int64_t(units.size()));
  for (uint16_t u : units)
    u16(u);
}

uint32_t adler32(const std::string& data)
{
  const uint32_t MOD = 65521;
  uint32_t a = 1, b = 0;
  size_t i = 0;
  while (i < data.size())
  {
    // 5552 is the largest run for which b cannot overflow 32 bits before reduction.
    const size_t end = std::min(data.size(), i + 5552);
    for (; i < end; ++i)
    {
      a += uint8_t(data[i]);
      b += a;
    }
    a %= MOD;
    b %= MOD;
  }
  return (b << 16) | a;
}

// RFC-1950 zlib stream made of RFC-1951 STORED blocks (no compression).
std::string zlib_stored(const std::string& data)
{
  std::string out;
  out.reserve(data.size() + data.size() / 65535 * 5 + 16);
  out += char(0x78); // CMF: deflate, 32K window
  out += char(0x01); // FLG: fastest, no dict; (0x78 * 256 + 0x01) % 31 == 0
  size_t pos = 0;
  do
  {
    const size_t n = std::min<size_t>(65535, data.size() - pos);
    const bool last = pos + n == data.size();
    out += char(last ? 1 : 0); // BFINAL, BTYPE = 00 (stored); rest of the byte is padding
    const uint16_t len = uint16_t(n), nlen = uint16_t(~len);
    out += char(len & 0xFF);
    out += char(len >> 8);
    out += char(nlen & 0xFF);
    out += char(nlen >> 8);
    out.append(data, pos, n);
    pos += n;
  } while (pos < data.size());
  const uint32_t ad = adler32(data);
  for (int s = 24; s >= 0; s -= 8)
    out += char((ad >> s) & 0xFF); // Adler-32 is big-endian
  return out;
}

// Supplies segment GUIDs: random version-4 GUIDs (stored like Python's uuid4().bytes_le)
// or, with a fixed base, base + k in the uint32 at bytes 12..15 for the k-th GUID.
class GuidSource
{
public:
  explicit GuidSource(const Guid* base)
  {
    if (base)
      base_ = *base, fixed_ = true;
    else
      rng_.seed((uint64_t(std::random_device{}()) << 32) ^ std::random_device{}());
  }

  Guid next()
  {
    Guid g{};
    if (fixed_)
    {
      g = base_;
      uint32_t w = uint32_t(g[12]) | uint32_t(g[13]) << 8 | uint32_t(g[14]) << 16 | uint32_t(g[15]) << 24;
      w += counter_++;
      for (int i = 0; i < 4; ++i)
        g[12 + i] = uint8_t(w >> (8 * i));
      return g;
    }
    const uint64_t lo = rng_(), hi = rng_();
    for (int i = 0; i < 8; ++i)
    {
      g[i] = uint8_t(lo >> (8 * i));
      g[8 + i] = uint8_t(hi >> (8 * i));
    }
    g[7] = uint8_t((g[7] & 0x0F) | 0x40); // version 4 (time_hi_and_version, little-endian)
    g[8] = uint8_t((g[8] & 0x3F) | 0x80); // RFC 4122 variant
    return g;
  }

private:
  Guid base_{};
  bool fixed_ = false;
  uint32_t counter_ = 0;
  std::mt19937_64 rng_;
};

// ===================================================================================
// 2. Hashes and the CDP2 bit-length codec
// ===================================================================================

inline void mix(uint32_t& a, uint32_t& b, uint32_t& c)
{
  a -= b; a -= c; a ^= (c >> 13);
  b -= c; b -= a; b ^= (a << 8);
  c -= a; c -= b; c ^= (b >> 13);
  a -= b; a -= c; a ^= (c >> 12);
  b -= c; b -= a; b ^= (a << 16);
  c -= a; c -= b; c ^= (b >> 5);
  a -= b; a -= c; a ^= (c >> 3);
  b -= c; b -= a; b ^= (a << 10);
  c -= a; c -= b; c ^= (b >> 15);
}

// Bob Jenkins' lookup2 "hash2" over 32-bit words, with c += word count (as JT uses it).
uint32_t hash32(const uint32_t* k, size_t n, uint32_t init)
{
  uint32_t a = 0x9e3779b9, b = 0x9e3779b9, c = init;
  size_t i = 0, l = n;
  while (l >= 3)
  {
    a += k[i];
    b += k[i + 1];
    c += k[i + 2];
    mix(a, b, c);
    i += 3;
    l -= 3;
  }
  c += uint32_t(n);
  if (l == 2)
    b += k[i + 1];
  if (l >= 1)
    a += k[i];
  mix(a, b, c);
  return c;
}

uint32_t hash32(const std::vector<int64_t>& v, uint32_t init)
{
  std::vector<uint32_t> k(v.size());
  for (size_t i = 0; i < v.size(); ++i)
    k[i] = uint32_t(v[i]); // == Python x & 0xffffffff
  return hash32(k.data(), k.size(), init);
}

uint32_t hash32(const std::vector<uint32_t>& v, uint32_t init)
{
  return hash32(v.data(), v.size(), init);
}

// lookup2 variant over 16-bit values (two per 32-bit lane), used for vertex flags.
uint32_t hash16(const std::vector<int64_t>& v, uint32_t init)
{
  auto k = [&](size_t i) { return uint32_t(v[i]) & 0xFFFF; };
  const size_t n = v.size();
  uint32_t a = 0x9e3779b9, b = 0x9e3779b9, c = init;
  size_t i = 0, l = n;
  while (l >= 6)
  {
    a += k(i) + (k(i + 1) << 16);
    b += k(i + 2) + (k(i + 3) << 16);
    c += k(i + 4) + (k(i + 5) << 16);
    mix(a, b, c);
    i += 6;
    l -= 6;
  }
  c += uint32_t(n);
  if (l >= 5)
    c += k(i + 4) << 16;
  if (l >= 4)
    b += k(i + 3) << 16;
  if (l >= 3)
    b += k(i + 2);
  if (l >= 2)
    a += k(i + 1) << 16;
  if (l >= 1)
    a += k(i);
  mix(a, b, c);
  return c;
}

// MSB-first bit packer into 32-bit words (the CDP2 bit-stream order).
class BitWriter
{
public:
  void put(uint64_t v, int w)
  {
    while (w > 0)
    {
      if (nbits_ % 32 == 0)
        words_.push_back(0);
      const int room = 32 - int(nbits_ % 32);
      const int take = std::min(w, room);
      const uint32_t chunk = uint32_t((v >> (w - take)) & ((uint64_t(1) << take) - 1));
      words_.back() |= chunk << (room - take);
      w -= take;
      nbits_ += uint64_t(take);
    }
  }
  uint64_t nbits() const { return nbits_; }
  const std::vector<uint32_t>& words() const { return words_; }

private:
  std::vector<uint32_t> words_;
  uint64_t nbits_ = 0;
};

// Bits needed to hold v as a signed two's complement number (bl2.sbits).
int sbits(int64_t v)
{
  int n = 1;
  while (!(-(int64_t(1) << (n - 1)) <= v && v < (int64_t(1) << (n - 1))))
    ++n;
  return n;
}

// Bits needed to hold the non-negative v (bl2.ubits).
int ubits(uint64_t v)
{
  int n = 0;
  while (n < 64 && (uint64_t(1) << n) <= v)
    ++n;
  return n;
}

// Int32 Compressed Data Packet Mk.2, bit-length codec in fixed-width mode (enc.py):
//   count, codec 1, bit count, words of: '0' | w1:6 | w2:6 | min:w1 | max:w2 | (v-min):w each
void cdp2_bitlength(Bytes& out, const std::vector<int64_t>& vals)
{
  if (vals.empty())
  {
    out.i32(0);
    return;
  }
  const auto mm = std::minmax_element(vals.begin(), vals.end());
  const int64_t mn = *mm.first, mx = *mm.second;
  const int w1 = sbits(mn), w2 = sbits(mx), w = ubits(uint64_t(mx - mn));
  auto low = [](int64_t v, int bits) {
    return bits >= 64 ? uint64_t(v) : uint64_t(v) & ((uint64_t(1) << bits) - 1);
  };
  BitWriter bw;
  bw.put(0, 1);
  bw.put(uint64_t(w1), 6);
  bw.put(uint64_t(w2), 6);
  bw.put(low(mn, w1), w1);
  bw.put(low(mx, w2), w2);
  if (w)
    for (int64_t v : vals)
      bw.put(uint64_t(v - mn), w);
  out.i32(int64_t(vals.size()));
  out.u8(1);
  out.i32(int64_t(bw.nbits()));
  for (uint32_t word : bw.words())
    out.u32(word);
}

// JT Lag1 predictor residuals; the first four values are stored verbatim.
std::vector<int64_t> lag1(const std::vector<int64_t>& v)
{
  std::vector<int64_t> r(v.size());
  for (size_t i = 0; i < v.size(); ++i)
    r[i] = i < 4 ? v[i] : v[i] - v[i - 1];
  return r;
}

// Python's floored modulo for a positive divisor.
inline int pymod(int a, int n)
{
  const int r = a % n;
  return r < 0 ? r + n : r;
}

// ===================================================================================
// 3. Source mesh (meshtest.close_mesh + topoenc.SourceDual)
// ===================================================================================

// Raised when the connected encoder cannot handle the input; triggers the pillow path.
struct EncodeError : std::runtime_error
{
  using std::runtime_error::runtime_error;
};

// Directed-edge index: for each vertex, its outgoing corners sorted by target vertex.
// Corner c of polygon p (vertex verts[c]) owns the directed edge verts[c] -> next(c).
class EdgeIndex
{
public:
  EdgeIndex(int nverts, const std::vector<int>& verts, const std::vector<int>& next_vert)
    : start_(size_t(nverts) + 1, 0), entries_(verts.size())
  {
    for (int v : verts)
      ++start_[size_t(v) + 1];
    for (int v = 0; v < nverts; ++v)
      start_[size_t(v) + 1] += start_[size_t(v)];
    std::vector<int> fill(start_.begin(), start_.end() - 1);
    for (size_t c = 0; c < verts.size(); ++c)
      entries_[size_t(fill[size_t(verts[c])]++)] = {next_vert[c], int(c)};
    for (int v = 0; v < nverts; ++v)
    {
      auto b = entries_.begin() + start_[size_t(v)], e = entries_.begin() + start_[size_t(v) + 1];
      std::sort(b, e);
      for (auto it = b; it + 1 < e; ++it)
        if (it->first == (it + 1)->first)
          throw EncodeError("duplicate directed edge (non-manifold or inconsistently oriented)");
    }
  }

  // Corner owning directed edge a -> b, or -1.
  int find(int a, int b) const
  {
    auto bg = entries_.begin() + start_[size_t(a)], e = entries_.begin() + start_[size_t(a) + 1];
    auto it = std::lower_bound(bg, e, std::make_pair(b, std::numeric_limits<int>::min()));
    return it != e && it->first == b ? it->second : -1;
  }

private:
  std::vector<int> start_;
  std::vector<std::pair<int, int>> entries_; // (target vertex, corner)
};

// Polygon mesh: triangles followed by cover polygons, stored as flat corner arrays.
struct PolyMesh
{
  int nverts = 0;
  int ntris = 0;
  std::vector<int> poly_start{0}; // npolys + 1
  std::vector<int> verts;         // corner -> vertex
  std::vector<int> groups;        // per polygon
  std::vector<int> flags;         // per polygon: 0 triangle, 1 cover

  int npolys() const { return int(poly_start.size()) - 1; }
  int size(int t) const { return poly_start[size_t(t) + 1] - poly_start[size_t(t)]; }

  void add(const int* v, int n, int group, int flag)
  {
    verts.insert(verts.end(), v, v + n);
    poly_start.push_back(int(verts.size()));
    groups.push_back(group);
    flags.push_back(flag);
  }
};

std::vector<int> next_vertices(const PolyMesh& m)
{
  std::vector<int> nx(m.verts.size());
  for (int t = 0; t < m.npolys(); ++t)
  {
    const int b = m.poly_start[size_t(t)], n = m.size(t);
    for (int k = 0; k < n; ++k)
      nx[size_t(b + k)] = m.verts[size_t(b + (k + 1) % n)];
  }
  return nx;
}

// Adds one cover polygon per boundary loop, traversing the loop reversed (close_mesh).
// Boundary edges are visited in triangle-corner order (the Python iterates a set, whose
// order is arbitrary; its patched reference copy uses this same order).
void close_mesh(PolyMesh& m)
{
  const std::vector<int> nx = next_vertices(m);
  const EdgeIndex edges(m.nverts, m.verts, nx);
  std::vector<int> bnext(size_t(m.nverts), -1), keys;
  for (size_t c = 0; c < m.verts.size(); ++c)
  {
    const int a = m.verts[c], b = nx[c];
    if (edges.find(b, a) >= 0)
      continue;
    if (bnext[size_t(b)] != -1)
      throw EncodeError("non-manifold boundary vertex");
    bnext[size_t(b)] = a; // boundary edge a -> b; the cover traverses b -> a
    keys.push_back(b);
  }
  std::vector<char> seen(size_t(m.nverts), 0);
  std::vector<int> loop;
  const int group0 = m.groups.empty() ? 0 : m.groups[0];
  for (int s0 : keys)
  {
    if (seen[size_t(s0)])
      continue;
    loop.assign(1, s0);
    seen[size_t(s0)] = 1;
    for (int x = bnext[size_t(s0)]; x != s0; x = bnext[size_t(x)])
    {
      if (x < 0 || seen[size_t(x)]) // Python: KeyError / endless loop
        throw EncodeError("open boundary chain");
      loop.push_back(x);
      seen[size_t(x)] = 1;
    }
    m.add(loop.data(), int(loop.size()), group0, 1);
  }
}

// Dual view of the closed mesh: for every primal vertex the ordered ring of corners
// around it (topoenc.SourceDual with ring_dir = +1), plus lookups the encoder needs.
struct SourceDual
{
  const PolyMesh& m;
  std::vector<int> corner_poly;  // corner -> polygon
  std::vector<int> ring_start;   // vertex -> offset into ring (CSR, nverts + 1)
  std::vector<int> ring;         // ring corners, vertex by vertex
  std::vector<int> ring_pos;     // corner -> position in its vertex's ring
  std::unordered_map<uint64_t, int> big_corner; // (polygon, vertex) -> corner, polygons > 8 sides

  explicit SourceDual(const PolyMesh& mesh);

  int ring_size(int v) const { return ring_start[size_t(v) + 1] - ring_start[size_t(v)]; }

  // Corner of polygon t at vertex v, or -1.
  int corner_of(int t, int v) const
  {
    const int b = m.poly_start[size_t(t)], n = m.size(t);
    if (n <= 8)
    {
      for (int k = 0; k < n; ++k)
        if (m.verts[size_t(b + k)] == v)
          return b + k;
      return -1;
    }
    auto it = big_corner.find(uint64_t(uint32_t(t)) << 32 | uint32_t(v));
    return it == big_corner.end() ? -1 : it->second;
  }
};

SourceDual::SourceDual(const PolyMesh& mesh)
  : m(mesh), corner_poly(mesh.verts.size()), ring_start(size_t(mesh.nverts) + 1, 0),
    ring(mesh.verts.size()), ring_pos(mesh.verts.size(), -1)
{
  const size_t ncorners = m.verts.size();
  for (int t = 0; t < m.npolys(); ++t)
  {
    const int b = m.poly_start[size_t(t)], n = m.size(t);
    for (int k = 0; k < n; ++k)
    {
      corner_poly[size_t(b + k)] = t;
      if (n > 8 && !big_corner.emplace(uint64_t(uint32_t(t)) << 32 | uint32_t(m.verts[size_t(b + k)]), b + k).second)
        throw EncodeError("repeated vertex in polygon");
    }
  }
  const EdgeIndex nxt(m.nverts, m.verts, next_vertices(m));

  // Corner counts per vertex size the rings; a vertex whose ring walk does not reach all
  // its corners has several fans (non-manifold vertex).
  std::vector<int> first(size_t(m.nverts), -1);
  for (size_t c = 0; c < ncorners; ++c)
  {
    const int v = m.verts[c];
    ++ring_start[size_t(v) + 1];
    if (first[size_t(v)] < 0)
      first[size_t(v)] = int(c);
  }
  for (int v = 0; v < m.nverts; ++v)
    ring_start[size_t(v) + 1] += ring_start[size_t(v)];

  for (int v = 0; v < m.nverts; ++v)
  {
    if (first[size_t(v)] < 0)
      continue;
    const int base = ring_start[size_t(v)], count = ring_size(v);
    int c = first[size_t(v)], len = 0;
    do
    {
      if (len == count)
        throw EncodeError("vertex ring does not close");
      ring[size_t(base + len)] = c;
      ring_pos[size_t(c)] = len++;
      // Neighbour across edge (prev, v): the polygon holding directed edge v -> prev.
      const int t = corner_poly[size_t(c)], b = m.poly_start[size_t(t)], n = m.size(t);
      const int prev = m.verts[size_t(b + pymod(c - b - 1, n))];
      c = nxt.find(v, prev);
      if (c < 0)
        throw EncodeError("open edge after closing");
    } while (c != first[size_t(v)]);
    if (len != count)
      throw EncodeError("non-manifold vertex (several fans)");
  }
}

// ===================================================================================
// 4. Topology encoder (topoenc.Encoder)
// ===================================================================================

// Symbol streams of the Topologically Compressed Rep Data.
struct Symbols
{
  std::array<std::vector<int64_t>, 8> deg;   // face degrees per valence context
  std::vector<int64_t> val, grp, vflags;     // per dual vertex
  std::array<std::vector<int64_t>, 8> masks; // attribute masks per context (ctx 7: low 30 bits)
  std::vector<int64_t> m7b, m7c;             // ctx 7 masks: bits 30..59 and 60..63
  std::vector<uint32_t> hd;                  // masks of faces with degree > 64
  std::vector<int64_t> ssym, spos;           // split offsets / positions
  std::vector<int> coord_order;              // primal vertex per dual face
  std::vector<int> attr_order;               // source corner per attribute
};

// Runs the Appendix-E decoder state machine, answering each symbol request from the
// source mesh and recording the symbol.  Dual vertex <-> source polygon, dual face <->
// primal vertex.  The symbol order is fixed by the state machine and identical to the
// Python; only the searches are indexed:
//   * dual-vertex slots live at poly_start[t] + slot, dual-face slots at
//     ring_start[pv] + slot, so slot <-> source corner is O(1) both ways;
//   * every slot write is checked against the source mesh (slot s of a dual vertex holds
//     the face of corner (rot + s), slot j of a dual face holds the polygon of ring
//     corner (rot + j)), which makes the "find slot" searches O(1) lookups;
//   * empty-slot counts, the first-empty-slot cursor, the context sums and the position
//     of each face in the active list are maintained incrementally.
class TopoEncoder
{
public:
  TopoEncoder(const SourceDual& src, const std::vector<int>& cattr,
              const std::vector<std::array<float, 3>>& normals)
    : S_(src), M_(src.m), cattr_(cattr), normals_(normals), vf_(M_.verts.size(), -1),
      fv_(M_.verts.size(), -1), visited_(size_t(M_.npolys()), 0), s2df_(size_t(M_.nverts), -1)
  {
    const size_t np = size_t(M_.npolys());
    dv_poly_.reserve(np);
    dv_rot_.reserve(np);
    vnk_.reserve(np);
    vtot_.reserve(np);
  }

  Symbols run();

private:
  // --- dual mesh accessors
  int vval(int v) const { return M_.size(dv_poly_[size_t(v)]); }
  int fdeg(int f) const { return S_.ring_size(df_vert_[size_t(f)]); }
  int& vf(int v, int s) { return vf_[size_t(M_.poly_start[size_t(dv_poly_[size_t(v)])] + s)]; }
  int& fv(int f, int j) { return fv_[size_t(S_.ring_start[size_t(df_vert_[size_t(f)])] + j)]; }
  int vcorner(int v, int s) const
  {
    const int t = dv_poly_[size_t(v)];
    return M_.poly_start[size_t(t)] + (dv_rot_[size_t(v)] + s) % M_.size(t);
  }
  int fcorner(int f, int j) const
  {
    const int pv = df_vert_[size_t(f)];
    return S_.ring[size_t(S_.ring_start[size_t(pv)] + (df_rot_[size_t(f)] + j) % S_.ring_size(pv))];
  }

  void set_vf(int v, int s, int f);
  void set_fv(int f, int j, int v);
  int find_vtx_slot(int f, int v) const;
  int find_face_slot(int v, int f, int corner) const;
  bool same_attr(int a, int b) const;

  // --- encoder steps (names follow topoenc.py)
  int ctx(int v) const;
  int new_vtx(int t, int rot);
  int io_vtx_init();
  int io_face(int v, int corner, int pv);
  void add_vtx_to_face(int v, int jF, int f, int iVS);
  int activate_f(int v, int slot);
  int activate_v(int f, int slot);
  void complete_v(int v, int iVS);
  int next_active();
  void complete_f(int f);

  const SourceDual& S_;
  const PolyMesh& M_;
  const std::vector<int>& cattr_;                    // corner -> attribute key (see same_attr)
  const std::vector<std::array<float, 3>>& normals_; // triangle corner normals

  // dual vertices
  std::vector<int> dv_poly_, dv_rot_, vnk_;
  std::vector<int64_t> vtot_;
  std::vector<int> vf_; // face per dual-vertex slot (indexed by corner storage)
  // dual faces
  std::vector<int> df_vert_, df_rot_, fempty_, fcursor_, apos_;
  std::vector<int> fv_; // dual vertex per dual-face slot (indexed by ring storage)

  std::vector<char> visited_, removed_;
  std::vector<int> s2df_;   // primal vertex -> dual face
  std::vector<int> active_; // active face list
  int init_cursor_ = 0;
  Symbols out_;
  std::vector<uint64_t> masks_raw7_;    // ctx-7 masks, split into 30/30/4 bits at the end
  std::vector<uint32_t> mask_scratch_; // attribute mask words of the face being emitted
};

// Attribute keys: >= 0 is a triangle corner (compared by normal value, like the Python's
// float tuples); < 0 is an unresolved cover corner keyed by its vertex.
bool TopoEncoder::same_attr(int a, int b) const
{
  if (a >= 0 && b >= 0)
  {
    const auto &na = normals_[size_t(a)], &nb = normals_[size_t(b)];
    return na[0] == nb[0] && na[1] == nb[1] && na[2] == nb[2];
  }
  return a == b;
}

void TopoEncoder::set_vf(int v, int s, int f)
{
  int& slot = vf(v, s);
  if (f >= 0 && M_.verts[size_t(vcorner(v, s))] != df_vert_[size_t(f)])
    throw EncodeError("dual vertex slot inconsistent with source");
  if (slot >= 0)
  {
    --vnk_[size_t(v)];
    vtot_[size_t(v)] -= fdeg(slot);
  }
  slot = f;
  if (f >= 0)
  {
    ++vnk_[size_t(v)];
    vtot_[size_t(v)] += fdeg(f);
  }
}

void TopoEncoder::set_fv(int f, int j, int v)
{
  int& slot = fv(f, j);
  if (S_.corner_poly[size_t(fcorner(f, j))] != dv_poly_[size_t(v)])
    throw EncodeError("dual face slot inconsistent with source");
  if (slot == -1)
    --fempty_[size_t(f)];
  slot = v;
}

// Slot of dual vertex v in face f, or -1 (Mesh.findVtxSlot).  Writes are validated, so
// v can only sit in the slot of the corner of its polygon at f's primal vertex.
int TopoEncoder::find_vtx_slot(int f, int v) const
{
  const int c = S_.corner_of(dv_poly_[size_t(v)], df_vert_[size_t(f)]);
  if (c < 0)
    return -1;
  const int j = pymod(S_.ring_pos[size_t(c)] - df_rot_[size_t(f)], fdeg(f));
  return fv_[size_t(S_.ring_start[size_t(df_vert_[size_t(f)])] + j)] == v ? j : -1;
}

// Slot of face f in dual vertex v (Mesh.findFaceSlot), where `corner` is the ring corner
// of f that holds v.
int TopoEncoder::find_face_slot(int v, int f, int corner) const
{
  const int t = dv_poly_[size_t(v)];
  const int s = pymod(corner - M_.poly_start[size_t(t)] - dv_rot_[size_t(v)], M_.size(t));
  return vf_[size_t(M_.poly_start[size_t(t)] + s)] == f ? s : -1;
}

int TopoEncoder::ctx(int v) const
{
  const int cval = vval(v), nk = vnk_[size_t(v)];
  const int64_t tot = vtot_[size_t(v)];
  if (cval == 3)
    return tot < nk * 6 ? 0 : (tot == nk * 6 ? 1 : 2);
  if (cval == 4)
    return tot < nk * 4 ? 3 : (tot == nk * 4 ? 4 : 5);
  return cval == 5 ? 6 : 7;
}

int TopoEncoder::new_vtx(int t, int rot)
{
  if (visited_[size_t(t)])
    throw EncodeError("polygon reached twice");
  visited_[size_t(t)] = 1;
  dv_poly_.push_back(t);
  dv_rot_.push_back(rot);
  vnk_.push_back(0);
  vtot_.push_back(0);
  out_.val.push_back(M_.size(t));
  out_.grp.push_back(M_.groups[size_t(t)]);
  out_.vflags.push_back(M_.flags[size_t(t)]);
  return int(dv_poly_.size()) - 1;
}

int TopoEncoder::io_vtx_init()
{
  while (init_cursor_ < M_.npolys() && visited_[size_t(init_cursor_)])
    ++init_cursor_;
  return init_cursor_ < M_.npolys() ? new_vtx(init_cursor_, 0) : -1;
}

// Emits the degree (and attribute mask) of the face at `corner` of dual vertex v; returns
// the new dual face, or -1 when the primal vertex already has one (a split follows).
int TopoEncoder::io_face(int v, int corner, int pv)
{
  const int c = ctx(v);
  if (s2df_[size_t(pv)] >= 0)
  {
    out_.deg[size_t(c)].push_back(0);
    return -1;
  }
  const int rot = S_.ring_pos[size_t(corner)], d = S_.ring_size(pv);
  const int* rg = &S_.ring[size_t(S_.ring_start[size_t(pv)])];
  out_.deg[size_t(c)].push_back(d);

  // Mask bit j: attribute changes between ring corners j-1 and j (starting at rot).
  std::vector<uint32_t>& mask = mask_scratch_;
  mask.assign(size_t((d + 31) / 32), 0);
  int na = 0;
  for (int j = 0; j < d; ++j)
  {
    const int kj = cattr_[size_t(rg[(rot + j) % d])], kp = cattr_[size_t(rg[(rot + j + d - 1) % d])];
    if (!same_attr(kj, kp))
      mask[size_t(j / 32)] |= uint32_t(1) << (j % 32), ++na;
  }
  if (na == 0)
    mask[0] = 1, na = 1;
  if (d <= 64)
  {
    const uint64_t m64 = uint64_t(mask[0]) | (mask.size() > 1 ? uint64_t(mask[1]) << 32 : 0);
    const int mc = std::min(7, std::max(0, d - 2));
    if (mc < 7)
      out_.masks[size_t(mc)].push_back(int64_t(m64));
    else
      masks_raw7_.push_back(m64);
  }
  else
    out_.hd.insert(out_.hd.end(), mask.begin(), mask.end());

  const int f = int(df_vert_.size());
  df_vert_.push_back(pv);
  df_rot_.push_back(rot);
  fempty_.push_back(d);
  fcursor_.push_back(0);
  apos_.push_back(-1);
  removed_.push_back(0);
  s2df_[size_t(pv)] = f;
  out_.coord_order.push_back(pv);
  for (int j = 0; j < d; ++j)
    if (mask[size_t(j / 32)] >> (j % 32) & 1)
      out_.attr_order.push_back(rg[(rot + j) % d]);
  return f;
}

void TopoEncoder::add_vtx_to_face(int v, int jF, int f, int iVS)
{
  const int d = fdeg(f), ccw = (iVS + 1) % d, cw = pymod(iVS - 1, d);
  set_fv(f, iVS, v);
  const int fp = fv(f, cw);
  if (fp != -1)
  {
    int ip = find_face_slot(fp, f, fcorner(f, cw));
    const int sl = (jF + 1) % vval(v);
    if (vf(v, sl) == -1)
    {
      ip = pymod(ip - 1, vval(fp));
      set_vf(v, sl, vf(fp, ip));
    }
  }
  const int fn = fv(f, ccw);
  if (fn != -1)
  {
    int in = find_face_slot(fn, f, fcorner(f, ccw));
    const int sl = pymod(jF - 1, vval(v));
    if (vf(v, sl) == -1)
    {
      in = (in + 1) % vval(fn);
      set_vf(v, sl, vf(fn, in));
    }
  }
}

int TopoEncoder::activate_f(int v, int slot)
{
  const int corner = vcorner(v, slot), pv = M_.verts[size_t(corner)];
  int f = io_face(v, corner, pv);
  if (f >= 0)
  {
    set_vf(v, slot, f);
    set_fv(f, 0, v);
    apos_[size_t(f)] = int(active_.size());
    active_.push_back(f);
    return f;
  }
  // Split: the face already exists; reference it by its offset from the active list end.
  f = s2df_[size_t(pv)];
  if (apos_[size_t(f)] < 0)
    throw EncodeError("split face no longer active");
  out_.ssym.push_back(int64_t(active_.size()) - apos_[size_t(f)]);
  const int j = pymod(S_.ring_pos[size_t(corner)] - df_rot_[size_t(f)], fdeg(f));
  out_.spos.push_back(j);
  set_vf(v, slot, f);
  add_vtx_to_face(v, slot, f, j);
  return f;
}

int TopoEncoder::activate_v(int f, int slot)
{
  const int corner = fcorner(f, slot), t = S_.corner_poly[size_t(corner)];
  const int v = new_vtx(t, corner - M_.poly_start[size_t(t)]);
  set_vf(v, 0, f);
  add_vtx_to_face(v, 0, f, slot);
  return v;
}

void TopoEncoder::complete_v(int v, int iVS)
{
  const int cval = vval(v);
  int vp = vf(v, 0), jp = iVS, i = 1;
  if (vp < 0)
    throw EncodeError("dual vertex without face");
  while (true)
  {
    const int vn = vf(v, i);
    if (vn == -1)
      break;
    jp = pymod(jp - 1, fdeg(vp));
    const int v2 = fv(vp, jp);
    if (v2 == -1)
      break;
    const int jn = pymod(find_vtx_slot(vn, v2) - 1, fdeg(vn));
    add_vtx_to_face(v, i, vn, jn);
    vp = vn;
    jp = jn;
    if (++i >= cval)
      return;
  }
  int ilast = i;
  vp = vf(v, 0);
  jp = iVS;
  i = cval - 1;
  while (true)
  {
    const int vn = vf(v, i);
    if (vn == -1)
      break;
    jp = (jp + 1) % fdeg(vp);
    const int v2 = fv(vp, jp);
    if (v2 == -1)
      break;
    const int jn = pymod(find_vtx_slot(vn, v2) + 1, fdeg(vn));
    add_vtx_to_face(v, i, vn, jn);
    vp = vn;
    jp = jn;
    if (--i < ilast)
      return;
  }
  for (; ilast <= i; ++ilast)
    activate_f(v, ilast);
}

// Picks, among the last 16 active faces, the one with the fewest empty slots (latest
// wins ties), dropping completed faces on the way.
int TopoEncoder::next_active()
{
  while (!active_.empty() && removed_[size_t(active_.back())])
  {
    apos_[size_t(active_.back())] = -1;
    active_.pop_back();
  }
  int best = -1, lo = std::numeric_limits<int>::max();
  int k = int(active_.size()) - 1;
  const int lim = std::max(0, int(active_.size()) - 16);
  while (k >= lim && k >= 0)
  {
    const int f = active_[size_t(k)];
    if (removed_[size_t(f)])
    {
      apos_[size_t(f)] = -1;
      for (size_t q = size_t(k) + 1; q < active_.size(); ++q)
        --apos_[size_t(active_[q])];
      active_.erase(active_.begin() + k);
      --k;
      continue;
    }
    if (fempty_[size_t(f)] < lo)
    {
      lo = fempty_[size_t(f)];
      best = f;
    }
    --k;
  }
  return best;
}

void TopoEncoder::complete_f(int f)
{
  const int d = fdeg(f);
  while (true)
  {
    int& j = fcursor_[size_t(f)];
    while (j < d && fv(f, j) != -1)
      ++j;
    if (j == d)
      break;
    const int slot = j;
    const int v = activate_v(f, slot);
    complete_v(v, slot);
  }
}

Symbols TopoEncoder::run()
{
  while (true)
  {
    const int v = io_vtx_init();
    if (v == -1)
      break;
    for (int i = 0, n = vval(v); i < n; ++i)
      activate_f(v, i);
    while (true)
    {
      const int f = next_active();
      if (f == -1)
        break;
      complete_f(f);
      removed_[size_t(f)] = 1;
    }
  }
  for (uint64_t x : masks_raw7_)
  {
    out_.masks[7].push_back(int64_t(x & 0x3fffffff));
    out_.m7b.push_back(int64_t((x >> 30) & 0x3fffffff));
    out_.m7c.push_back(int64_t((x >> 60) & 0xf));
  }
  return std::move(out_);
}

// ===================================================================================
// 5. Shape LOD element
// ===================================================================================

const uint64_t BIND = 0x4a; // coordinates xyz + normals + vertex flags (as Onshape)

Guid make_guid(uint32_t d1, uint16_t d2, uint16_t d3, std::array<uint8_t, 8> b8)
{
  Guid g{};
  for (int i = 0; i < 4; ++i)
    g[size_t(i)] = uint8_t(d1 >> (8 * i));
  g[4] = uint8_t(d2), g[5] = uint8_t(d2 >> 8), g[6] = uint8_t(d3), g[7] = uint8_t(d3 >> 8);
  std::copy(b8.begin(), b8.end(), g.begin() + 8);
  return g;
}

const std::array<uint8_t, 8> JT_TAIL{0x9b, 0x6b, 0x00, 0x80, 0xc7, 0xbb, 0x59, 0x97};
const std::array<uint8_t, 8> META_TAIL{0xa5, 0x06, 0x00, 0x60, 0x97, 0xbd, 0xc6, 0xe1};
const Guid T_PARTITION = make_guid(0x10dd103e, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_GROUP = make_guid(0x10dd101b, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_PART = make_guid(0xce357244, 0x38fb, 0x11d1, META_TAIL);
const Guid T_META = make_guid(0xce357245, 0x38fb, 0x11d1, META_TAIL);
const Guid T_INST = make_guid(0x10dd102a, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_RLOD = make_guid(0x10dd104c, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_TSNODE = make_guid(0x10dd1077, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_MAT = make_guid(0x10dd1030, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_XFORM = make_guid(0x10dd1083, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_STR = make_guid(0x10dd106e, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_LL = make_guid(0xe0b05be5, 0xfbbd, 0x11d1, {0xa3, 0xa7, 0x00, 0xaa, 0x00, 0xd1, 0x09, 0x54});
const Guid T_TSLOD = make_guid(0x10dd10ab, 0x2ac8, 0x11d1, JT_TAIL);
const Guid T_XT = make_guid(0x873a70e0, 0x2ac9, 0x11d1, JT_TAIL);

// Logical element: int32 length | object type GUID | base type | object id | data.
void element(Bytes& out, const Guid& type, int base, int id, const std::string& data)
{
  out.i32(int64_t(16 + 1 + 4 + data.size()));
  out.guid(type);
  out.u8(uint8_t(base));
  out.i32(id);
  out.raw(data);
}

// End-of-elements marker.
void eoe(Bytes& out)
{
  out.i32(16);
  out.guid(Guid{0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff});
}

// Every late-loaded segment payload ends with EOE + an empty property table (Onshape).
void ll_trailer(Bytes& out)
{
  eoe(out);
  out.i16(1);
  out.i32(0);
}

// Topology symbols plus the vertex records referenced by them.
struct EncodedMesh
{
  Symbols sym;
  std::vector<std::array<double, 3>> coords;
  std::vector<std::array<float, 3>> normals;
  std::string mode;
};

// Connected encoding of a triangle mesh (topo_symbols' try-branch); throws EncodeError.
EncodedMesh encode_connected(const std::vector<std::array<double, 3>>& positions,
                             const std::vector<std::array<int, 3>>& tris,
                             const std::vector<int>& groups,
                             const std::vector<std::array<float, 3>>& corner_normals)
{
  PolyMesh m;
  m.nverts = int(positions.size());
  m.ntris = int(tris.size());
  m.verts.reserve(tris.size() * 3 + 64);
  for (size_t t = 0; t < tris.size(); ++t)
  {
    const auto& tri = tris[t];
    if (tri[0] == tri[1] || tri[1] == tri[2] || tri[2] == tri[0])
      throw EncodeError("degenerate triangle");
    m.add(tri.data(), 3, groups[t], 0);
  }
  close_mesh(m);
  const SourceDual sd(m);

  // Corner attribute keys: triangle corners are their own normals; cover corners reuse
  // the attribute of the preceding (clockwise in the ring) triangle corner.
  std::vector<int> cattr(m.verts.size());
  const int ntc = m.ntris * 3;
  for (int c = 0; c < int(cattr.size()); ++c)
    cattr[size_t(c)] = c < ntc ? c : -1 - m.verts[size_t(c)];
  for (int v = 0; v < m.nverts; ++v)
  {
    const int n = sd.ring_size(v);
    const int* rg = &sd.ring[size_t(sd.ring_start[size_t(v)])];
    for (int i = 0; i < n; ++i)
    {
      if (rg[i] < ntc)
        continue;
      for (int j = 1; j < n; ++j)
      {
        const int c2 = rg[pymod(i - j, n)];
        if (c2 < ntc)
        {
          cattr[size_t(rg[i])] = c2;
          break;
        }
      }
    }
  }

  EncodedMesh em;
  em.sym = TopoEncoder(sd, cattr, corner_normals).run();
  em.coords.reserve(em.sym.coord_order.size());
  for (int v : em.sym.coord_order)
    em.coords.push_back(positions[size_t(v)]);
  em.normals.reserve(em.sym.attr_order.size());
  for (int c : em.sym.attr_order)
    em.normals.push_back(c < ntc ? corner_normals[size_t(c)] : std::array<float, 3>{0.0f, 0.0f, 1.0f});
  em.mode = "connected";
  return em;
}

// topo_symbols: connected encoding, else one pillow (triangle + cover) per triangle.
EncodedMesh topo_symbols(const Part& p)
{
  std::vector<std::array<float, 3>> cn(p.tris.size() * 3);
  for (size_t t = 0; t < p.tris.size(); ++t)
    for (size_t k = 0; k < 3; ++k)
      cn[3 * t + k] = p.corner_normals[t][k];
  try
  {
    return encode_connected(p.positions, p.tris, p.groups, cn);
  }
  catch (const EncodeError& e)
  {
    std::vector<std::array<double, 3>> P(p.tris.size() * 3);
    std::vector<std::array<int, 3>> T(p.tris.size());
    for (size_t t = 0; t < p.tris.size(); ++t)
    {
      const int b = int(3 * t);
      T[t] = {b, b + 1, b + 2};
      for (size_t k = 0; k < 3; ++k)
        P[3 * t + k] = p.positions[size_t(p.tris[t][k])];
    }
    EncodedMesh em = encode_connected(P, T, p.groups, cn);
    em.mode = std::string("pillow(") + e.what() + ")";
    return em;
  }
}

// Exponent (u >> 23) and mantissa (u & 0x07FFFFFF) streams of float32 bit patterns; the
// decoder ORs them back together, so the overlapping exponent bits are harmless.
void float_streams(const std::vector<uint32_t>& bits, std::vector<int64_t>& e, std::vector<int64_t>& m)
{
  e.resize(bits.size());
  m.resize(bits.size());
  for (size_t i = 0; i < bits.size(); ++i)
  {
    e[i] = bits[i] >> 23;
    m[i] = bits[i] & 0x07ffffff;
  }
}

// Tri-Strip Set Shape LOD element (data size returned for the shape node).
std::string shape_lod_element(const Part& p, size_t& data_size, std::string& mode)
{
  const EncodedMesh em = topo_symbols(p);
  const Symbols& S = em.sym;
  mode = em.mode;

  Bytes d;
  d.i16(1);
  d.i16(1);
  d.u64(BIND);
  d.i16(2);
  d.i32(0x6ecc2581);
  d.i16(2);

  // Topologically compressed rep data.
  for (const auto& v : S.deg)
    cdp2_bitlength(d, v);
  cdp2_bitlength(d, S.val);
  cdp2_bitlength(d, S.grp);
  cdp2_bitlength(d, lag1(S.vflags));
  for (const auto& v : S.masks)
    cdp2_bitlength(d, v);
  cdp2_bitlength(d, S.m7b);
  cdp2_bitlength(d, S.m7c);
  d.i32(int64_t(S.hd.size()));
  for (uint32_t w : S.hd)
    d.u32(w);
  cdp2_bitlength(d, lag1(S.ssym));
  cdp2_bitlength(d, S.spos);
  uint32_t h = 0;
  for (const auto& v : S.deg)
    h = hash32(v, h);
  h = hash32(S.val, h);
  h = hash32(S.grp, h);
  h = hash16(S.vflags, h);
  for (const auto& v : S.masks)
    h = hash32(v, h);
  h = hash32(S.m7b, h);
  h = hash32(S.m7c, h);
  h = hash32(S.hd, h);
  h = hash32(S.ssym, h);
  h = hash32(S.spos, h);
  d.u32(h);

  // Topologically compressed vertex records.
  const size_t nt = em.coords.size(), na = em.normals.size();
  d.u64(BIND);
  d.zeros(4);
  d.i32(int64_t(nt));
  d.i32(int64_t(na));

  // Coordinates: lossless (point quantizer bits 0), Lag1 on exponents and mantissae.
  d.i32(int64_t(nt));
  d.u8(3);
  std::array<std::vector<uint32_t>, 3> cbits;
  for (size_t k = 0; k < 3; ++k)
  {
    double mn = em.coords[0][k], mx = mn;
    cbits[k].resize(nt);
    for (size_t i = 0; i < nt; ++i)
    {
      mn = std::min(mn, em.coords[i][k]);
      mx = std::max(mx, em.coords[i][k]);
      cbits[k][i] = float_bits(float(em.coords[i][k]));
    }
    d.f32(mn);
    d.f32(mx);
    d.u8(0);
  }
  std::vector<int64_t> e, m;
  for (size_t k = 0; k < 3; ++k)
  {
    float_streams(cbits[k], e, m);
    cdp2_bitlength(d, lag1(e));
    cdp2_bitlength(d, lag1(m));
  }
  uint32_t ch = 0;
  for (size_t k = 0; k < 3; ++k)
    for (uint32_t u : cbits[k])
      ch = hash32(&u, 1, ch);
  d.u32(ch);

  // Normals: lossless (quantization bits 0), no predictor.
  d.i32(int64_t(na));
  d.u8(3);
  d.u8(0);
  std::array<std::vector<uint32_t>, 3> nbits;
  for (size_t k = 0; k < 3; ++k)
  {
    nbits[k].resize(na);
    for (size_t i = 0; i < na; ++i)
      nbits[k][i] = float_bits(em.normals[i][k]);
    float_streams(nbits[k], e, m);
    cdp2_bitlength(d, e);
    cdp2_bitlength(d, m);
  }
  uint32_t nh = 0;
  for (size_t k = 0; k < 3; ++k)
    for (uint32_t u : nbits[k])
      nh = hash32(&u, 1, nh);
  d.u32(nh);

  // Vertex flag array (Onshape writes all 1).
  d.i32(int64_t(na));
  cdp2_bitlength(d, std::vector<int64_t>(na, 1));

  // Rep-Data-V2 trailer (version 1, bindings) + Tri-Strip LOD version 1.
  d.i16(1);
  d.u64(BIND);
  d.i16(1);

  data_size = d.size();
  Bytes el;
  element(el, T_TSLOD, 4, 0, d.str());
  return el.str();
}

// ===================================================================================
// 6. LSG segment and file assembly
// ===================================================================================

// Segment: GUID | type | length (incl. 24-byte header) | payload.
std::string segment(const Guid& g, int type, const std::string& payload)
{
  Bytes s;
  s.guid(g);
  s.i32(type);
  s.i32(int64_t(24 + payload.size()));
  s.raw(payload);
  return s.str();
}

// Whole-segment zlib: compression flag 2 | compressed length + 1 | algorithm 2 | stream.
std::string zlib_segment(const Guid& g, int type, const std::string& payload)
{
  const std::string z = zlib_stored(payload);
  Bytes data;
  data.i32(2);
  data.i32(int64_t(z.size() + 1));
  data.u8(2);
  data.raw(z);
  return segment(g, type, data.str());
}

// Property atoms (deduplicated by kind + value) and the element property table.
class Properties
{
public:
  explicit Properties(int& next_id) : next_id_(next_id) {}

  void add(int eid, const std::string& key, const std::string& value)
  {
    const int k = string_atom(key);
    table(eid).emplace_back(k, string_atom(value));
  }
  void add_late_loaded(int eid, const std::string& key, const Guid& seg, int seg_type)
  {
    const int k = string_atom(key);
    const int id = next_id_++;
    Bytes d;
    atom_header(d);
    d.guid(seg);
    d.i32(seg_type);
    d.i32(0);
    d.i32(1);
    element(atoms_, T_LL, 8, id, d.str());
    table(eid).emplace_back(k, id);
  }

  // Atom elements followed by EOE.
  void write_atoms(Bytes& out) const
  {
    out.raw(atoms_.str());
    eoe(out);
  }
  void write_table(Bytes& out) const
  {
    out.i16(1);
    out.i32(int64_t(order_.size()));
    for (int eid : order_)
    {
      out.i32(eid);
      for (const auto& kv : rows_.at(eid))
      {
        out.i32(kv.first);
        out.i32(kv.second);
      }
      out.i32(0);
    }
  }

private:
  static void atom_header(Bytes& d)
  {
    d.i16(1);
    d.u32(0x40000000);
    d.i16(1);
  }
  int string_atom(const std::string& s)
  {
    auto it = strings_.find(s);
    if (it != strings_.end())
      return it->second;
    const int id = next_id_++;
    strings_.emplace(s, id);
    Bytes d;
    atom_header(d);
    d.mbstring(s);
    element(atoms_, T_STR, 5, id, d.str());
    return id;
  }
  std::vector<std::pair<int, int>>& table(int eid)
  {
    auto it = rows_.find(eid);
    if (it == rows_.end())
    {
      order_.push_back(eid);
      it = rows_.emplace(eid, std::vector<std::pair<int, int>>{}).first;
    }
    return it->second;
  }

  int& next_id_;
  Bytes atoms_;
  std::unordered_map<std::string, int> strings_;
  std::vector<int> order_; // elements in first-property order
  std::unordered_map<int, std::vector<std::pair<int, int>>> rows_;
};

void base_node(Bytes& d, const std::vector<int>& attrs)
{
  d.i16(1);
  d.u32(0);
  d.i32(int64_t(attrs.size()));
  for (int a : attrs)
    d.i32(a);
}

void group_node(Bytes& d, const std::vector<int>& children, const std::vector<int>& attrs = {})
{
  base_node(d, attrs);
  d.i16(1);
  d.i32(int64_t(children.size()));
  for (int c : children)
    d.i32(c);
}

void bbox(Bytes& d, const std::array<double, 3>& mn, const std::array<double, 3>& mx)
{
  for (double x : mn)
    d.f32(x);
  for (double x : mx)
    d.f32(x);
}

void material(Bytes& out, int id, const std::array<float, 3>& rgb, double spec)
{
  Bytes d;
  d.i16(1);
  d.u8(8);
  d.u32(0);
  d.i16(2);
  d.u16(0x3980);
  for (double x : {0.2, 0.2, 0.2, 1.0})
    d.f32(x);
  for (float x : rgb)
    d.f32(x);
  d.f32(1.0);
  for (double x : {spec, spec, spec, 1.0})
    d.f32(x);
  for (double x : {0.0, 0.0, 0.0, 1.0})
    d.f32(x);
  d.f32(15.0);
  d.f32(0.0);
  element(out, T_MAT, 3, id, d.str());
}

void identity_transform(Bytes& out, int id)
{
  Bytes d;
  d.i16(1);
  d.u8(8);
  d.u32(0);
  d.i16(1);
  d.u16(0);
  element(out, T_XFORM, 3, id, d.str());
}

// Product rounded to double.  The volatile store keeps the compiler from fusing it into
// an FMA (-ffp-contract with FMA-capable -march), which would round differently from the
// Python reference.
inline double rounded_mul(double a, double b)
{
  volatile double p = a * b;
  return p;
}

void validate(const std::vector<Part>& parts)
{
  if (parts.empty())
    throw std::invalid_argument("jt::write: no parts");
  for (const Part& p : parts)
  {
    if (p.tris.empty())
      throw std::invalid_argument("jt::write: part '" + p.name + "' has no triangles");
    if (p.groups.size() != p.tris.size() || p.corner_normals.size() != p.tris.size())
      throw std::invalid_argument("jt::write: part '" + p.name + "': groups/corner_normals size != tris size");
    for (const auto& t : p.tris)
      for (int v : t)
        if (v < 0 || size_t(v) >= p.positions.size())
          throw std::invalid_argument("jt::write: part '" + p.name + "': vertex index out of range");
  }
}

} // namespace

std::string write(const std::vector<Part>& parts, const std::string& file_name,
                  const std::array<uint8_t, 16>* fixed_guids, std::vector<std::string>* part_modes)
{
  validate(parts);
  GuidSource guids(fixed_guids);
  struct Seg
  {
    Guid guid;
    int type;
    std::string bytes;
  };
  std::vector<Seg> segs;
  int next_id = 1; // 0 is the partition node
  Properties props(next_id);
  Bytes body;
  std::vector<int> root_children;
  const bool multi = parts.size() > 1;
  const double big = 1e30;
  std::array<double, 3> allmin{big, big, big}, allmax{-big, -big, -big};
  double totarea = 0;
  int64_t totv = 0, totp = 0;
  if (part_modes)
    part_modes->clear();

  for (size_t pi = 0; pi < parts.size(); ++pi)
  {
    const Part& p = parts[pi];
    std::array<double, 3> mn = p.positions[0], mx = p.positions[0];
    for (const auto& q : p.positions)
      for (size_t k = 0; k < 3; ++k)
      {
        mn[k] = std::min(mn[k], q[k]);
        mx[k] = std::max(mx[k], q[k]);
      }
    // Area as the Python computes it (same operation order; pow(x, 0.5) like x ** .5).
    // Products go through rounded_mul so FMA contraction cannot change the result.
    double area = 0;
    for (const auto& t : p.tris)
    {
      const auto &A = p.positions[size_t(t[0])], &B = p.positions[size_t(t[1])], &C = p.positions[size_t(t[2])];
      const double e1[3] = {B[0] - A[0], B[1] - A[1], B[2] - A[2]};
      const double e2[3] = {C[0] - A[0], C[1] - A[1], C[2] - A[2]};
      const double cr[3] = {rounded_mul(e1[1], e2[2]) - rounded_mul(e1[2], e2[1]),
                            rounded_mul(e1[2], e2[0]) - rounded_mul(e1[0], e2[2]),
                            rounded_mul(e1[0], e2[1]) - rounded_mul(e1[1], e2[0])};
      const double sq = rounded_mul(cr[0], cr[0]) + rounded_mul(cr[1], cr[1]) + rounded_mul(cr[2], cr[2]);
      area += 0.5 * std::pow(sq, 0.5); // 0.5 * x is exact, so a fused add is harmless
    }

    size_t lod_size = 0;
    std::string mode;
    const std::string lod = shape_lod_element(p, lod_size, mode);
    if (part_modes)
      part_modes->push_back(mode);
    const Guid shp_guid = guids.next();
    {
      Bytes payload;
      payload.raw(lod);
      ll_trailer(payload);
      segs.push_back({shp_guid, 7, segment(shp_guid, 7, payload.str())});
    }
    const int64_t nv = int64_t(p.tris.size()) * 3, npoly = int64_t(p.tris.size());
    const int part_id = next_id++, mat_id = next_id++, xf_id = next_id++, rlod_id = next_id++,
              grp_id = next_id++, shp_id = next_id++, smat_id = next_id++;

    Bytes d;
    if (multi)
    {
      const int inst_id = next_id++;
      root_children.push_back(inst_id);
      base_node(d, {mat_id, xf_id});
      d.i16(1);
      d.i32(part_id);
      element(body, T_INST, 0, inst_id, d.str());
      material(body, mat_id, p.rgb, 0.0);
      identity_transform(body, xf_id);
      d = Bytes();
      group_node(d, {rlod_id});
      d.i16(1);
      d.i16(1);
      d.i32(99);
      element(body, T_PART, 1, part_id, d.str());
      props.add(inst_id, "JT_PROP_NAME", p.name + ".part;0;" + std::to_string(pi) + ":");
    }
    else
    {
      root_children.push_back(part_id);
      group_node(d, {rlod_id}, {mat_id, xf_id});
      d.i16(1);
      d.i16(1);
      d.i32(99);
      element(body, T_PART, 1, part_id, d.str());
      material(body, mat_id, p.rgb, 0.0);
      identity_transform(body, xf_id);
    }
    d = Bytes();
    group_node(d, {grp_id});
    d.i16(1);
    d.i32(0); // empty reserved-field vector
    d.i32(0);
    d.i16(1);
    d.i32(0); // empty range limits
    for (int k = 0; k < 3; ++k)
      d.f32(0.0); // center
    element(body, T_RLOD, 1, rlod_id, d.str());
    d = Bytes();
    group_node(d, {shp_id});
    element(body, T_GROUP, 1, grp_id, d.str());

    d = Bytes();
    base_node(d, {smat_id});
    d.i16(1);
    bbox(d, mn, mx);
    bbox(d, mn, mx);
    d.f32(area);
    d.i32(nv);
    d.i32(nv);
    d.i32(1);
    d.i32(1);
    d.i32(npoly);
    d.i32(npoly);
    d.i32(int64_t(lod_size));
    d.f32(0.0);
    d.i16(2);
    d.u64(BIND);
    d.zeros(4);
    d.u64(BIND);
    element(body, T_TSNODE, 2, shp_id, d.str());
    material(body, smat_id, p.rgb, 0.75);

    props.add_late_loaded(shp_id, "JT_LLPROP_SHAPEIMPL", shp_guid, 7);
    props.add(part_id, "JT_PROP_MEASUREMENT_UNITS", "millimeters");
    if (!p.xt.empty())
    {
      const Guid xg = guids.next();
      Bytes x;
      x.i32(2);
      x.i32(p.xt_version[0]);
      x.i32(p.xt_version[1]);
      x.i32(p.xt_version[2]);
      x.i32(int64_t(p.xt.size()));
      x.raw(p.xt);
      Bytes payload;
      element(payload, T_XT, 9, 0, x.str());
      ll_trailer(payload);
      segs.push_back({xg, 17, zlib_segment(xg, 17, payload.str())});
      props.add(part_id, "JT_PROP_ORIGINATING_BREPTYPE", "XTBrep");
      props.add_late_loaded(part_id, "JT_LLPROP_XTBREP", xg, 17);
    }
    for (size_t k = 0; k < 3; ++k)
    {
      allmin[k] = std::min(allmin[k], mn[k]);
      allmax[k] = std::max(allmax[k], mx[k]);
    }
    totarea += area;
    totv += nv;
    totp += npoly;
  }

  Bytes head;
  std::vector<int> pchildren;
  std::string pname;
  int nhead = 0;
  if (multi)
  {
    const int asm_id = next_id++;
    Bytes d;
    group_node(d, root_children);
    d.i16(1);
    element(head, T_META, 1, asm_id, d.str());
    nhead = 1;
    props.add(asm_id, "JT_PROP_MEASUREMENT_UNITS", "millimeters");
    pchildren = {asm_id};
    pname = "Assembly.asm;0;0:";
  }
  else
  {
    pchildren = root_children;
    pname = parts[0].name + ".part;0;0:";
  }
  // Node count as in the Python: body elements (8 per instanced part, 7 per single part)
  // + head + partition.
  const int64_t nnodes = int64_t(parts.size()) * (multi ? 8 : 7) + nhead + 1;

  Bytes lsg;
  {
    Bytes d;
    group_node(d, pchildren);
    d.i32(1); // partition flags: untransformed bbox present
    d.mbstring(file_name.substr(file_name.find_last_of('/') == std::string::npos ? 0 : file_name.find_last_of('/') + 1));
    bbox(d, allmin, allmax);
    d.f32(totarea);
    d.i32(totv);
    d.i32(totv);
    d.i32(nnodes);
    d.i32(nnodes);
    d.i32(totp);
    d.i32(totp);
    bbox(d, allmin, allmax);
    element(lsg, T_PARTITION, 1, 0, d.str());
  }
  props.add(0, "JT_PROP_NAME", pname);
  lsg.raw(head.str());
  lsg.raw(body.str());
  eoe(lsg);
  props.write_atoms(lsg);
  props.write_table(lsg);

  const Guid lsg_guid = guids.next();
  segs.insert(segs.begin(), Seg{lsg_guid, 1, zlib_segment(lsg_guid, 1, lsg.str())});

  // Header: 80-byte version string, byte order 0, reserved 0, TOC offset, LSG GUID.
  std::string ver = "Version 9.5 JT  DM 10.7.1.0";
  ver.resize(75, ' ');
  ver += " \n\r\n ";
  Bytes file;
  file.raw(ver);
  file.u8(0);
  file.i32(0);
  const int64_t toc_off = 105;
  file.i32(toc_off);
  file.guid(lsg_guid);
  file.i32(int64_t(segs.size()));
  int64_t off = toc_off + 4 + 28 * int64_t(segs.size());
  for (const Seg& s : segs)
  {
    file.guid(s.guid);
    file.i32(off);
    file.i32(int64_t(s.bytes.size()));
    file.u32(uint32_t(s.type) << 24);
    off += int64_t(s.bytes.size());
  }
  for (const Seg& s : segs)
    file.raw(s.bytes);
  return std::move(file.str());
}

} // namespace jt
} // namespace noshape
