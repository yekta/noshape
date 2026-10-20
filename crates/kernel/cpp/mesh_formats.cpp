// 3MF and glTF / GLB files as mesh bodies (as STL and OBJ): every placed
// object's triangles, in millimetres. 3MF: the model XML in its zip (objects,
// components, build items with their transforms, the file's unit). glTF 2.0:
// the default scene's nodes (matrices or TRS), triangle primitives from the
// buffers (GLB chunk, base64 data URIs, or files beside the .gltf). Standard
// library only.

#include "internal.hpp"
#include "pvz_reader.hpp"

#include <cmath>
#include <functional>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>

namespace noshape {

namespace {

std::string read_file(const std::string& path)
{
  std::ifstream in(path, std::ios::binary);
  if (!in)
    fail_io("cannot read " + path);
  return std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
}

// Triangles of every placed object: points in millimetres.
struct Mesh
{
  std::vector<gp_Pnt>             nodes;
  std::vector<std::array<int, 3>> tris;
};

// A 3D affine map: p' = R p + t (row-major 3x4).
struct Affine
{
  double m[12] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
  gp_Pnt apply(double x, double y, double z) const
  {
    return gp_Pnt(m[0] * x + m[1] * y + m[2] * z + m[3], m[4] * x + m[5] * y + m[6] * z + m[7], m[8] * x + m[9] * y + m[10] * z + m[11]);
  }
  Affine then(const Affine& outer) const // outer ∘ this
  {
    Affine r;
    for (int i = 0; i < 3; ++i)
    {
      for (int j = 0; j < 3; ++j)
        r.m[i * 4 + j] = outer.m[i * 4] * m[j] + outer.m[i * 4 + 1] * m[4 + j] + outer.m[i * 4 + 2] * m[8 + j];
      r.m[i * 4 + 3] = outer.m[i * 4] * m[3] + outer.m[i * 4 + 1] * m[7] + outer.m[i * 4 + 2] * m[11] + outer.m[i * 4 + 3];
    }
    return r;
  }
};

// ---------------------------------------------------------------- 3MF

// The attributes of the XML tag starting at `at` ("<name a="1" …").
std::map<std::string, std::string> attributes(const std::string& x, size_t at, size_t end)
{
  std::map<std::string, std::string> out;
  size_t                             p = x.find_first_of(" \t\r\n/>", at + 1);
  while (p < end)
  {
    while (p < end && (std::isspace((unsigned char)x[p]) || x[p] == '/'))
      ++p;
    const size_t eq = x.find('=', p);
    if (eq == std::string::npos || eq >= end)
      break;
    std::string key = x.substr(p, eq - p);
    while (!key.empty() && std::isspace((unsigned char)key.back()))
      key.pop_back();
    const size_t q0 = x.find_first_of("\"'", eq);
    if (q0 == std::string::npos || q0 >= end)
      break;
    const size_t q1 = x.find(x[q0], q0 + 1);
    if (q1 == std::string::npos || q1 >= end)
      break;
    // Drop a namespace prefix ("p:UUID" stays as written).
    out[key] = x.substr(q0 + 1, q1 - q0 - 1);
    p        = q1 + 1;
  }
  return out;
}

// The tag's local name ("m:vertex" -> "vertex").
std::string tag_name(const std::string& x, size_t at)
{
  const size_t e = x.find_first_of(" \t\r\n/>", at + 1);
  std::string  n = x.substr(at + 1, e - at - 1);
  const size_t c = n.find(':');
  return c == std::string::npos ? n : n.substr(c + 1);
}

Affine transform_3mf(const std::string& s)
{
  Affine a;
  if (s.empty())
    return a;
  double       v[12];
  const char*  p = s.c_str();
  for (double& x : v)
  {
    char* e = nullptr;
    x       = std::strtod(p, &e);
    if (e == p)
      fail_invalid("3mf: bad transform");
    p = e;
  }
  // 3MF writes row vectors (p' = p M, last row the translation).
  a.m[0] = v[0], a.m[1] = v[3], a.m[2] = v[6], a.m[3] = v[9];
  a.m[4] = v[1], a.m[5] = v[4], a.m[6] = v[7], a.m[7] = v[10];
  a.m[8] = v[2], a.m[9] = v[5], a.m[10] = v[8], a.m[11] = v[11];
  return a;
}

Mesh read_3mf(const std::string& bytes)
{
  const auto  members = pvz::zip_members(bytes);
  std::string xml;
  for (const auto& [name, data] : members)
  {
    std::string lower = name;
    for (auto& c : lower)
      c = (char)std::tolower((unsigned char)c);
    if (lower.size() >= 6 && lower.compare(lower.size() - 6, 6, ".model") == 0 && lower.find("3d/") != std::string::npos)
    {
      xml = data;
      break;
    }
  }
  if (xml.empty())
    fail_invalid("3mf: no 3D model in the archive");
  struct Object
  {
    std::vector<gp_XYZ>                           v;
    std::vector<std::array<int, 3>>               t;
    std::vector<std::pair<std::string, Affine>>   components;
  };
  std::map<std::string, Object>               objects;
  std::vector<std::pair<std::string, Affine>> items;
  double                                      mm = 1;
  Object*                                     cur = nullptr;
  for (size_t at = xml.find('<'); at != std::string::npos; at = xml.find('<', at + 1))
  {
    if (xml.compare(at, 2, "</") == 0 || xml.compare(at, 2, "<?") == 0 || xml.compare(at, 4, "<!--") == 0)
      continue;
    const size_t      end = xml.find('>', at);
    if (end == std::string::npos)
      break;
    const std::string n = tag_name(xml, at);
    if (n == "model")
    {
      const std::string u = attributes(xml, at, end)["unit"];
      mm = u == "micron" ? 1e-3 : u == "centimeter" ? 10 : u == "inch" ? 25.4 : u == "foot" ? 304.8 : u == "meter" ? 1000 : 1;
    }
    else if (n == "object")
      cur = &objects[attributes(xml, at, end)["id"]];
    else if (n == "vertex" && cur)
    {
      auto a = attributes(xml, at, end);
      cur->v.emplace_back(std::atof(a["x"].c_str()), std::atof(a["y"].c_str()), std::atof(a["z"].c_str()));
    }
    else if (n == "triangle" && cur)
    {
      auto a = attributes(xml, at, end);
      cur->t.push_back({std::atoi(a["v1"].c_str()), std::atoi(a["v2"].c_str()), std::atoi(a["v3"].c_str())});
    }
    else if (n == "component" && cur)
    {
      auto a = attributes(xml, at, end);
      cur->components.emplace_back(a["objectid"], transform_3mf(a["transform"]));
    }
    else if (n == "item")
    {
      auto a = attributes(xml, at, end);
      items.emplace_back(a["objectid"], transform_3mf(a["transform"]));
    }
    at = end;
  }
  Mesh                                                           out;
  std::function<void(const std::string&, const Affine&, int)> place = [&](const std::string& id, const Affine& tf, int depth) {
    auto it = objects.find(id);
    if (it == objects.end() || depth > 32)
      return;
    const Object& o    = it->second;
    const int     base = (int)out.nodes.size();
    for (const gp_XYZ& p : o.v)
      out.nodes.push_back(tf.apply(p.X() * mm, p.Y() * mm, p.Z() * mm));
    for (const auto& t : o.t)
    {
      if (t[0] < 0 || t[1] < 0 || t[2] < 0 || t[0] >= (int)o.v.size() || t[1] >= (int)o.v.size() || t[2] >= (int)o.v.size())
        fail_invalid("3mf: triangle vertex out of range");
      out.tris.push_back({base + t[0], base + t[1], base + t[2]});
    }
    for (const auto& [child, ctf] : o.components)
    {
      Affine scaled = ctf; // component transforms are in file units
      for (int i : {3, 7, 11})
        scaled.m[i] *= mm;
      place(child, scaled.then(tf), depth + 1);
    }
  };
  for (const auto& [id, tf] : items)
  {
    Affine scaled = tf;
    for (int i : {3, 7, 11})
      scaled.m[i] *= mm;
    place(id, scaled, 0);
  }
  return out;
}

// ---------------------------------------------------------------- glTF

struct Json
{
  enum Kind
  {
    Null,
    Bool,
    Num,
    Str,
    Arr,
    Obj
  } k     = Null;
  double                                 n = 0;
  std::string                            s;
  std::vector<Json>                      a;
  std::vector<std::pair<std::string, Json>> o;

