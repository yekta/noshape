// ACIS import: .sat (text) and .sab (binary) files and the ShapeManager
// ("ASM") streams Inventor embeds. Topology, analytic surfaces and curves
// exactly; spline surfaces and intcurves from the B-spline data ACIS stores
// with them (exact for exactsur / exactcur, ACIS's own fit for procedural
// ones); pcurves are not read (the healing projects them). Record layouts per
// version as ACIS writes them (SAT 4.0 through 2x.0 and ASM 2xx).

#include "internal.hpp"

#include <BRepBuilderAPI_Transform.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRepFill_Filling.hxx>
#include <BRepLib.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>

#include <cmath>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <optional>
#include <set>

namespace noshape {

namespace {

// ---------------------------------------------------------------- tokens

struct Tok
{
  enum Kind
  {
    Word, // a keyword (text) or identifier (binary)
    Num,
    Ref,
    Str,
    Bool, // binary TRUE / FALSE
    Open,
    Close
  } k     = Word;
  double      x = 0;
  long long   r = -1;
  bool        b  = false;
  bool        en = false; // binary: an enum value (tag 21), not an integer
  std::string s;
  size_t      begin = 0, end = 0; // text: the token's span in the record
};

struct Record
{
  std::string      type; // "spline-surface"
  std::vector<Tok> toks;
  std::string      text;      // text files: the record's source
  size_t           start = 0; // and where it starts in the file (tokens' spans are file offsets)
  bool             is(const char* base) const
  {
    const size_t n = std::strlen(base);
    return type.size() >= n && type.compare(type.size() - n, n, base) == 0 && (type.size() == n || type[type.size() - n - 1] == '-');
  }
};

struct File
{
  double              version = 0; // 7.0 for "700", 226 for ASM 226
  bool                asm_     = false;
  int                 asm_major = 0;
  double              scale    = 1; // millimetres per file unit (10 in Inventor's cm files)
  std::vector<Record> recs;
};

bool numeric(const std::string& w, double& x)
{
  if (w.empty())
    return false;
  char*  end = nullptr;
  x          = std::strtod(w.c_str(), &end);
  return end && *end == 0;
}

// Text: header (3 lines), then records "[-n] type data #".
File parse_text(const std::string& d)
{
  File   f;
  size_t at = 0;
  auto skip_space = [&]() {
    while (at < d.size() && std::isspace((unsigned char)d[at]))
      ++at;
  };
  auto word = [&]() {
    skip_space();
    const size_t b = at;
    while (at < d.size() && !std::isspace((unsigned char)d[at]))
      ++at;
    return d.substr(b, at - b);
  };
  // Version, records, bodies, flags.
  f.version = std::atof(word().c_str()) / 100.0;
  for (int k = 0; k < 3; ++k)
    word();
  // Product, ACIS version and date as counted strings (not always on their
  // own line), then the scale (millimetres per unit), resabs, resnor.
  if (f.version >= 2.0)
    for (int k = 0; k < 3; ++k)
    {
      std::string n = word();
      if (!n.empty() && n[0] == '@')
        n = n.substr(1);
      double x;
      if (!numeric(n, x))
        fail_invalid("acis: bad header");
      at = std::min(d.size(), at + 1 + (size_t)x);
    }
  f.scale = std::atof(word().c_str());
  if (!(f.scale > 0))
    f.scale = 1;
  word();
  word();
  // Records.
  while (at < d.size())
  {
    // Skip whitespace.
    while (at < d.size() && std::isspace((unsigned char)d[at]))
      ++at;
    if (at >= d.size())
      break;
    if (d.compare(at, 16, "End-of-ACIS-data") == 0 || d.compare(at, 15, "End-of-ASM-data") == 0 ||
        d.compare(at, 28, "Begin-of-ACIS-History-Data") == 0)
      break;
    // The record runs to a '#' outside strings.
    Record      rec;
    long long   seq = -1;
    size_t      p = at;
    std::string text;
    bool        first = true;
    while (p < d.size())
    {
      while (p < d.size() && std::isspace((unsigned char)d[p]))
        ++p;
      if (p >= d.size())
        break;
      if (d[p] == '#')
      {
        ++p;
        break;
      }
      if (d[p] == '@')
      {
        // "@n text": a string of n characters after one space.
        size_t q = p + 1;
        while (q < d.size() && std::isdigit((unsigned char)d[q]))
          ++q;
        const size_t n = (size_t)std::atoll(d.substr(p + 1, q - p - 1).c_str());
        Tok          t;
        t.k     = Tok::Str;
        t.s     = d.substr(std::min(q + 1, d.size()), n);
        t.begin = p;
        t.end   = std::min(d.size(), q + 1 + n);
        rec.toks.push_back(t);
        p = t.end;
        continue;
      }
      size_t q = p;
      while (q < d.size() && !std::isspace((unsigned char)d[q]) && d[q] != '#')
        ++q;
      std::string w = d.substr(p, q - p);
      Tok         t;
      t.begin = p;
      t.end   = q;
      p       = q;
      if (first)
      {
        // An optional "-n" sequence number, then the type.
        double x;
        if (w.size() > 1 && w[0] == '-' && numeric(w, x))
        {
          seq = (long long)-x;
          continue;
        }
        rec.type = w;
        first    = false;
        continue;
      }
      double x;
      if (w == "{")
        t.k = Tok::Open;
      else if (w == "}")
        t.k = Tok::Close;
      else if (w.size() > 1 && w[0] == '$')
      {
        t.k = Tok::Ref;
        t.r = std::atoll(w.c_str() + 1);
      }
      else if (numeric(w, x))
      {
        t.k = Tok::Num;
        t.x = x;
      }
      else
      {
        t.k = Tok::Word;
        t.s = w;
      }
      rec.toks.push_back(t);
    }
    rec.text  = d.substr(at, p - at);
    rec.start = at;
    at        = p;
    // Records go where their sequence number says (when written).
    const size_t slot = seq >= 0 ? (size_t)seq : f.recs.size();
    if (slot > 50000000)
      fail_invalid("acis: bad record number");
    if (f.recs.size() <= slot)
      f.recs.resize(slot + 1);
    f.recs[slot] = std::move(rec);
  }
  return f;
}

// Binary: "ACIS BinaryFile" / "ASM BinaryFile4", 4 ints, tagged header
// values, then tagged records each ending with tag 17.
File parse_binary(const std::string& d, size_t magic)
{
  File   f;
  size_t at = magic;
  auto   need = [&](size_t n) {
    if (at + n > d.size())
      fail_invalid("acis: truncated binary file");
  };
  auto u8  = [&]() { need(1); return (unsigned char)d[at++]; };
  auto i32 = [&]() {
    need(4);
    int32_t v;
    std::memcpy(&v, d.data() + at, 4);
    at += 4;
    return (long long)v;
  };
  auto i64 = [&]() {
    need(8);
    int64_t v;
    std::memcpy(&v, d.data() + at, 8);
    at += 8;
    return (long long)v;
  };
  auto f64 = [&]() {
    need(8);
    double v;
    std::memcpy(&v, d.data() + at, 8);
    at += 8;
    return v;
  };
  auto f32 = [&]() {
    need(4);
    float v;
    std::memcpy(&v, d.data() + at, 4);
    at += 4;
    return (double)v;
  };
  auto str = [&](size_t n) {
    need(n);
    std::string s = d.substr(at, n);
    at += n;
    return s;
  };
  f.version = (double)i32() / 100.0;
  i32(); // records
  i32(); // bodies
  i32(); // flags
  // One tagged value (or a name / end marker).
  auto value = [&](Tok& t, int& tag) {
    tag = u8();
    switch (tag)
    {
      case 2:
        t.k = Tok::Num, t.x = (double)u8();
        break;
      case 3:
      {
        need(2);
        int16_t v;
        std::memcpy(&v, d.data() + at, 2);
        at += 2;
        t.k = Tok::Num, t.x = v;
        break;
      }
      case 4:
      case 21:
        t.k = Tok::Num, t.x = (double)i32(), t.en = tag == 21;
        break;
      case 23:
        t.k = Tok::Num, t.x = (double)i64();
        break;
      case 5:
        t.k = Tok::Num, t.x = f32();
        break;
      case 6:
        t.k = Tok::Num, t.x = f64();
        break;
      case 7:
      case 13:
      case 14:
        t.k = tag == 7 ? Tok::Str : Tok::Word, t.s = str(u8());
        break;
      case 8:
      {
        need(2);
        uint16_t n;
        std::memcpy(&n, d.data() + at, 2);
        at += 2;
        t.k = Tok::Str, t.s = str(n);
        break;
      }
      case 9:
      case 18:
        t.k = Tok::Str, t.s = str((size_t)i32());
        break;
      case 10:
      case 11:
        t.k = Tok::Bool, t.b = tag == 10;
        break;
      case 12:
        t.k = Tok::Ref, t.r = i32();
        break;
      case 15:
        t.k = Tok::Open;
        break;
      case 16:
        t.k = Tok::Close;
        break;
      case 17:
        break;
      case 19:
      case 20:
      case 22:
        t.k = Tok::Num; // expanded by the caller
        break;
      default:
        fail_invalid("acis: bad binary tag " + std::to_string(tag));
    }
  };
  // Header: product strings, then scale, resabs, resnor.
  {
    std::vector<double> nums;
    while (nums.size() < 3)
    {
      Tok t;
      int tag;
      value(t, tag);
      if (t.k == Tok::Num && tag == 6)
        nums.push_back(t.x);
    }
    f.scale = nums[0] > 0 ? nums[0] : 1;
  }
  while (at < d.size())
  {
    Record rec;
    // The type: identifiers until the first value.
    std::vector<std::string> names;
    for (;;)
    {
      need(1);
      const int tag = (unsigned char)d[at];
      if (tag != 13 && tag != 14)
        break;
      ++at;
      names.push_back(str(u8()));
    }
    if (names.empty())
      fail_invalid("acis: binary record without a type");
    for (size_t i = 0; i < names.size(); ++i)
      rec.type += (i ? "-" : "") + names[i];
    if (rec.type.rfind("End-of-", 0) == 0 || rec.type.rfind("Begin-of-ACIS-History", 0) == 0)
      break;
    for (;;)
    {
      Tok t;
      int tag;
      value(t, tag);
      if (tag == 17)
        break;
      if (tag == 19 || tag == 20 || tag == 22)
      {
        for (int k = 0; k < (tag == 22 ? 2 : 3); ++k)
        {
          Tok c;
          c.k = Tok::Num;
          c.x = f64();
          rec.toks.push_back(c);
        }
        continue;
      }
      rec.toks.push_back(t);
    }
    if (rec.type == "asmheader")
    {
      for (const Tok& t : rec.toks)
        if (t.k == Tok::Str)
          f.asm_major = std::atoi(t.s.c_str());
    }
    f.recs.push_back(std::move(rec));
  }
  return f;
}

// ---------------------------------------------------------------- a record's fields

struct Cursor
{
  const File*   file;
  const Record* rec;
  size_t        i = 0;

