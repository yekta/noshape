// PVZ writer implementation; see pvz_writer.hpp and /root/kern-docs/pvz/format-notes.md.
#include "pvz_writer.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <map>
#include <set>
#include <stdexcept>
#include <unordered_map>

namespace noshape {
namespace pvz {

namespace {

// ---------------------------------------------------------------------------------
// byte buffer helpers (little-endian)
// ---------------------------------------------------------------------------------
struct Buf {
  std::string b;
  void u8(uint32_t v) { b.push_back(static_cast<char>(v & 0xff)); }
  void u16(uint32_t v) { u8(v); u8(v >> 8); }
  void u32(uint32_t v) { u16(v & 0xffff); u16(v >> 16); }
  void i32(int32_t v) { u32(static_cast<uint32_t>(v)); }
  void u64(uint64_t v) { u32(static_cast<uint32_t>(v)); u32(static_cast<uint32_t>(v >> 32)); }
  void f32(double v) { float f = static_cast<float>(v); uint32_t u; std::memcpy(&u, &f, 4); u32(u); }
  void f64(double v) { uint64_t u; std::memcpy(&u, &v, 8); u64(u); }
  void raw(const std::string& s) { b += s; }
  void varint(uint64_t v) {      // LEB128 (PVS)
    for (;;) {
      uint32_t c = v & 0x7f; v >>= 7;
      if (v) { u8(c | 0x80); } else { u8(c); return; }
    }
  }
};

void fail(const std::string& m) { throw std::invalid_argument("pvz::write: " + m); }

// ---------------------------------------------------------------------------------
// CRC-32 and stored deflate / gzip
// ---------------------------------------------------------------------------------
uint32_t crc32(const std::string& s)
{
  static uint32_t table[256];
  static bool init = false;
  if (!init) {
    for (uint32_t i = 0; i < 256; ++i) {
      uint32_t c = i;
      for (int k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
      table[i] = c;
    }
    init = true;
  }
  uint32_t c = 0xffffffffu;
  for (unsigned char ch : s) c = table[(c ^ ch) & 0xff] ^ (c >> 8);
  return c ^ 0xffffffffu;
}

// Raw deflate stream made of stored blocks (RFC 1951 3.2.4), max 65535 bytes each.
std::string deflate_stored(const std::string& data)
{
  Buf o;
  size_t pos = 0;
  do {
    size_t n = std::min<size_t>(65535, data.size() - pos);
    bool final = pos + n == data.size();
    o.u8(final ? 1 : 0);              // BFINAL, BTYPE = 00, then pad to byte boundary
    o.u16(static_cast<uint32_t>(n));
    o.u16(static_cast<uint32_t>(~n & 0xffff));
    o.b.append(data, pos, n);
    pos += n;
  } while (pos < data.size());
  return o.b;
}

// gzip member (RFC 1952) exactly like zlib's wrapper: no name, mtime 0, XFL 0, OS 3.
std::string gzip_stored(const std::string& data)
{
  Buf o;
  o.u8(0x1f); o.u8(0x8b); o.u8(8); o.u8(0); o.u32(0); o.u8(0); o.u8(3);
  o.raw(deflate_stored(data));
  o.u32(crc32(data));
  o.u32(static_cast<uint32_t>(data.size()));
  return o.b;
}

// ---------------------------------------------------------------------------------
// OL record tree
// ---------------------------------------------------------------------------------
constexpr uint32_t kContainer = 0x01000000u;

struct Rec {               // a record whose children (if container) follow inline
  Buf out;
  void leaf(uint32_t type, const Buf& payload) { out.u32(type); out.u32(static_cast<uint32_t>(payload.b.size())); out.raw(payload.b); }
  void open(uint32_t type, const Buf& payload) { leaf(type | kContainer, payload); }
  void close() { out.u32(0); out.u32(0); }
};

void put_olstr(Buf& b, const std::string& s)
{
  b.u32(static_cast<uint32_t>(s.size() + 1));
  b.raw(s);
  b.u8(0);
}

int dim_check(const NurbsCurve& c, int dim, const char* what)
{
  size_t sum = 0;
  for (int m : c.mults) sum += static_cast<size_t>(m);
  if (c.knots.size() != c.mults.size()) fail(std::string(what) + ": knots/mults size mismatch");
  if (c.poles.size() % dim) fail(std::string(what) + ": pole array size");
  size_t np = c.poles.size() / dim;
  if (sum != np + static_cast<size_t>(c.degree) + 1) fail(std::string(what) + ": sum(mults) != poles + degree + 1");
  if (!c.weights.empty() && c.weights.size() != np) fail(std::string(what) + ": weights size");
  return static_cast<int>(np);
}

void put_knots(Buf& b, const std::vector<double>& knots, const std::vector<int>& mults)
{
  uint32_t n = 0;
  for (int m : mults) n += static_cast<uint32_t>(m);
  b.u32(n);
  for (size_t i = 0; i < knots.size(); ++i)
    for (int k = 0; k < mults[i]; ++k) b.f32(knots[i]);
}

// order, knots, npoles, homogeneous poles
void put_nurbs_curve_body(Buf& b, const NurbsCurve& c, int dim, const char* what)
{
  int np = dim_check(c, dim, what);
  b.u32(static_cast<uint32_t>(c.degree + 1));
  put_knots(b, c.knots, c.mults);
  b.u32(static_cast<uint32_t>(np));
  for (int i = 0; i < np; ++i) {
    double w = c.weights.empty() ? 1.0 : c.weights[i];
    for (int k = 0; k < dim; ++k) b.f32(c.poles[i * dim + k] * w);
    b.f32(w);
  }
}

void put_nurbs_surface_body(Buf& b, const NurbsSurface& s)
{
  auto sum = [](const std::vector<int>& m) { size_t t = 0; for (int x : m) t += static_cast<size_t>(x); return t; };
  if (s.uknots.size() != s.umults.size() || s.vknots.size() != s.vmults.size()) fail("surface knots/mults size");
  if (sum(s.umults) != static_cast<size_t>(s.nu + s.udegree + 1) || sum(s.vmults) != static_cast<size_t>(s.nv + s.vdegree + 1))
    fail("surface knot count != poles + degree + 1");
  size_t np = static_cast<size_t>(s.nu) * static_cast<size_t>(s.nv);
  if (s.poles.size() != 3 * np) fail("surface pole array size");
  if (!s.weights.empty() && s.weights.size() != np) fail("surface weights size");
  b.u32(static_cast<uint32_t>(s.udegree + 1)); put_knots(b, s.uknots, s.umults);
  b.u32(static_cast<uint32_t>(s.vdegree + 1)); put_knots(b, s.vknots, s.vmults);
  b.u32(static_cast<uint32_t>(s.nu)); b.u32(static_cast<uint32_t>(s.nv));
  for (size_t i = 0; i < np; ++i) {
    double w = s.weights.empty() ? 1.0 : s.weights[i];
    for (int k = 0; k < 3; ++k) b.f32(s.poles[i * 3 + k] * w);
    b.f32(w);
  }
}

Buf surface_payload(const Surface& s)
{
  Buf b;
  uint8_t rational = s.nurbs.weights.empty() ? 0 : 1;
  if (s.kind == SurfaceKind::Nurbs) {
    b.u8(4); b.u8(0); b.u8(s.nurbs_interval ? 1 : 0); b.u8(rational);
    put_nurbs_surface_body(b, s.nurbs);
    if (s.nurbs_interval) { b.f32((*s.nurbs_interval)[0]); b.f32((*s.nurbs_interval)[1]); }
    b.u8(0);
    return b;
  }
  b.u8(static_cast<uint8_t>(s.kind)); b.u8(0x80); b.u8(s.nurbs_uv_swapped ? 0 : 1);
  for (const Vec3* v : {&s.origin, &s.axis1, &s.axis2, &s.axis3})
    for (double x : *v) b.f32(x);
  b.f32(s.urange[0]); b.f32(s.urange[1]); b.f32(s.vrange[0]); b.f32(s.vrange[1]);
  b.u8(4); b.u8(0); b.u8(0); b.u8(rational);
  put_nurbs_surface_body(b, s.nurbs);
  return b;
}

std::string hex2(float c)
{
  long v = std::lround(static_cast<double>(c) * 255.0);
  if (v < 0) v = 0;
  if (v > 255) v = 255;
  char buf[4];
  std::snprintf(buf, sizeof buf, "%02lx", v);
  return buf;
}

std::string appearance_name(const Material& m)
{
  if (!m.appearance_name.empty()) return m.appearance_name;
  std::string s = "A";
  for (float c : m.ambient) s += hex2(c);
  s += "D";
  for (float c : m.diffuse) s += hex2(c);
  return s;
}

struct IdTable {
  std::vector<std::string> values;
  std::unordered_map<std::string, uint32_t> index;
  int ref(const std::string& v) {
    if (v.empty()) return -1;
    auto it = index.find(v);
    if (it != index.end()) return static_cast<int>(it->second);
    index[v] = static_cast<uint32_t>(values.size());
    values.push_back(v);
    return static_cast<int>(values.size() - 1);
  }
};

void put_idref(Rec& r, int idx)
{
  if (idx < 0) return;
  Buf b; b.u32(0); b.u32(1); b.u32(static_cast<uint32_t>(idx));
  r.leaf(0x0a, b);
}

// Builds the three decompressed OL sections of a part.
std::array<std::string, 3> build_ol_sections(const Part& p, bool write_opacity)
{
  const size_t nf = p.faces.size();
  // ---- topology numbering
  // loops: global index; edges renumbered by first use (face order, loop order, coedge order)
  std::vector<int> edge_new(p.edges.size(), -1);
  std::vector<int> edge_order;              // new index -> caller index
  std::vector<std::vector<int>> loop_ids(nf);
  std::vector<std::vector<std::vector<int>>> new_edges_of_loop(nf);
  int nloops = 0;
  for (size_t f = 0; f < nf; ++f) {
    const Face& face = p.faces[f];
    if (face.loops.empty()) fail("face without loops");
    if (face.material >= std::max<size_t>(1, p.materials.size())) fail("face material index out of range");
    for (const Loop& l : face.loops) {
      loop_ids[f].push_back(nloops++);
      new_edges_of_loop[f].emplace_back();
      for (const Coedge& c : l.coedges) {
        if (c.edge < 0 || static_cast<size_t>(c.edge) >= p.edges.size()) fail("coedge edge index out of range");
        if (edge_new[c.edge] < 0) {
          edge_new[c.edge] = static_cast<int>(edge_order.size());
          edge_order.push_back(c.edge);
          new_edges_of_loop[f].back().push_back(c.edge);
        }
      }
    }
  }
  if (p.write_brep && edge_order.size() != p.edges.size()) fail("edge not used by any coedge");

  // ---- ID table: body, then per face: face id, then the ids of its new edges
  IdTable ids;
  int body_ref = ids.ref(p.body_det_id);
  std::vector<int> face_ref(nf), edge_ref(p.edges.size(), -1);
  for (size_t f = 0; f < nf; ++f) {
    face_ref[f] = ids.ref(p.faces[f].det_id);
    if (p.write_brep)
      for (auto& le : new_edges_of_loop[f])
        for (int e : le) edge_ref[e] = ids.ref(p.edges[e].det_id);
  }

  std::array<std::string, 3> sec;
  // ---- section 0: header
  {
    Rec r;
    { Buf b; b.u32(0); r.open(0x02, b); }
    std::vector<Material> mats = p.materials;
    if (mats.empty()) mats.emplace_back();
    for (size_t i = 0; i < mats.size(); ++i) {
      const Material& m = mats[i];
      Buf b;
      for (const auto* a : {&m.ambient, &m.diffuse, &m.specular, &m.emissive})
        for (float x : *a) b.f32(x);
      b.f32(m.shininess);
      b.f32(write_opacity ? m.opacity : 1.0f);
      r.leaf(0x05, b);
      Buf a;
      put_olstr(a, appearance_name(m));
      a.u32(static_cast<uint32_t>(i)); a.i32(-1); a.u8(1); a.i32(-1); a.i32(-1); a.i32(-1);
      r.leaf(0x07, a);
    }
    if (!ids.values.empty()) {
      Buf b;
      put_olstr(b, p.id_table_name);
      b.u32(static_cast<uint32_t>(ids.values.size()));
      for (const auto& v : ids.values) put_olstr(b, v);
      r.leaf(0x03, b);
    }
    { Buf b; b.u32(0); b.u8(p.has_area ? 1 : 0); b.f32(p.area_m2); b.u8(p.has_volume ? 1 : 0); b.f32(p.volume_m3); b.u32(0);
      r.leaf(0x09, b); }
    {
      double chord = p.chord_tolerance;
      if (chord < 0) {
        double d2 = 0;
        for (int k = 0; k < 3; ++k) {
          double d = static_cast<double>(static_cast<float>(p.bbox_max[k])) - static_cast<double>(static_cast<float>(p.bbox_min[k]));
          d2 += d * d;
        }
        chord = 0.0005 * std::sqrt(d2);
      }
      Buf b; b.u32(14); b.u32(0); b.u8(0); b.f64(p.angle_tolerance); b.f64(chord);
      r.leaf(0xcd, b);
    }
    {
      Buf b; b.u8(1);
      for (double x : p.bbox_min) b.f32(x);
      for (double x : p.bbox_max) b.f32(x);
      b.u8(0); b.u8(1); for (int k = 0; k < 7; ++k) b.u8(0);
      r.open(0x08, b);
      put_idref(r, body_ref);
      r.close();
    }
    r.close();
    sec[0] = r.out.b;
  }
  // ---- section 1: tessellation
  {
    Rec r;
    { Buf b; b.u32(1); r.open(0x02, b); }
    r.open(0x0b, Buf{});
    { Buf b; b.u32(0); r.open(0x0c, b); }
    for (size_t f = 0; f < nf; ++f) {
      const Face& face = p.faces[f];
      const Mesh& m = face.mesh;
      if (m.positions.size() % 3 || m.normals.size() != m.positions.size()) fail("mesh positions/normals size");
      const uint32_t nv = static_cast<uint32_t>(m.positions.size() / 3);
      Buf h;
      h.u32(face.material); h.u32(static_cast<uint32_t>(f)); h.u8(face.same_sense ? 1 : 0);
      h.u32(static_cast<uint32_t>(loop_ids[f].size()));
      for (int l : loop_ids[f]) h.u32(static_cast<uint32_t>(l));
      r.open(0x0d, h);
      put_idref(r, face_ref[f]);
      Buf v; v.u8(1); v.u32(nv);
      for (uint32_t i = 0; i < nv; ++i) {
        for (int k = 0; k < 3; ++k) v.f32(m.positions[3 * i + k]);
        for (int k = 0; k < 3; ++k) v.f32(m.normals[3 * i + k]);
      }
      r.open(0x16, v);
      { Buf b; b.u32(0); b.f32(-1.0); b.u16(0); r.open(0x17, b); }
      { Buf b; b.u32(0); b.u32(face.material); b.u16(0); b.u8(1); r.open(0x18, b); }
      auto prim = [&](uint32_t kind, const std::vector<uint32_t>& idx) {
        for (uint32_t i : idx) if (i >= nv) fail("mesh index out of range");
        Buf b; for (uint32_t i : idx) b.u32(i);
        r.leaf(kind, b);
      };
      if (!m.primitives.empty()) {
        for (const auto& pr : m.primitives) {
          if (pr.kind != 0x19 && pr.kind != 0x1a && pr.kind != 0x1b) fail("primitive kind");
          prim(pr.kind, pr.indices);
        }
      } else {
        if (m.triangles.size() % 3) fail("triangle index count not a multiple of 3");
        prim(0x1b, m.triangles);
      }
      r.close(); r.close(); r.close();   // 0x18, 0x17, 0x16
      r.close();                         // 0x0d
    }
    r.close(); r.close(); r.close();     // 0x0c, 0x0b, 0x02
    sec[1] = r.out.b;
  }
  // ---- section 3: B-rep
  {
    Rec r;
    { Buf b; b.u32(3); r.open(0x02, b); }
    int coedge_no = 0;
    struct Use { const Coedge* c; int face; };
    std::vector<Use> uses;
    if (p.write_brep) {
      for (size_t f = 0; f < nf; ++f) {
        const Face& face = p.faces[f];
        r.leaf(0x13, surface_payload(face.surface));
        const bool nurbs_only = face.surface.kind == SurfaceKind::Nurbs;
        for (size_t li = 0; li < face.loops.size(); ++li) {
          for (int e : new_edges_of_loop[f][li]) {
            const Edge& ed = p.edges[e];
            Buf b; b.u32(static_cast<uint32_t>(edge_new[e])); b.f32(ed.t0); b.f32(ed.t1);
            r.open(0x12, b);
            put_idref(r, edge_ref[e]);
            r.close();
          }
          Buf lb; lb.i32(li == 0 ? -2 : loop_ids[f][0]);
          r.open(0x10, lb);
          for (const Coedge& c : face.loops[li].coedges) {
            Buf b; b.u32(static_cast<uint32_t>(edge_new[c.edge])); b.u8(c.forward ? 1 : 0);
            b.i32(nurbs_only ? coedge_no : -1); b.i32(nurbs_only ? -1 : coedge_no);
            r.leaf(0x11, b);
            uses.push_back({&c, static_cast<int>(f)});
            ++coedge_no;
          }
          r.close();
        }
      }
      // curves and pcurves: per coedge in order; the edge curve right before its first pcurve
      std::vector<bool> done(p.edges.size(), false);
      for (const Use& u : uses) {
        const Coedge& c = *u.c;
        if (!done[c.edge]) {
          done[c.edge] = true;
          const Edge& ed = p.edges[c.edge];
          Buf b;
          if (ed.is_point) {
            b.u8(3); b.u8(0); for (double x : ed.point) b.f32(x); b.u8(0);
          } else {
            b.u8(6); b.u8(0); b.u8(ed.curve.weights.empty() ? 0 : 1);
            put_nurbs_curve_body(b, ed.curve, 3, "edge curve");
            b.u8(0);
          }
          r.leaf(0x14, b);
        }
        Buf b;
        const uint8_t fl = c.pcurve_reversed ? 4 : 0;
        if (c.pcurve.is_line) {
          b.u8(7); b.u8(fl); b.u32(2);
          b.f32(c.pcurve.start[0]); b.f32(c.pcurve.start[1]); b.f32(c.pcurve.end[0]); b.f32(c.pcurve.end[1]);
          b.u8(0);
        } else {
          b.u8(6); b.u8(fl); b.u8(c.pcurve.nurbs.weights.empty() ? 0 : 1);
          put_nurbs_curve_body(b, c.pcurve.nurbs, 2, "pcurve");
          b.u8(0);
        }
        r.leaf(0x15, b);
      }
    }
    r.close();
    sec[2] = r.out.b;
  }
  return sec;
}

std::string build_ol(const Part& p, bool write_opacity)
{
  auto secs = build_ol_sections(p, write_opacity);
  const uint32_t ids[3] = {0, 1, 3};
  std::vector<std::string> gz;
  for (int i = 0; i < 3; ++i) {
    if (i == 2 && !p.write_brep) break;
    gz.push_back(gzip_stored(secs[i]));
  }
  const uint32_t n = static_cast<uint32_t>(gz.size()) + 1;
  Buf o;
  o.raw("DIV-OL09BL");
  o.u32(1); o.u32(4 + 8 * n); o.u32(n);
  uint32_t off = 10 + 8 + (4 + 8 * n) + 19;
  for (size_t i = 0; i < gz.size(); ++i) {
    o.u32(ids[i] | kContainer); o.u32(off);
    off += static_cast<uint32_t>(gz[i].size());
  }
  o.u32(0x00ffffffu); o.u32(off);
  o.u32(0x010000d4u); o.u32(3); o.u8(3); o.u8(9); o.u8(3); o.u32(0); o.u32(0);
  for (const auto& g : gz) o.raw(g);
  return o.b;
}

// ---------------------------------------------------------------------------------
// PVS
// ---------------------------------------------------------------------------------
struct FlatNode {
  const Node* n;
  std::vector<std::pair<int, const Transform*>> children;   // (flat index, transform)
};

int flatten(const Node& n, std::vector<FlatNode>& out, size_t nparts)
{
  if (n.part >= 0 && !n.children.empty()) fail("node '" + n.name + "' has both a part and children");
  if (n.part >= 0 && static_cast<size_t>(n.part) >= nparts) fail("node part index out of range");
  if (n.part < 0 && n.children.empty()) fail("node '" + n.name + "' has neither a part nor children");
  FlatNode f{&n, {}};
  for (const Node& c : n.children) f.children.push_back({flatten(c, out, nparts), &c.transform});
  out.push_back(f);
  return static_cast<int>(out.size() - 1);
}

struct StrTable {
  std::vector<std::string> list;
  std::map<std::string, int> idx;
  void put(Buf& b, const std::string& s) const {
    auto it = idx.find(s);
    if (it != idx.end()) { b.varint(static_cast<uint64_t>(it->second) * 2 + 1); return; }
    b.varint(static_cast<uint64_t>(s.size()) * 2);
    b.raw(s);
  }
};

void put_node(Buf& o, uint8_t tag, uint8_t kind, const Buf& payload, int extra = -1)
{
  o.u8(tag); o.u8(kind);
  o.varint(payload.b.size());
  if (extra >= 0) o.u8(static_cast<uint32_t>(extra));
  o.raw(payload.b);
}

std::string build_pvs(const Model& m, const std::vector<std::string>& ol_names)
{
  std::vector<FlatNode> flat;
  flatten(m.root, flat, m.parts.size());
  // string table: node names and property keys/values used at least twice, first-use order
  std::vector<std::string> seq;
  for (const auto& f : flat) seq.push_back(f.n->name);
  for (const auto& f : flat)
    for (const auto& pr : f.n->properties) { seq.push_back(pr.key); seq.push_back(pr.value); }
  std::map<std::string, int> count;
  for (const auto& s : seq) ++count[s];
  StrTable st;
  for (const auto& s : seq)
    if (count[s] >= 2 && !st.idx.count(s)) { st.idx[s] = static_cast<int>(st.list.size()); st.list.push_back(s); }

  std::vector<std::pair<uint8_t, std::string>> blocks;
  if (!st.list.empty()) {
    Buf body; body.varint(st.list.size());
    for (const auto& s : st.list) { body.varint(s.size()); body.raw(s); }
    Buf o; put_node(o, 0x01, 0x02, body);
    blocks.push_back({1, o.b});
  }
  {
    Buf o; o.u8(0x02); o.u8(0x80);
    std::vector<bool> seen(m.parts.size(), false);
    for (const auto& f : flat) {
      Buf pl; st.put(pl, f.n->name); pl.varint(4);
      put_node(o, 0x20, 0x92, pl, 2);
      if (f.n->part >= 0) {
        const Part& p = m.parts[f.n->part];
        Buf s; const std::string& fn = ol_names[f.n->part];
        s.varint(fn.size() * 2); s.raw(fn);
        if (!seen[f.n->part]) {
          seen[f.n->part] = true;
          for (double x : p.bbox_min) s.f32(x);
          for (double x : p.bbox_max) s.f32(x);
          put_node(o, 0x23, 0x12, s, 1);
        } else {
          put_node(o, 0x23, 0x02, s);
        }
      }
      for (const auto& [ci, t] : f.children) {
        const Transform& x = *t;
        if (x[12] != 0 || x[13] != 0 || x[14] != 0 || x[15] != 1) fail("transform: last row must be 0 0 0 1");
        const bool rot = !(x[0] == 1 && x[1] == 0 && x[2] == 0 && x[4] == 0 && x[5] == 1 && x[6] == 0 &&
                           x[8] == 0 && x[9] == 0 && x[10] == 1);
        const bool tr = x[3] != 0 || x[7] != 0 || x[11] != 0;
        Buf c; c.varint(static_cast<uint64_t>(ci));
        if (!rot && !tr) { put_node(o, 0x21, 0x02, c); continue; }
        c.f64(x[3]); c.f64(x[7]); c.f64(x[11]);
        if (rot) {   // columns: images of the x, y, z axes
          for (int col = 0; col < 3; ++col)
            for (int row = 0; row < 3; ++row) c.f64(x[row * 4 + col]);
        }
        put_node(o, 0x21, 0x12, c, rot ? 6 : 2);
      }
      o.u8(0); o.u8(0);          // end list0, empty list1
    }
    o.u8(0); o.u8(0);
    blocks.push_back({2, o.b});
  }
  {
    Buf o; o.u8(0x03); o.u8(0x80);
    for (const auto& f : flat) {
      o.u8(0x2a); o.u8(0x80);
      for (const auto& pr : f.n->properties) {
        Buf pl; st.put(pl, pr.key); st.put(pl, pr.value);
        put_node(o, 0x2b, 0x02, pl);
      }
      o.u8(0); o.u8(0);
    }
    o.u8(0); o.u8(0);
    blocks.push_back({3, o.b});
  }
  std::vector<std::string> gz;
  for (const auto& b : blocks) gz.push_back(gzip_stored(b.second));
  Buf o;
  o.raw("PV-PVS0305");
  o.u32(static_cast<uint32_t>(18 + 7 * blocks.size()));
  o.u32(static_cast<uint32_t>(blocks.size()));
  for (size_t i = 0; i < blocks.size(); ++i) { o.u8(blocks[i].first); o.u8(1); o.u8(0); o.u32(static_cast<uint32_t>(gz[i].size())); }
  for (const auto& g : gz) o.raw(g);
  return o.b;
}

// ---------------------------------------------------------------------------------
// ZIP: Onshape's streamed Zip64 layout (local header with zero Zip64 sizes, data
// descriptor with 64-bit sizes, Zip64 extra in the central directory, plain EOCD).
// ---------------------------------------------------------------------------------
std::string build_zip(const std::vector<std::pair<std::string, std::string>>& members, bool utf8_flag)
{
  Buf out, cd;
  for (const auto& [name, data] : members) {
    std::string comp = deflate_stored(data);
    uint32_t crc = crc32(data);
    bool nonascii = false;
    for (unsigned char ch : name) if (ch >= 0x80) nonascii = true;
    uint16_t flag = 0x0008 | ((utf8_flag && nonascii) ? 0x0800 : 0);
    uint32_t lho = static_cast<uint32_t>(out.b.size());
    out.u32(0x04034b50); out.u16(45); out.u16(flag); out.u16(8); out.u16(0); out.u16(0);
    out.u32(0); out.u32(0xffffffffu); out.u32(0xffffffffu);
    out.u16(static_cast<uint32_t>(name.size())); out.u16(20);
    out.raw(name);
    out.u16(1); out.u16(16); out.u64(0); out.u64(0);
    out.raw(comp);
    out.u32(0x08074b50); out.u32(crc); out.u64(comp.size()); out.u64(data.size());
    cd.u32(0x02014b50); cd.u16(0x032d); cd.u16(45); cd.u16(flag); cd.u16(8); cd.u16(0); cd.u16(0);
    cd.u32(crc); cd.u32(0xffffffffu); cd.u32(0xffffffffu);
    cd.u16(static_cast<uint32_t>(name.size())); cd.u16(20); cd.u16(0); cd.u16(0); cd.u16(0); cd.u32(0); cd.u32(lho);
    cd.raw(name);
    cd.u16(1); cd.u16(16); cd.u64(data.size()); cd.u64(comp.size());
  }
  uint32_t cdoff = static_cast<uint32_t>(out.b.size());
  out.raw(cd.b);
  out.u32(0x06054b50); out.u16(0); out.u16(0);
  out.u16(static_cast<uint32_t>(members.size())); out.u16(static_cast<uint32_t>(members.size()));
  out.u32(static_cast<uint32_t>(cd.b.size())); out.u32(cdoff); out.u16(0);
  return out.b;
}

} // namespace

// ---------------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------------
static Vec3 scale(const Vec3& v, double s) { return {v[0] * s, v[1] * s, v[2] * s}; }

Surface makePlane(const Vec3& origin, const Vec3& xdir, const Vec3& ydir, Vec2 urange, Vec2 vrange, NurbsSurface nurbs)
{
  Surface s; s.kind = SurfaceKind::Plane; s.origin = origin; s.axis1 = xdir; s.axis2 = ydir;
  s.axis3 = {xdir[1] * ydir[2] - xdir[2] * ydir[1], xdir[2] * ydir[0] - xdir[0] * ydir[2], xdir[0] * ydir[1] - xdir[1] * ydir[0]};
  s.urange = urange; s.vrange = vrange; s.nurbs = std::move(nurbs);
  return s;
}

Surface makeCylinder(const Vec3& origin, const Vec3& xdir, const Vec3& ydir, const Vec3& axis, double radius,
                     Vec2 angle_range, Vec2 height_range, NurbsSurface nurbs)
{
  Surface s; s.kind = SurfaceKind::Cylinder; s.origin = origin; s.axis1 = scale(xdir, radius);
  s.axis2 = scale(ydir, radius); s.axis3 = axis; s.urange = angle_range; s.vrange = height_range;
  s.nurbs = std::move(nurbs);
  return s;
}

Surface makeCone(const Vec3& apex, const Vec3& xdir, const Vec3& ydir, const Vec3& axis, double base_radius,
                 double height, Vec2 generator_range, Vec2 angle_range, NurbsSurface nurbs, bool nurbs_uv_swapped)
{
  Surface s; s.kind = SurfaceKind::Cone; s.origin = apex; s.axis1 = scale(xdir, base_radius);
  s.axis2 = scale(ydir, base_radius); s.axis3 = scale(axis, height); s.urange = generator_range;
  s.vrange = angle_range; s.nurbs_uv_swapped = nurbs_uv_swapped; s.nurbs = std::move(nurbs);
  return s;
}

Surface makeSphere(const Vec3& centre, const Vec3& xdir, const Vec3& ydir, const Vec3& zdir, double radius,
                   Vec2 longitude_range, Vec2 latitude_range, NurbsSurface nurbs)
{
  Surface s; s.kind = SurfaceKind::Sphere; s.origin = centre; s.axis1 = scale(xdir, radius);
  s.axis2 = scale(ydir, radius); s.axis3 = scale(zdir, radius); s.urange = longitude_range;
  s.vrange = latitude_range; s.nurbs = std::move(nurbs);
  return s;
}

std::vector<Property> onshapeRootProperties(const std::string& document_id)
{
  return {{"documentId", document_id},
          {"Adapter_name", "OnshapeCreoViewWriter"},
          {"Adapter_version", "22.1.0.0-RBRGyC"},
          {"Adapter_platform", "x86e_linux64"}};
}

Model makePartStudioModel(std::vector<Part> parts, const std::string& document_id,
                          const std::vector<std::string>& det_ids)
{
  Model m;
  auto detprop = [&](size_t i, std::vector<Property>& props) {
    if (i < det_ids.size() && !det_ids[i].empty()) props.push_back({"BTI/DetId", det_ids[i]});
  };
  if (parts.size() == 1) {
    m.root.name = parts[0].name; m.root.part = 0;
    auto rp = onshapeRootProperties(document_id);
    m.root.properties.push_back(rp[0]);
    detprop(0, m.root.properties);
    m.root.properties.insert(m.root.properties.end(), rp.begin() + 1, rp.end());
  } else {
    m.root.name = "RootAssembly";
    m.root.properties = onshapeRootProperties(document_id);
    for (size_t i = 0; i < parts.size(); ++i) {
      Node n; n.name = parts[i].name; n.part = static_cast<int>(i);
      detprop(i, n.properties);
      m.root.children.push_back(n);
    }
  }
  m.parts = std::move(parts);
  return m;
}

std::string write(const Model& m)
{
  if (m.parts.empty()) fail("no parts");
  std::vector<std::string> ol_names;
  std::set<std::string> used;
  for (size_t i = 0; i < m.parts.size(); ++i) {
    std::string n = m.parts[i].ol_name;
    if (n.empty()) n = m.parts[i].name + "_" + std::to_string(54 + i) + ".ol";
    if (!used.insert(n).second) fail("duplicate .ol name " + n);
    ol_names.push_back(n);
  }
  std::string pvs = build_pvs(m, ol_names);
  std::string pvs_name = m.pvs_name;
  if (pvs_name.empty()) {
    uint32_t c = crc32(pvs);
    char buf[32];
    std::snprintf(buf, sizeof buf, "%08x%016x", c, 0u);
    pvs_name = std::string(buf) + ".pvs";
  }
  std::map<std::string, std::string> files;
  files[pvs_name] = pvs;
  for (size_t i = 0; i < m.parts.size(); ++i) files[ol_names[i]] = build_ol(m.parts[i], m.write_opacity);
  std::vector<std::pair<std::string, std::string>> members;
  if (!m.member_order.empty()) {
    if (m.member_order.size() != files.size()) fail("member_order size");
    for (const auto& n : m.member_order) {
      auto it = files.find(n);
      if (it == files.end()) fail("member_order names unknown member " + n);
      members.push_back(*it);
    }
  } else {
    members.push_back({pvs_name, pvs});
    for (const auto& n : ol_names) members.push_back({n, files[n]});
  }
  return build_zip(members, m.zip_utf8_flag);
}

} // namespace pvz
} // namespace noshape