  const Json* get(const std::string& key) const
  {
    for (const auto& [k2, v] : o)
      if (k2 == key)
        return &v;
    return nullptr;
  }
  const Json& at(size_t i) const
  {
    static const Json none;
    return i < a.size() ? a[i] : none;
  }
  double num(const std::string& key, double def) const
  {
    const Json* v = get(key);
    return v && v->k == Num ? v->n : def;
  }
};

class JsonParser
{
public:
  explicit JsonParser(const std::string& t) : s(t) {}
  Json parse()
  {
    Json v = value(0);
    return v;
  }

private:
  const std::string& s;
  size_t             p = 0;
  void               ws()
  {
    while (p < s.size() && std::isspace((unsigned char)s[p]))
      ++p;
  }
  char peek()
  {
    ws();
    if (p >= s.size())
      fail_invalid("gltf: truncated JSON");
    return s[p];
  }
  Json value(int depth)
  {
    if (depth > 200)
      fail_invalid("gltf: JSON nested too deeply");
    Json       v;
    const char c = peek();
    if (c == '{')
    {
      v.k = Json::Obj;
      ++p;
      if (peek() == '}')
      {
        ++p;
        return v;
      }
      for (;;)
      {
        Json key = value(depth + 1);
        if (key.k != Json::Str || peek() != ':')
          fail_invalid("gltf: bad JSON object");
        ++p;
        v.o.emplace_back(key.s, value(depth + 1));
        const char d = peek();
        ++p;
        if (d == '}')
          break;
        if (d != ',')
          fail_invalid("gltf: bad JSON object");
      }
    }
    else if (c == '[')
    {
      v.k = Json::Arr;
      ++p;
      if (peek() == ']')
      {
        ++p;
        return v;
      }
      for (;;)
      {
        v.a.push_back(value(depth + 1));
        const char d = peek();
        ++p;
        if (d == ']')
          break;
        if (d != ',')
          fail_invalid("gltf: bad JSON array");
      }
    }
    else if (c == '"')
    {
      v.k = Json::Str;
      ++p;
      while (p < s.size() && s[p] != '"')
      {
        if (s[p] == '\\' && p + 1 < s.size())
        {
          const char e = s[++p];
          v.s += e == 'n' ? '\n' : e == 't' ? '\t' : e == 'u' ? '?' : e;
          if (e == 'u')
            p += 4;
          ++p;
          continue;
        }
        v.s += s[p++];
      }
      if (p >= s.size())
        fail_invalid("gltf: unterminated JSON string");
      ++p;
    }
    else if (s.compare(p, 4, "true") == 0 || s.compare(p, 5, "false") == 0)
    {
      v.k = Json::Bool;
      v.n = s[p] == 't';
      p += s[p] == 't' ? 4 : 5;
    }
    else if (s.compare(p, 4, "null") == 0)
      p += 4;
    else
    {
      char* e = nullptr;
      v.k     = Json::Num;
      v.n     = std::strtod(s.c_str() + p, &e);
      if (e == s.c_str() + p)
        fail_invalid("gltf: bad JSON value");
      p = (size_t)(e - s.c_str());
    }
    return v;
  }
};

std::string base64(const std::string& in)
{
  std::string out;
  int         val = 0, bits = -8;
  for (unsigned char c : in)
  {
    int d;
    if (c >= 'A' && c <= 'Z')
      d = c - 'A';
    else if (c >= 'a' && c <= 'z')
      d = c - 'a' + 26;
    else if (c >= '0' && c <= '9')
      d = c - '0' + 52;
    else if (c == '+' || c == '-')
      d = 62;
    else if (c == '/' || c == '_')
      d = 63;
    else
      continue;
    val = (val << 6) | d;
    bits += 6;
    if (bits >= 0)
    {
      out += (char)((val >> bits) & 0xff);
      bits -= 8;
    }
  }
  return out;
}

Mesh read_gltf(const std::string& bytes, const std::string& path)
{
  std::string json, bin;
  if (bytes.compare(0, 4, "glTF") == 0)
  {
    // GLB: header, then chunks (JSON, then BIN).
    auto u32 = [&](size_t at) {
      if (at + 4 > bytes.size())
        fail_invalid("gltf: truncated GLB");
      uint32_t v;
      std::memcpy(&v, bytes.data() + at, 4);
      return (size_t)v;
    };
    for (size_t at = 12; at + 8 <= bytes.size();)
    {
      const size_t len = u32(at), type = u32(at + 4);
      if (at + 8 + len > bytes.size())
        fail_invalid("gltf: truncated GLB chunk");
      if (type == 0x4E4F534A)
        json = bytes.substr(at + 8, len);
      else if (type == 0x004E4942)
        bin = bytes.substr(at + 8, len);
      at += 8 + len;
    }
  }
  else
    json = bytes;
  const Json doc = JsonParser(json).parse();
  // Buffers.
  std::vector<std::string> buffers;
  if (const Json* bs = doc.get("buffers"))
    for (const Json& b : bs->a)
    {
      const Json* uri = b.get("uri");
      if (!uri)
        buffers.push_back(bin);
      else if (uri->s.rfind("data:", 0) == 0)
        buffers.push_back(base64(uri->s.substr(uri->s.find(',') + 1)));
      else
      {
        // A file beside the .gltf (its name only: no paths out of the folder).
        std::string name = uri->s;
        const size_t slash = name.find_last_of("/\\");
        if (slash != std::string::npos)
          name = name.substr(slash + 1);
        const size_t dir = path.find_last_of("/\\");
        buffers.push_back(read_file((dir == std::string::npos ? std::string() : path.substr(0, dir + 1)) + name));
      }
    }
  const Json* views     = doc.get("bufferViews");
  const Json* accessors = doc.get("accessors");
  const Json* meshes    = doc.get("meshes");
  const Json* nodes     = doc.get("nodes");
  if (!views || !accessors || !meshes)
    fail_invalid("gltf: no meshes");
  // An accessor's values as doubles (components per element in `comps`).
  auto read_accessor = [&](size_t idx, int& comps) {
    const Json&  acc   = accessors->at(idx);
    const size_t count = (size_t)acc.num("count", 0);
    const Json*  type  = acc.get("type");
    comps              = !type ? 1 : type->s == "VEC3" ? 3 : type->s == "VEC2" ? 2 : type->s == "VEC4" ? 4 : 1;
    const int    ct    = (int)acc.num("componentType", 5126);
    const size_t csize = ct == 5126 || ct == 5125 ? 4 : ct == 5123 || ct == 5122 ? 2 : 1;
    std::vector<double> out;
    if (!acc.get("bufferView"))
      return std::vector<double>(count * (size_t)comps, 0.0);
    const Json&  view   = views->at((size_t)acc.num("bufferView", 0));
    const size_t bi     = (size_t)view.num("buffer", 0);
    if (bi >= buffers.size())
      fail_invalid("gltf: missing buffer");
    const std::string& buf    = buffers[bi];
    const size_t       stride = (size_t)view.num("byteStride", 0) ? (size_t)view.num("byteStride", 0) : csize * (size_t)comps;
    const size_t       base   = (size_t)view.num("byteOffset", 0) + (size_t)acc.num("byteOffset", 0);
    if (count && base + (count - 1) * stride + csize * (size_t)comps > buf.size())
      fail_invalid("gltf: accessor outside its buffer");
    for (size_t i = 0; i < count; ++i)
      for (int c = 0; c < comps; ++c)
      {
        const char* q = buf.data() + base + i * stride + (size_t)c * csize;
        double      v = 0;
        if (ct == 5126)
        {
          float f;
          std::memcpy(&f, q, 4);
          v = f;
        }
        else if (ct == 5125)
        {
          uint32_t u;
          std::memcpy(&u, q, 4);
          v = u;
        }
        else if (ct == 5123)
        {
          uint16_t u;
          std::memcpy(&u, q, 2);
          v = u;
        }
        else
          v = (unsigned char)*q;
        out.push_back(v);
      }
    return out;
  };
  Mesh out;
  auto add_mesh = [&](size_t mi, const Affine& tf) {
    const Json* prims = meshes->at(mi).get("primitives");
    if (!prims)
      return;
    for (const Json& pr : prims->a)
    {
      if ((int)pr.num("mode", 4) != 4)
        continue; // triangles only
      const Json* attrs = pr.get("attributes");
      const Json* pos   = attrs ? attrs->get("POSITION") : nullptr;
      if (!pos)
        continue;
      int                       pc;
      const std::vector<double> p    = read_accessor((size_t)pos->n, pc);
      const int                 base = (int)out.nodes.size();
      if (pc != 3)
        fail_invalid("gltf: positions are not 3D");
      for (size_t i = 0; i + 2 < p.size(); i += 3)
        out.nodes.push_back(tf.apply(p[i] * 1000, p[i + 1] * 1000, p[i + 2] * 1000)); // metres -> mm
      const int n = (int)(p.size() / 3);
      if (const Json* ix = pr.get("indices"))
      {
        int                       ic;
        const std::vector<double> idx = read_accessor((size_t)ix->n, ic);
        for (size_t i = 0; i + 2 < idx.size(); i += 3)
        {
          const int a = (int)idx[i], b = (int)idx[i + 1], c = (int)idx[i + 2];
          if (a >= n || b >= n || c >= n)
            fail_invalid("gltf: index out of range");
          out.tris.push_back({base + a, base + b, base + c});
        }
      }
      else
        for (int i = 0; i + 2 < n; i += 3)
          out.tris.push_back({base + i, base + i + 1, base + i + 2});
    }
  };
  // A node's local transform (mm translations).
  auto local = [](const Json& nd) {
    Affine a;
    if (const Json* m = nd.get("matrix"); m && m->a.size() == 16)
    {
      // Column-major 4x4.
      for (int r = 0; r < 3; ++r)
        for (int c = 0; c < 4; ++c)
          a.m[r * 4 + c] = m->a[(size_t)(c * 4 + r)].n * (c == 3 ? 1000 : 1);
      return a;
    }
    double t[3] = {0, 0, 0}, q[4] = {0, 0, 0, 1}, s[3] = {1, 1, 1};
    if (const Json* v = nd.get("translation"))
      for (int i = 0; i < 3; ++i)
        t[i] = v->at((size_t)i).n * 1000;
    if (const Json* v = nd.get("rotation"))
      for (int i = 0; i < 4; ++i)
        q[i] = v->at((size_t)i).n;
    if (const Json* v = nd.get("scale"))
      for (int i = 0; i < 3; ++i)
        s[i] = v->at((size_t)i).n;
    const double x = q[0], y = q[1], z = q[2], w = q[3];
    const double R[9] = {1 - 2 * (y * y + z * z), 2 * (x * y - z * w),     2 * (x * z + y * w),
                         2 * (x * y + z * w),     1 - 2 * (x * x + z * z), 2 * (y * z - x * w),
                         2 * (x * z - y * w),     2 * (y * z + x * w),     1 - 2 * (x * x + y * y)};
    for (int r = 0; r < 3; ++r)
    {
      for (int c = 0; c < 3; ++c)
        a.m[r * 4 + c] = R[r * 3 + c] * s[c];
      a.m[r * 4 + 3] = t[r];
    }
    return a;
  };
  std::function<void(size_t, const Affine&, int)> visit = [&](size_t ni, const Affine& parent, int depth) {
    if (!nodes || ni >= nodes->a.size() || depth > 64)
      return;
    const Json&  nd = nodes->a[ni];
    const Affine tf = local(nd).then(parent);
    if (const Json* m = nd.get("mesh"))
      add_mesh((size_t)m->n, tf);
    if (const Json* ch = nd.get("children"))
      for (const Json& c : ch->a)
        visit((size_t)c.n, tf, depth + 1);
  };
  const Json* scenes = doc.get("scenes");
  if (nodes && scenes && !scenes->a.empty())
  {
    const Json& sc = scenes->at((size_t)doc.num("scene", 0));
    if (const Json* roots = sc.get("nodes"))
      for (const Json& r : roots->a)
        visit((size_t)r.n, Affine(), 0);
  }
  else
    for (size_t i = 0; i < meshes->a.size(); ++i)
      add_mesh(i, Affine());
  return out;
}

} // namespace

// 3MF / glTF / GLB as mesh bodies, in millimetres (empty for other files).
bool read_mesh_format(const std::string& path, std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>>& tris)
{
  std::string lower = path;
  for (auto& c : lower)
    c = (char)std::tolower((unsigned char)c);
  const auto ends = [&](const char* e) {
    const size_t n = std::strlen(e);
    return lower.size() >= n && lower.compare(lower.size() - n, n, e) == 0;
  };
  Mesh m;
  try
  {
    if (ends(".3mf"))
      m = read_3mf(read_file(path));
    else if (ends(".gltf") || ends(".glb"))
      m = read_gltf(read_file(path), path);
    else
      return false;
  }
  catch (const std::runtime_error& e)
  {
    fail_invalid(e.what());
  }
  if (m.tris.empty())
    fail_invalid("the file has no triangles");
  nodes = std::move(m.nodes);
  tris  = std::move(m.tris);
  return true;
}

} // namespace noshape