  bool       more() const { return i < rec->toks.size(); }
  const Tok& peek() const
  {
    if (i >= rec->toks.size())
      fail_invalid("acis: " + rec->type + ": record too short");
    return rec->toks[i];
  }
  const Tok& next()
  {
    const Tok& t = peek();
    ++i;
    return t;
  }
  void      skip(size_t n = 1) { i += n; }
  long long ref()
  {
    const Tok& t = next();
    if (t.k != Tok::Ref)
      fail_invalid("acis: " + rec->type + ": expected a pointer");
    return t.r;
  }
  double num()
  {
    const Tok& t = next();
    if (t.k == Tok::Num)
      return t.x;
    if (t.k == Tok::Bool)
      return t.b ? 1 : 0;
    fail_invalid("acis: " + rec->type + ": expected a number");
  }
  gp_XYZ xyz()
  {
    const double x = num(), y = num(), z = num(); // in order (argument evaluation order is unspecified)
    return gp_XYZ(x, y, z);
  }
  // A keyword: text word, binary identifier, enum value or boolean.
  std::string word()
  {
    const Tok& t = next();
    if (t.k == Tok::Word || t.k == Tok::Str)
      return t.s;
    if (t.k == Tok::Bool)
      return t.b ? "T" : "F";
    if (t.k == Tok::Num)
      return std::to_string((long long)t.x);
    fail_invalid("acis: " + rec->type + ": expected a keyword");
  }
  // forward / reversed (binary: FALSE / TRUE, or an enum 0 / 1).
  bool reversed()
  {
    const Tok& t = next();
    if (t.k == Tok::Bool)
      return t.b;
    if (t.k == Tok::Num)
      return t.x != 0;
    return t.s == "reversed" || t.s == "reverse_v" || t.s == "reversed_v";
  }
  // A range bound: I (infinite) or F value (binary: FALSE / TRUE value).
  void range()
  {
    const Tok& t = next();
    const bool finite = (t.k == Tok::Word && (t.s == "F" || t.s == "T")) || (t.k == Tok::Bool && t.b);
    if (finite)
      skip(t.k == Tok::Word && t.s == "T" ? 7 : 1);
  }
  // ENTITY data.
  void entity()
  {
    ref(); // attributes
    if (file->version > 6.0)
      skip(); // id
  }
  // TOPOLOGY / GEOMETRY data.
  void topology()
  {
    entity();
    if (file->version > 10.0 && !file->asm_)
      skip();
    if (file->version > 6.0)
      skip();
  }
};

// ---------------------------------------------------------------- builder

class Builder
{
public:
  explicit Builder(const File& f) : file(f), mm(f.scale * 1e-3) {}

  std::vector<XtPart> parts()
  {
    std::vector<XtPart> out;
    for (size_t i = 0; i < file.recs.size(); ++i)
      if (file.recs[i].type == "body")
      {
        XtPart p;
        p.shape = body((long long)i);
        if (p.shape.IsNull())
          continue;
        attributes((long long)i, p);
        out.push_back(p);
      }
    if (out.empty())
      fail_unsupported("acis: the file has no bodies");
    return out;
  }

private:
  const File&  file;
  double       mm; // metres per file unit
  BRep_Builder B;
  const double tol = 1e-7;
  std::map<long long, occ::handle<Geom_Curve>> curves;
  std::map<long long, TopoDS_Vertex>           verts;
  std::map<long long, TopoDS_Edge>             edges;

  const Record& at(long long i) const
  {
    if (i < 0 || (size_t)i >= file.recs.size())
      fail_invalid("acis: dangling pointer " + std::to_string(i));
    return file.recs[(size_t)i];
  }
  Cursor cursor(long long i) const { return Cursor{&file, &at(i)}; }
  gp_Pnt P(const gp_XYZ& x) const { return gp_Pnt(x * mm); }

  // ---- geometry

  // A B-spline in ACIS form: "nubs|nurbs deg [rational] form [form]
  // [singularity…] n knots… poles…" (knots as value/multiplicity pairs with
  // end multiplicities one short of OCCT's).
  struct Knots
  {
    std::vector<double> k;
    std::vector<int>    m;
  };
  static Knots knots(Cursor& c, int n, int deg)
  {
    Knots out;
    for (int j = 0; j < n; ++j)
    {
      out.k.push_back(c.num());
      out.m.push_back((int)c.num());
    }
    if (!out.m.empty())
    {
      out.m.front() += 1;
      out.m.back() += 1;
      for (int& m : out.m)
        m = std::min(m, deg + 1);
    }
    return out;
  }
  static int pole_count(const Knots& k, int deg)
  {
    int s = 0;
    for (int m : k.m)
      s += m;
    return s - deg - 1;
  }
  static bool form_word(const std::string& w) { return w == "open" || w == "closed" || w == "periodic" || w == "0" || w == "1" || w == "2"; }

  occ::handle<Geom_BSplineCurve> bs3_curve(Cursor& c)
  {
    const std::string kind = c.word(); // nubs / nurbs
    const bool        rat  = kind == "nurbs";
    const int         deg  = (int)c.num();
    if (form_word(c.peek().k == Tok::Word || c.peek().k == Tok::Num ? (c.peek().k == Tok::Word ? c.peek().s : std::to_string((long long)c.peek().x)) : ""))
      c.skip(); // open / closed / periodic
    const int   n  = (int)c.num();
    const Knots kn = knots(c, n, deg);
    const int   np = pole_count(kn, deg);
    if (np < 2)
      fail_invalid("acis: bad B-spline curve");
    NCollection_Array1<gp_Pnt> poles(1, np);
    NCollection_Array1<double> w(1, np);
    for (int i = 1; i <= np; ++i)
    {
      poles(i) = P(c.xyz());
      w(i)     = rat ? c.num() : 1.0;
    }
    NCollection_Array1<double> K(1, (int)kn.k.size());
    NCollection_Array1<int>    M(1, (int)kn.m.size());
    for (size_t j = 0; j < kn.k.size(); ++j)
    {
      K((int)j + 1) = kn.k[j];
      M((int)j + 1) = kn.m[j];
    }
    return rat ? new Geom_BSplineCurve(poles, w, K, M, deg) : new Geom_BSplineCurve(poles, K, M, deg);
  }

  occ::handle<Geom_BSplineSurface> bs3_surface(Cursor& c)
  {
    const std::string kind = c.word();
    const bool        rat  = kind == "nurbs";
    const int         du = (int)c.num(), dv = (int)c.num();
    if (rat)
      c.skip(); // rational in: both / u / v
    c.skip(2); // forms
    if (file.version >= 4.0)
      c.skip(2); // singularities
    const int   nu = (int)c.num(), nv = (int)c.num();
    const Knots ku = knots(c, nu, du), kv = knots(c, nv, dv);
    const int   npu = pole_count(ku, du), npv = pole_count(kv, dv);
    if (npu < 2 || npv < 2)
      fail_invalid("acis: bad B-spline surface (degrees " + std::to_string(du) + " " + std::to_string(dv) + ", " + std::to_string(nu) + " x " +
                   std::to_string(nv) + " knots)");
    NCollection_Array2<gp_Pnt> poles(1, npu, 1, npv);
    NCollection_Array2<double> w(1, npu, 1, npv);
    for (int j = 1; j <= npv; ++j) // u varies fastest
      for (int i = 1; i <= npu; ++i)
      {
        poles(i, j) = P(c.xyz());
        w(i, j)     = rat ? c.num() : 1.0;
      }
    auto arr = [](const std::vector<double>& v) {
      NCollection_Array1<double> a(1, (int)v.size());
      for (size_t i = 0; i < v.size(); ++i)
        a((int)i + 1) = v[i];
      return a;
    };
    auto arri = [](const std::vector<int>& v) {
      NCollection_Array1<int> a(1, (int)v.size());
      for (size_t i = 0; i < v.size(); ++i)
        a((int)i + 1) = v[i];
      return a;
    };
    return rat ? new Geom_BSplineSurface(poles, w, arr(ku.k), arr(kv.k), arri(ku.m), arri(kv.m), du, dv)
               : new Geom_BSplineSurface(poles, arr(ku.k), arr(kv.k), arri(ku.m), arri(kv.m), du, dv);
  }

  // Subtype blocks ("{ exactsur … }"), numbered in file order for "{ ref n }".
  std::vector<std::pair<long long, size_t>> subtypes; // record, token index of "{"
  bool                                      subtypes_indexed = false;
  // The cursor at the subtype body (after "{ name"), following refs.
  Cursor subtype(Cursor c, std::string& name)
  {
    if (!subtypes_indexed)
    {
      for (size_t r = 0; r < file.recs.size(); ++r)
        for (size_t t = 0; t < file.recs[r].toks.size(); ++t)
          if (file.recs[r].toks[t].k == Tok::Open)
          {
            const auto& n = file.recs[r].toks;
            if (!(t + 1 < n.size() && n[t + 1].k == Tok::Word && n[t + 1].s == "ref"))
              subtypes.emplace_back((long long)r, t);
          }
      subtypes_indexed = true;
    }
    for (int guard = 0; guard < 64; ++guard)
    {
      if (c.next().k != Tok::Open)
        fail_invalid("acis: " + c.rec->type + ": expected a subtype");
      name = c.word();
      if (name != "ref")
        return c;
      const long long k = (long long)c.num();
      if (k < 0 || (size_t)k >= subtypes.size())
        fail_invalid("acis: bad subtype reference");
      Cursor d = cursor(subtypes[(size_t)k].first);
      d.i      = subtypes[(size_t)k].second;
      c        = d;
    }
    fail_invalid("acis: subtype reference loop");
  }

  // The B-spline data stored with a spline surface / intcurve subtype, at
  // the subtype's own level: an intcurve's 3D curve comes first (its 2D
  // curves after it), a procedural surface's approximation last (after its
  // supports, which nest). A surface's data is "nubs|nurbs du dv …", a
  // curve's "nubs|nurbs d form …".
  std::optional<Cursor> stored_bs(Cursor c, bool want_surface) const
  {
    std::optional<Cursor> found;
    int                   depth = 0;
    while (c.more())
    {
      const Tok& t = c.peek();
      if (t.k == Tok::Open)
        ++depth;
      else if (t.k == Tok::Close)
      {
        if (depth == 0)
          break;
        --depth;
      }
      else if (depth == 0 && t.k == Tok::Word && (t.s == "nubs" || t.s == "nurbs") && c.i + 2 < c.rec->toks.size())
      {
        const Tok& a = c.rec->toks[c.i + 1];
        const Tok& b = c.rec->toks[c.i + 2];
        const bool surface = a.k == Tok::Num && b.k == Tok::Num && !b.en;
        if (surface == want_surface)
        {
          found = c;
          if (!want_surface)
            break;
        }
      }
      c.skip();
    }
    return found;
  }

  // A surface and whether its ACIS normal (before the face's sense) agrees
  // with OCCT's.
  occ::handle<Geom_Surface> surface(long long idx, bool& agrees)
  {
    Cursor c = cursor(idx);
    c.topology();
    const std::string& type = c.rec->type;
    return surface_data(c, type.substr(0, type.find('-')) + "-surface", agrees);
  }

  // A surface's data from the cursor ("plane-surface" …; also surfaces
  // written inline in subtypes: "spline", "plane" …).
  occ::handle<Geom_Surface> surface_data(Cursor& c, const std::string& t, bool& agrees)
  {
    agrees = true;
    if (t == "plane-surface")
    {
      const gp_XYZ o = c.xyz(), n = c.xyz(), ux = c.xyz();
      gp_Dir       x = gp_Dir(ux.Modulus() > 0 ? ux : gp_XYZ(1, 0, 0));
      const gp_Dir z(n);
      if (std::abs(x.Dot(z)) > 1e-9)
        x = z.IsParallel(gp::DX(), 1e-6) ? gp_Dir(z.Crossed(gp::DY())) : gp_Dir(z.Crossed(gp::DX()));
      return new Geom_Plane(gp_Ax3(P(o), z, x));
    }
    if (t == "cone-surface")
    {
      const gp_XYZ o = c.xyz(), ax = c.xyz(), major = c.xyz();
      const double ratio = c.num();
      c.range();
      c.range();
      const double sine = c.num(), cosine = c.num();
      if (file.version >= 4.0)
        c.num(); // u scale
      const bool rev = c.reversed();
      const double r = major.Modulus() * mm;
      gp_Dir       x(major);
      gp_Dir       a(ax);
      // The radius along the axis is r + t sine / cosine; the normal points
      // away from the axis when cosine > 0.
      agrees = (cosine >= 0) != rev;
      if (std::abs(sine) < 1e-12)
      {
        if (std::abs(ratio - 1) > 1e-9)
        {
          // An elliptical cylinder.
          occ::handle<Geom_Ellipse> e = new Geom_Ellipse(gp_Ax2(P(o), a, x), r, r * ratio);
          occ::handle<Geom_Surface> s = new Geom_SurfaceOfLinearExtrusion(e, a);
          // Its normal (u along the ellipse, v along a) points outward.
          return s;
        }
        return new Geom_CylindricalSurface(gp_Ax3(P(o), a, x), r);
      }
      if (std::abs(ratio - 1) > 1e-9)
        fail_unsupported("acis: elliptical cones");
      const double tn = sine / cosine;
      const gp_Dir w  = tn > 0 ? a : a.Reversed();
      return new Geom_ConicalSurface(gp_Ax3(P(o), w, x), std::atan(std::abs(tn)), r);
    }
    if (t == "sphere-surface")
    {
      const gp_XYZ o = c.xyz();
      const double r = c.num();
      const gp_XYZ ux = c.xyz(), pole = c.xyz();
      agrees          = r > 0;
      gp_Dir z(pole), x(ux);
      if (std::abs(x.Dot(z)) > 1e-9)
        x = gp_Dir(z.XYZ().Crossed(gp::DX().XYZ()).Modulus() > 1e-6 ? z.XYZ().Crossed(gp::DX().XYZ()) : z.XYZ().Crossed(gp::DY().XYZ()));
      return new Geom_SphericalSurface(gp_Ax3(P(o), z, x), std::abs(r) * mm);
    }
    if (t == "torus-surface")
    {
      const gp_XYZ o = c.xyz(), ax = c.xyz();
      const double R = c.num(), r = c.num();
      const gp_XYZ ux = c.xyz();
      agrees          = r > 0;
      gp_Dir z(ax), x(ux);
      if (std::abs(x.Dot(z)) > 1e-9)
        x = gp_Dir(z.XYZ().Crossed(gp::DX().XYZ()).Modulus() > 1e-6 ? z.XYZ().Crossed(gp::DX().XYZ()) : z.XYZ().Crossed(gp::DY().XYZ()));
      if (R * mm <= std::abs(r) * mm)
        fail_unsupported("acis: self-intersecting torus");
      return new Geom_ToroidalSurface(gp_Ax3(P(o), z, x), R * mm, std::abs(r) * mm);
    }
    if (t == "spline-surface")
    {
      agrees = !c.reversed();
      std::string name;
      Cursor      b = subtype(c, name);
      std::optional<Cursor> at_bs = stored_bs(b, true);
      if (!at_bs)
        fail_unsupported("acis: spline surface \"" + name + "\" without B-spline data");
      return bs3_surface(*at_bs);
    }
    fail_unsupported("acis: surface type " + t);
  }

  // Whether the first curve data of an intcurve subtype is its 3D curve:
  // not when a surface comes before it ("summary" instead of the 3D data: a
  // parameter curve on that surface).
  bool at_bs_is_3d(Cursor b, const Cursor& at_bs) const
  {
    int depth = 0;
    for (; b.i < at_bs.i && b.more(); b.skip())
    {
      const Tok& t = b.peek();
      if (t.k == Tok::Open)
        ++depth;
      else if (t.k == Tok::Close)
        --depth;
      else if (depth == 0 && t.k == Tok::Word && inline_surface(t.s))
        return false;
    }
    return true;
  }
  static bool inline_surface(const std::string& w) { return w == "spline" || w == "plane" || w == "cone" || w == "sphere" || w == "torus"; }

  // An intcurve kept only as a 2D curve on a surface (e.g. "parcur summary
  // … spline … null_surface nubs …"): its 3D curve built from the two.
  occ::handle<Geom_Curve> on_surface(Cursor b, const std::string& name)
  {
    int depth = 0;
    while (b.more() && !(depth == 0 && b.peek().k == Tok::Word && inline_surface(b.peek().s)))
    {
      depth += b.peek().k == Tok::Open ? 1 : b.peek().k == Tok::Close ? -1 : 0;
      b.skip();
    }
    if (!b.more())
      fail_unsupported("acis: intcurve \"" + name + "\" without B-spline data");
    const std::string kind = b.word();
    if (kind != "spline" && kind != "plane")
      fail_unsupported("acis: intcurve \"" + name + "\" on a " + kind + " surface");
    bool                            agrees = true;
    gp_Ax3                          plane_frame;
    double                          plane_scale = 1, plane_vsign = 1;
    occ::handle<Geom_Surface>       s;
    if (kind == "plane")
    {
      // P(u, v) = root + u U + v (N x U): OCCT's (u, v) scaled by |U|.
      const gp_XYZ o = b.xyz(), n = b.xyz(), ux = b.xyz();
      plane_vsign    = b.reversed() ? -1 : 1;
      plane_scale    = ux.Modulus() > 0 ? ux.Modulus() : 1;
      plane_frame    = gp_Ax3(P(o), gp_Dir(n), gp_Dir(ux.Modulus() > 0 ? ux : gp_XYZ(1, 0, 0)));
      s              = new Geom_Plane(plane_frame);
    }
    else
      s = surface_data(b, "spline-surface", agrees);
    std::optional<Cursor> at = stored_bs(b, false);
    if (!at)
      fail_unsupported("acis: intcurve \"" + name + "\" without its parameter curve");
    occ::handle<Geom2d_BSplineCurve> c2 = bs2_curve(*at);
    if (kind == "plane")
      for (int i = 1; i <= c2->NbPoles(); ++i)
      {
        const gp_Pnt2d q = c2->Pole(i);
        c2->SetPole(i, gp_Pnt2d(q.X() * plane_scale * mm, plane_vsign * q.Y() * plane_scale * mm), c2->Weight(i));
      }
    TopoDS_Edge E;
    B.MakeEdge(E);
    B.UpdateEdge(E, c2, s, TopLoc_Location(), tol);
    B.Range(E, c2->FirstParameter(), c2->LastParameter());
    BRepLib::BuildCurve3d(E, tol);
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(E, f, l);
    if (c.IsNull())
      fail_op("acis: could not build intcurve \"" + name + "\"");
    return new Geom_TrimmedCurve(c, f, l);
  }

  // A 2D B-spline: "nubs|nurbs d form n knots… (u v [w])…".
  occ::handle<Geom2d_BSplineCurve> bs2_curve(Cursor& c)
  {
    const bool rat = c.word() == "nurbs";
    const int  deg = (int)c.num();
    c.skip(); // open / closed / periodic
    const int   n  = (int)c.num();
    const Knots kn = knots(c, n, deg);
    const int   np = pole_count(kn, deg);
    if (np < 2)
      fail_invalid("acis: bad 2D B-spline curve");
    NCollection_Array1<gp_Pnt2d> poles(1, np);
    NCollection_Array1<double>   w(1, np);
    for (int i = 1; i <= np; ++i)
    {
      const double u = c.num(), v = c.num();
      poles(i)       = gp_Pnt2d(u, v);
      w(i)           = rat ? c.num() : 1.0;
    }
    NCollection_Array1<double> K(1, (int)kn.k.size());
    NCollection_Array1<int>    M(1, (int)kn.m.size());
    for (size_t j = 0; j < kn.k.size(); ++j)
    {
      K((int)j + 1) = kn.k[j];
      M((int)j + 1) = kn.m[j];
    }
    return rat ? new Geom2d_BSplineCurve(poles, w, K, M, deg) : new Geom2d_BSplineCurve(poles, K, M, deg);
  }

  occ::handle<Geom_Curve> curve(long long idx)
  {
    auto it = curves.find(idx);
    if (it != curves.end())
      return it->second;
    Cursor      c = cursor(idx);
    const auto& t = c.rec->type;
    c.topology();
    occ::handle<Geom_Curve> out;
    if (t == "straight-curve")
    {
      const gp_XYZ o = c.xyz(), d = c.xyz();
      out            = new Geom_Line(P(o), gp_Dir(d));
    }
    else if (t == "ellipse-curve")
    {
      const gp_XYZ o = c.xyz(), n = c.xyz(), major = c.xyz();
      const double ratio = c.num();
      const double R     = major.Modulus() * mm;
      const gp_Ax2 ax(P(o), gp_Dir(n), gp_Dir(major));
      if (std::abs(ratio - 1) < 1e-12)
        out = new Geom_Circle(ax, R);
      else if (ratio < 1)
        out = new Geom_Ellipse(ax, R, R * ratio);
      else
      {
        // Minor > major: the same ellipse with the axes swapped (parameter shifted).
        out = new Geom_Ellipse(gp_Ax2(P(o), gp_Dir(n), gp_Dir(gp_Dir(n).Crossed(gp_Dir(major)))), R * ratio, R);
      }
    }
    else if (t == "intcurve-curve" || t == "intcurve-intcurve-curve")
    {
      const bool  rev = c.reversed();
      std::string name;
      Cursor      b = subtype(c, name);
      std::optional<Cursor> at_bs = stored_bs(b, false);
      if (!at_bs)
        fail_unsupported("acis: intcurve \"" + name + "\" without B-spline data");
      out = at_bs_is_3d(b, *at_bs) ? occ::handle<Geom_Curve>(bs3_curve(*at_bs)) : on_surface(b, name);
      if (rev)
        out = out->Reversed();
    }
    else if (t == "degenerate-curve")
      out = nullptr;
    else
      fail_unsupported("acis: curve type " + t);
    return curves[idx] = out;
  }

  // ---- topology

  TopoDS_Vertex vertex(long long idx)
  {
    auto it = verts.find(idx);
    if (it != verts.end())
      return it->second;
    Cursor c = cursor(idx);
    c.topology();
    c.ref(); // an edge
    if (file.asm_ && file.asm_major > 217)
      c.skip();
    if (c.peek().k != Tok::Ref)
      c.skip(); // number of edges
    const long long pt = c.ref();
    double          vt = tol;
    if (c.rec->type.rfind("tvertex", 0) == 0 && c.more() && c.peek().k == Tok::Num)
      vt = std::max(tol, c.num() * mm);
    Cursor p = cursor(pt);
    p.topology();
    TopoDS_Vertex V;
    B.MakeVertex(V, P(p.xyz()), vt);
    return verts[idx] = V;
  }

  static double param_on(const occ::handle<Geom_Curve>& c, const gp_Pnt& p)
  {
    GeomAPI_ProjectPointOnCurve pr(p, c);
    if (pr.NbPoints() > 0)
      return pr.LowerDistanceParameter();
    const double f = c->FirstParameter(), l = c->LastParameter();
    return c->Value(f).Distance(p) < c->Value(l).Distance(p) ? f : l;
  }

  // Null when the edge has no curve (a degenerate edge at an apex).
  TopoDS_Edge edge(long long idx)
  {
    auto it = edges.find(idx);
    if (it != edges.end())
      return it->second;
    Cursor c = cursor(idx);
    c.topology();
    const long long vs = c.ref();
    if (file.version > 4.0)
      c.num();
    const long long ve = c.ref();
    if (file.version > 4.0)
      c.num();
    c.ref(); // a coedge
    const long long cv  = c.ref();
    const bool      rev = c.reversed();
    TopoDS_Edge     E;
    if (cv < 0)
      return edges[idx] = E;
    occ::handle<Geom_Curve> g = curve(cv);
    if (g.IsNull())
      return edges[idx] = E;
    if (rev)
      g = g->Reversed();
    TopoDS_Vertex v1 = vertex(vs), v2 = vertex(ve);
    double        t1 = param_on(g, BRep_Tool::Pnt(v1)), t2 = param_on(g, BRep_Tool::Pnt(v2));
    if (g->IsPeriodic())
    {
      const double per = g->Period();
      while (t2 <= t1 + 1e-12)
        t2 += per;
      if (vs == ve && t2 - t1 < 1e-9)
        t2 = t1 + per;
    }
    else if (vs == ve && g->IsClosed())
    {
      t1 = g->FirstParameter();
      t2 = g->LastParameter();
    }
    if (t2 < t1)
      std::swap(t1, t2);
    B.MakeEdge(E, g, tol);
    B.Range(E, t1, t2);
    v1.Orientation(TopAbs_FORWARD);
    v2.Orientation(TopAbs_REVERSED);
    B.Add(E, v1);
    B.Add(E, v2);
    return edges[idx] = E;
  }

  TopoDS_Face face(long long idx, bool& double_sided)
  {
    Cursor c = cursor(idx);
    c.topology();
    c.ref(); // next
    const long long loop0 = c.ref();
    c.ref();          // shell
    c.ref();          // subshell
    const long long sf = c.ref();
    const bool      rev = c.reversed();
    if (c.more())
    {
      const Tok& sd = c.next();
      double_sided  = (sd.k == Tok::Word && sd.s == "double") || (sd.k == Tok::Bool && sd.b) || (sd.k == Tok::Num && sd.x != 0);
    }
    bool                      agrees = true;
    occ::handle<Geom_Surface> s;
    try
    {
      s = surface(sf, agrees);
    }
    catch (const std::exception&)
    {
      // A procedural spline surface saved without its approximation (a
      // vertex blend): filled from its boundary below.
      if (at(sf).type != "spline-surface")
        throw;
    }
    if (s.IsNull())
      return filled_face(loop0);
    TopoDS_Face F;
    B.MakeFace(F, s, tol);
    for (long long l = loop0; l >= 0;)
    {
      Cursor lc = cursor(l);
      lc.topology();
      const long long next = lc.ref(), first = lc.ref();
      TopoDS_Wire     W;
      B.MakeWire(W);
      int n = 0;
      for (long long ce = first, guard = 0; ce >= 0 && guard < 1000000; ++guard)
      {
        Cursor cc = cursor(ce);
        cc.topology();
        const long long nx = cc.ref();
        cc.ref(); // previous
        cc.ref(); // partner
        const long long ed  = cc.ref();
        const bool      crev = cc.reversed();
        TopoDS_Edge     E    = edge(ed);
        if (!E.IsNull())
        {
          E.Orientation(crev ? TopAbs_REVERSED : TopAbs_FORWARD);
          B.Add(W, E);
          ++n;
        }
        ce = nx;
        if (ce == first)
          break;
      }
      if (n)
        B.Add(F, W);
      l = next;
    }
    // ACIS loops run with the face (its outward side) on their left.
    const bool flip = rev == agrees;
    if (flip)
    {
      TopoDS_Face G;
      B.MakeFace(G, s, tol);
      for (TopoDS_Iterator w(F); w.More(); w.Next())
        B.Add(G, w.Value().Reversed());
      F = G;
      F.Reverse();
    }
    return F;
  }

  // A face over its loops' edges (no surface known): a filling surface
  // through the outer loop; its orientation is left to the shell's healing.
  TopoDS_Face filled_face(long long loop0)
  {
    BRepFill_Filling fill;
    int              n = 0;
    Cursor           lc = cursor(loop0);
    lc.topology();
    lc.ref(); // next loop
    const long long first = lc.ref();
    for (long long ce = first, guard = 0; ce >= 0 && guard < 100000; ++guard)
    {
      Cursor cc = cursor(ce);
      cc.topology();
      const long long nx = cc.ref();
      cc.ref();
      cc.ref();
      const TopoDS_Edge E = edge(cc.ref());
      if (!E.IsNull())
      {
        fill.Add(E, GeomAbs_C0);
        ++n;
      }
      ce = nx;
      if (ce == first)
        break;
    }
    if (n == 0)
      fail_unsupported("acis: a procedural face without boundary curves");
    fill.Build();
    if (!fill.IsDone())
      fail_op("acis: could not fill a procedural face");
    return fill.Face();
  }

  void shell_faces(long long sh, std::vector<long long>& out, int depth = 0)
  {
    if (sh < 0 || depth > 64)
      return;
    Cursor c = cursor(sh);
    c.topology();
    if (at(sh).type == "subshell")
    {
      c.ref(); // owner
      c.ref(); // next
      const long long child = c.ref(), f0 = c.ref();
      for (long long f = f0; f >= 0;)
      {
        out.push_back(f);
        Cursor fc = cursor(f);
        fc.topology();
        f = fc.ref();
      }
      for (long long s = child; s >= 0;)
      {
        shell_faces(s, out, depth + 1);
        Cursor sc = cursor(s);
        sc.topology();
        sc.ref();
        s = sc.ref(); // next subshell
      }
      return;
    }
    c.ref(); // next
    const long long sub = c.ref(), f0 = c.ref();
    for (long long f = f0; f >= 0;)
    {
      out.push_back(f);
      Cursor fc = cursor(f);
      fc.topology();
      f = fc.ref();
    }
    shell_faces(sub, out, depth + 1);
  }

  TopoDS_Shape body(long long bi)
  {
    Cursor c = cursor(bi);
    c.topology();
    if (file.version > 27 && !file.asm_)
      c.skip();
    const long long lump0 = c.ref();
    c.ref(); // wire
    const long long tf = c.ref();
    TopoDS_Compound part;
    B.MakeCompound(part);
    int          nsolids = 0, nshapes = 0;
    TopoDS_Shape last;
    for (long long lp = lump0; lp >= 0;)
    {
      Cursor lc = cursor(lp);
      lc.topology();
      const long long next = lc.ref(), sh0 = lc.ref();
      TopoDS_Solid    so;
      B.MakeSolid(so);
      bool                      sheet = false;
      std::vector<TopoDS_Shell> shells;
      for (long long sh = sh0; sh >= 0;)
      {
        std::vector<long long> fs;
        shell_faces(sh, fs);
        TopoDS_Shell shell;
        B.MakeShell(shell);
        for (long long f : fs)
        {
          bool dbl = false;
          B.Add(shell, face(f, dbl));
          sheet = sheet || dbl;
        }
        if (!fs.empty())
          shells.push_back(shell);
        Cursor sc = cursor(sh);
        sc.topology();
        sh = sc.ref();
      }
      for (auto& s : shells)
      {
        if (sheet)
        {
          B.Add(part, s);
          last = s;
          ++nshapes;
        }
        else
        {
          s.Closed(true);
          B.Add(so, s);
        }
      }
      if (!sheet && !shells.empty())
      {
        B.Add(part, so);
        last = so;
        ++nsolids;
        ++nshapes;
      }
      lp = next;
    }
    if (nshapes == 0)
      return TopoDS_Shape();
    TopoDS_Shape out = nshapes == 1 ? last : TopoDS_Shape(part);
    out              = heal_import(out, tol);
    if (tf >= 0)
    {
      Cursor t = cursor(tf);
      t.entity();
      double a[13];
      for (double& v : a)
        v = t.num();
      // Rows: the images of x, y, z, then the translation, then a scale.
      gp_Trsf tr;
      tr.SetValues(a[0] * a[12], a[3] * a[12], a[6] * a[12], a[9] * mm, a[1] * a[12], a[4] * a[12], a[7] * a[12], a[10] * mm, a[2] * a[12],
                   a[5] * a[12], a[8] * a[12], a[11] * mm);
      if (tr.Form() != gp_Identity)
        out = BRepBuilderAPI_Transform(out, tr, true).Shape();
    }
    return out;
  }

  // ---- names and colours

  void attributes(long long owner, XtPart& p)
  {
    Cursor c = cursor(owner);
    for (long long a = c.ref(), guard = 0; a >= 0 && guard < 100000; ++guard)
    {
      const Record& r = at(a);
      Cursor        ac = cursor(a);
      ac.entity();
      const long long next = ac.ref();
      ac.ref(); // previous
      ac.ref(); // owner
      if (r.type.rfind("rgb_color", 0) == 0)
      {
        const double r = ac.num(), g = ac.num(), b = ac.num();
        p.colour       = {r, g, b};
        p.has_colour = true;
      }
      else if (r.type.rfind("string_attrib-name_attrib", 0) == 0)
      {
        // … name value: two strings at the end ("@n text", or "n text"
        // before ACIS 7, read from the record's source).
        std::vector<std::string> ss;
        for (size_t k = ac.i; k < r.toks.size(); ++k)
        {
          const Tok& t = r.toks[k];
          if (t.k == Tok::Str)
            ss.push_back(t.s);
          else if (t.k == Tok::Num && !r.text.empty() && t.x >= 1 && t.x == std::floor(t.x))
          {
            const size_t b = t.end + 1 - r.start, n = (size_t)t.x;
            if (b + n > r.text.size())
              continue;
            ss.push_back(r.text.substr(b, n));
            while (k + 1 < r.toks.size() && r.toks[k + 1].begin < t.end + 1 + n)
              ++k;
          }
        }
        if (ss.size() >= 2 && (ss[ss.size() - 2] == "ATTRIB_XACIS_NAME" || ss[ss.size() - 2] == "ATTRIB_XSTEP_PRODUCT_ID") && p.name.empty())
          p.name = ss.back();
      }
      a = next;
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

} // namespace

std::vector<XtPart> read_sat_data(std::string data)
{
  File f;
  if (data.compare(0, 15, "ACIS BinaryFile") == 0)
    f = parse_binary(data, 15);
  else if (data.compare(0, 15, "ASM BinaryFile4") == 0)
  {
    f      = parse_binary(data, 15);
    f.asm_ = true;
  }
  else
    f = parse_text(data);
  if (f.recs.empty())
    fail_invalid("acis: no records");
  return Builder(f).parts();
}

std::vector<XtPart> read_sat_parts(const std::string& path)
{
  return read_sat_data(read_file(path));
}

} // namespace noshape
